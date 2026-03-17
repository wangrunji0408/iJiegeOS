use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::trap::{TrapContext, TRAP_CONTEXT_BASE};
use crate::fs::FileDescriptor;
use crate::mm::{MemorySet, MapPermission, PhysAddr, VirtAddr};
use crate::signal::Signal;
use crate::task::{TaskContext, Pid};
use crate::task::pid::PID_ALLOCATOR;

/// 任务（进程）状态
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Zombie(i32),  // 退出码
}

/// 内核栈大小：4MB
pub const KERNEL_STACK_SIZE: usize = 4 * 1024 * 1024;

/// 进程控制块
pub struct Task {
    pub pid: Pid,
    pub kernel_stack: KernelStack,
    pub inner: Mutex<TaskInner>,
}

pub struct TaskInner {
    pub state: TaskState,
    pub task_cx: TaskContext,
    pub memory_set: MemorySet,
    /// TrapContext 保存在内核栈上，这是内核栈中 TrapContext 的地址
    pub trap_cx_addr: usize,
    pub parent: Option<Weak<Task>>,
    pub children: Vec<Arc<Task>>,
    pub exit_code: i32,
    pub pending_signals: Vec<Signal>,

    /// 文件描述符表
    pub fd_table: Vec<Option<Arc<dyn FileDescriptor>>>,

    /// 工作目录
    pub cwd: String,

    /// 用户地址（brk）
    pub heap_start: usize,
    pub heap_end: usize,

    /// 用户 ID
    pub uid: u32,
    pub gid: u32,
    pub euid: u32,
    pub egid: u32,

    /// 进程组 ID
    pub pgid: i32,
    pub sid: i32,

    /// 等待状态
    pub robust_list: usize,
    pub clear_child_tid: usize,
    pub set_child_tid: usize,

    /// 资源限制
    pub rlimits: [RLimit; 16],

    /// 线程 ID
    pub tid: Pid,
}

#[derive(Clone, Copy)]
pub struct RLimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

impl Default for RLimit {
    fn default() -> Self {
        Self { rlim_cur: u64::MAX, rlim_max: u64::MAX }
    }
}

/// 内核栈
pub struct KernelStack {
    pub data: alloc::vec::Vec<u8>,
}

impl KernelStack {
    pub fn new() -> Self {
        Self {
            data: alloc::vec![0u8; KERNEL_STACK_SIZE],
        }
    }

    /// 内核栈顶（高地址）
    pub fn top(&self) -> usize {
        let bottom = self.data.as_ptr() as usize;
        bottom + self.data.len()
    }

    /// 在栈顶分配 TrapContext 空间（向下生长）
    pub fn trap_cx_addr(&self) -> usize {
        self.top() - core::mem::size_of::<TrapContext>()
    }
}

impl Task {
    /// 从 ELF 数据创建新进程
    pub fn new_from_elf(elf_data: &[u8]) -> Self {
        let pid = PID_ALLOCATOR.alloc();
        let kernel_stack = KernelStack::new();

        // TrapContext 放在内核栈顶部
        let trap_cx_addr = kernel_stack.trap_cx_addr();
        // 内核 sp 指向 TrapContext 位置（向下，trap_cx_addr 就是 TrapContext 起始）
        let kernel_sp = trap_cx_addr;

        // 创建用户地址空间
        let (memory_set, user_sp, entry_point) = MemorySet::new_user(elf_data);

        // 创建 TrapContext 并放在内核栈上
        let trap_cx = unsafe { &mut *(trap_cx_addr as *mut TrapContext) };
        *trap_cx = TrapContext::new(entry_point, user_sp);

        // 创建内核任务上下文
        // ra = trap_return，sp 指向 TrapContext 的位置
        let task_cx = TaskContext::goto_trap_return(kernel_sp);

        // 初始化文件描述符表
        let fd_table = setup_initial_fds();

        let heap_start = user_sp; // 将在ELF加载后修正

        let inner = TaskInner {
            state: TaskState::Ready,
            task_cx,
            memory_set,
            trap_cx_addr,
            parent: None,
            children: Vec::new(),
            exit_code: 0,
            pending_signals: Vec::new(),
            fd_table,
            cwd: String::from("/"),
            heap_start,
            heap_end: heap_start,
            uid: 0,
            gid: 0,
            euid: 0,
            egid: 0,
            pgid: pid.0,
            sid: pid.0,
            robust_list: 0,
            clear_child_tid: 0,
            set_child_tid: 0,
            rlimits: [RLimit::default(); 16],
            tid: pid,
        };

        Task {
            pid,
            kernel_stack,
            inner: Mutex::new(inner),
        }
    }

