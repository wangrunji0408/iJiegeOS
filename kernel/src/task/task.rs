use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::trap::{TrapContext, TRAP_CONTEXT_BASE};
use crate::fs::FileDescriptor;
use crate::mm::{MemorySet, MapPermission, PhysAddr, VirtAddr, TRAP_CONTEXT_BASE as MM_TRAP_CONTEXT_BASE};
use crate::signal::Signal;
use crate::task::{TaskContext, Pid, PID_ALLOCATOR};

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
    pub trap_cx_ppn: crate::mm::PhysPageNum,
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
}

impl Task {
    /// 从 ELF 数据创建新进程
    pub fn new_from_elf(elf_data: &[u8]) -> Self {
        let pid = PID_ALLOCATOR.alloc();
        let kernel_stack = KernelStack::new();
        let kernel_sp = kernel_stack.top();

        // 创建用户地址空间
        let (mut memory_set, user_sp, entry_point) = MemorySet::new_user(elf_data);

        // 获取 TrapContext 的物理页号
        let trap_cx_ppn = memory_set.translate(
            VirtAddr::from(TRAP_CONTEXT_BASE).floor()
        ).unwrap().ppn();

        // 创建 TrapContext
        let trap_cx = trap_cx_ppn.get_mut::<TrapContext>();
        *trap_cx = TrapContext::new(
            entry_point,
            user_sp,
            crate::mm::KERNEL_SPACE.lock().token(),
            kernel_sp,
            crate::arch::trap::trap_handler_entry as usize,
            memory_set.token(),
        );

        // 创建内核任务上下文
        let task_cx = TaskContext::goto_trap_return(kernel_sp);

        // 初始化文件描述符表
        let fd_table = setup_initial_fds();

        let inner = TaskInner {
            state: TaskState::Ready,
            task_cx,
            memory_set,
            trap_cx_ppn,
            parent: None,
            children: Vec::new(),
            exit_code: 0,
            pending_signals: Vec::new(),
            fd_table,
            cwd: String::from("/"),
            heap_start: user_sp,  // 将在ELF加载后设置
            heap_end: user_sp,
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

    pub fn inner_exclusive_access(&self) -> spin::MutexGuard<TaskInner> {
        self.inner.lock()
    }
}

impl TaskInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
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