    /// 从 ELF 数据创建新进程（带命令行参数）
    pub fn new_from_elf_with_args(elf_data: &[u8], _path: &str, argv: &[&str], envp: &[&str]) -> Self {
        let pid = PID_ALLOCATOR.alloc();
        let kernel_stack = KernelStack::new();

        // 创建用户地址空间
        let (mut memory_set, user_sp_top, entry_point) = MemorySet::new_user(elf_data);

        // 在用户栈上设置 argv/envp
        let user_sp = setup_user_stack(&mut memory_set, user_sp_top, argv, envp);

        // TrapContext 放在内核栈顶部
        let trap_cx_addr = kernel_stack.trap_cx_addr();
        let kernel_sp = trap_cx_addr;

        // 创建 TrapContext
        let trap_cx = unsafe { &mut *(trap_cx_addr as *mut TrapContext) };
        *trap_cx = TrapContext::new(entry_point, user_sp);
        // argc 在 a0，argv 在 a1
        trap_cx.x[10] = argv.len();  // a0 = argc
        trap_cx.x[11] = user_sp;     // a1 = argv（栈顶就是 argv 数组）

        let task_cx = TaskContext::goto_trap_return(kernel_sp);
        let fd_table = setup_initial_fds();

        let inner = TaskInner {
            state: TaskState::Ready,
            task_cx,
            memory_set,
            trap_cx_addr,
            parent: None,
            children: Vec::new(),
            exit_code: 0,
            pending_signals: Vec::new(),
            fd_table,
            cwd: String::from("/"),
            heap_start: user_sp_top,
            heap_end: user_sp_top,
            uid: 0,
            gid: 0,
            euid: 0,
            egid: 0,
            pgid: pid.0,
            sid: pid.0,
            robust_list: 0,
            clear_child_tid: 0,
            set_child_tid: 0,
            rlimits: [RLimit::default(); 16],
            tid: pid,
        };

        Task { pid, kernel_stack, inner: Mutex::new(inner) }
    }

    pub fn inner_exclusive_access(&self) -> spin::MutexGuard<TaskInner> {
        self.inner.lock()
    }
}

impl TaskInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        unsafe { &mut *(self.trap_cx_addr as *mut TrapContext) }
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    /// 分配文件描述符
    pub fn alloc_fd(&mut self) -> usize {
        for (fd, slot) in self.fd_table.iter().enumerate() {
            if slot.is_none() {
                return fd;
            }
        }
        self.fd_table.push(None);
        self.fd_table.len() - 1
    }

    /// 关闭文件描述符
    pub fn close_fd(&mut self, fd: usize) -> bool {
        if fd < self.fd_table.len() {
            self.fd_table[fd] = None;
            true
        } else {
            false
        }
    }

    /// 获取文件描述符
    pub fn get_fd(&self, fd: usize) -> Option<Arc<dyn FileDescriptor>> {
        if fd < self.fd_table.len() {
            self.fd_table[fd].clone()
        } else {
            None
        }
    }
}

/// 设置初始文件描述符（stdin=0, stdout=1, stderr=2）
fn setup_initial_fds() -> Vec<Option<Arc<dyn FileDescriptor>>> {
    let mut table = Vec::with_capacity(32);
    // fd 0: stdin
    table.push(Some(Arc::new(crate::fs::Stdin) as Arc<dyn FileDescriptor>));
    // fd 1: stdout
    table.push(Some(Arc::new(crate::fs::Stdout) as Arc<dyn FileDescriptor>));
    // fd 2: stderr
    table.push(Some(Arc::new(crate::fs::Stderr) as Arc<dyn FileDescriptor>));
    table
}
