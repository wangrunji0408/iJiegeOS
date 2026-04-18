use crate::loader::{load_elf, LoadedElf};
use crate::mm::memory_set::MemorySet;
use crate::trap::TrapContext;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use spin::Mutex;

pub const KERNEL_STACK_SIZE: usize = 32 * 4096;

pub struct KernelStack {
    pub base: *mut u8,
    pub size: usize,
}

unsafe impl Send for KernelStack {}
unsafe impl Sync for KernelStack {}

impl KernelStack {
    pub fn new() -> Self {
        let b: Box<[u8]> = alloc::vec![0u8; KERNEL_STACK_SIZE].into_boxed_slice();
        let base = Box::leak(b).as_mut_ptr();
        Self { base, size: KERNEL_STACK_SIZE }
    }
    pub fn top(&self) -> usize { self.base as usize + self.size }
}

/// Each task has a TrapContext in a pinned heap slot (UnsafeCell on the kernel heap).
pub struct TrapBox(pub UnsafeCell<TrapContext>);
unsafe impl Send for TrapBox {}
unsafe impl Sync for TrapBox {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState { Ready, Running, Zombie }

pub struct FileTable {
    pub files: Vec<Option<Arc<dyn crate::fs::File>>>,
}

impl FileTable {
    pub fn new() -> Self { Self { files: (0..64).map(|_| None).collect() } }
    pub fn alloc(&mut self, file: Arc<dyn crate::fs::File>) -> Option<i32> {
        for (i, slot) in self.files.iter_mut().enumerate() {
            if slot.is_none() { *slot = Some(file); return Some(i as i32); }
        }
        self.files.push(Some(file));
        Some((self.files.len() - 1) as i32)
    }
    pub fn get(&self, fd: i32) -> Option<Arc<dyn crate::fs::File>> {
        if fd < 0 { return None; }
        self.files.get(fd as usize).and_then(|x| x.clone())
    }
    pub fn close(&mut self, fd: i32) -> bool {
        if fd < 0 { return false; }
        if let Some(slot) = self.files.get_mut(fd as usize) {
            if slot.is_some() { *slot = None; return true; }
        }
        false
    }
}

pub struct Task {
    pub pid: usize,
    pub memory: Mutex<MemorySet>,
    pub kstack: KernelStack,
    pub trap_cx: Box<UnsafeCell<TrapContext>>,
    pub state: Mutex<TaskState>,
    pub exit_code: Mutex<i32>,
    pub program_break: Mutex<usize>,
    pub heap_base: usize,
    pub mmap_top: Mutex<usize>,
    pub cwd: Mutex<String>,
    pub files: Mutex<FileTable>,
    pub parent: Mutex<Option<Arc<Task>>>,
    pub children: Mutex<Vec<Arc<Task>>>,
}

unsafe impl Send for Task {}
unsafe impl Sync for Task {}

static PID_COUNTER: Mutex<usize> = Mutex::new(1);
fn next_pid() -> usize { let mut g = PID_COUNTER.lock(); let p = *g; *g += 1; p }

pub const MMAP_TOP: usize = 0x3F00_0000;

impl Task {
    pub fn from_elf(data: &[u8], args: &[&str], envs: &[&str]) -> Arc<Self> {
        crate::println!("[kernel] Task::from_elf start");
        let LoadedElf { mut memory, entry, mut stack_top, program_break, auxv_phdr, phnum, phent } = load_elf(data);
        crate::println!("[kernel] elf loaded, entry={:#x}", entry);
        let sp_after = setup_user_stack(&mut memory, &mut stack_top, args, envs, auxv_phdr, phnum, phent, entry);
        crate::println!("[kernel] user stack set up, sp={:#x}", sp_after);
        let kstack = KernelStack::new();
        let kstack_top = kstack.top();
        let cx = TrapContext::app_init(entry, sp_after, kstack_top);
        let trap_cx = Box::new(UnsafeCell::new(cx));
        crate::println!("[kernel] trap cx built");
        let mut files = FileTable::new();
        // fd 0..3 are stdin/out/err
        files.alloc(Arc::new(crate::fs::Stdin)).unwrap();
        files.alloc(Arc::new(crate::fs::Stdout)).unwrap();
        files.alloc(Arc::new(crate::fs::Stderr)).unwrap();
        Arc::new(Self {
            pid: next_pid(),
            memory: Mutex::new(memory),
            kstack,
            trap_cx,
            state: Mutex::new(TaskState::Ready),
            exit_code: Mutex::new(0),
            program_break: Mutex::new(program_break),
            heap_base: program_break,
            mmap_top: Mutex::new(MMAP_TOP),
            cwd: Mutex::new(String::from("/")),
            files: Mutex::new(files),
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    pub fn trap_cx_ptr(&self) -> *mut TrapContext { self.trap_cx.get() }
}

fn setup_user_stack(ms: &mut MemorySet, stack_top: &mut usize, args: &[&str], envs: &[&str],
                    phdr: usize, phnum: usize, phent: usize, entry: usize) -> usize {
    use crate::mm::address::VirtAddr;
    use crate::mm::page_table::write_user_bytes;

    let mut sp = *stack_top;

    // Push argv/envp strings, collect pointers
    let push_str = |sp: &mut usize, s: &[u8]| -> usize {
        *sp -= s.len() + 1;
        *sp &= !7;
        write_user_bytes(&ms.page_table, VirtAddr(*sp), s);
        // null terminator
        write_user_bytes(&ms.page_table, VirtAddr(*sp + s.len()), &[0u8]);
        *sp
    };

    let mut argv_ptrs: Vec<usize> = args.iter().map(|a| push_str(&mut sp, a.as_bytes())).collect();
    let mut envp_ptrs: Vec<usize> = envs.iter().map(|e| push_str(&mut sp, e.as_bytes())).collect();

    // random 16 bytes for AT_RANDOM
    sp -= 16; sp &= !7;
    let rand_addr = sp;
    let rand_bytes = [0x42u8; 16];
    write_user_bytes(&ms.page_table, VirtAddr(rand_addr), &rand_bytes);

    // platform string "riscv64"
    let plat = b"riscv64\0";
    sp -= plat.len(); sp &= !7;
    let plat_addr = sp;
    write_user_bytes(&ms.page_table, VirtAddr(plat_addr), plat);

    // Align to 16 bytes
    sp &= !15;

    // auxv: (type, value)* 0 pair
    let auxv: &[(usize, usize)] = &[
        (3,  phdr),       // AT_PHDR
        (4,  phent),      // AT_PHENT
        (5,  phnum),      // AT_PHNUM
        (6,  4096),       // AT_PAGESZ
        (9,  entry),      // AT_ENTRY
        (11, 0),          // AT_UID
        (12, 0),          // AT_EUID
        (13, 0),          // AT_GID
        (14, 0),          // AT_EGID
        (15, plat_addr),  // AT_PLATFORM
        (17, 100),        // AT_CLKTCK
        (23, 0),          // AT_SECURE
        (25, rand_addr),  // AT_RANDOM
        (31, plat_addr),  // AT_EXECFN
        (0,  0),
    ];

    // Total bytes: argc + argv* + NULL + envp* + NULL + auxv
    let total_words = 1 + args.len() + 1 + envs.len() + 1 + auxv.len() * 2;
    sp -= total_words * 8;
    sp &= !15; // Linux requires 16-byte alignment of stack at entry

    let mut cursor = sp;
    let mut w = |cursor: &mut usize, v: usize| {
        let bytes = v.to_le_bytes();
        write_user_bytes(&ms.page_table, VirtAddr(*cursor), &bytes);
        *cursor += 8;
    };
    w(&mut cursor, args.len()); // argc
    for p in &argv_ptrs { w(&mut cursor, *p); }
    w(&mut cursor, 0);
    for p in &envp_ptrs { w(&mut cursor, *p); }
    w(&mut cursor, 0);
    for (t, v) in auxv { w(&mut cursor, *t); w(&mut cursor, *v); }

    let _ = argv_ptrs.drain(..);
    let _ = envp_ptrs.drain(..);
    sp
}

/// Global task manager: ready queue + current task.
pub struct Scheduler {
    pub ready: VecDeque<Arc<Task>>,
    pub current: Option<Arc<Task>>,
}

impl Scheduler {
    const fn new() -> Self { Self { ready: VecDeque::new(), current: None } }
}

pub static SCHED: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn add_task(t: Arc<Task>) { SCHED.lock().ready.push_back(t); }

pub fn init() {}

pub fn current() -> Arc<Task> {
    SCHED.lock().current.as_ref().expect("no current task").clone()
}
pub fn current_opt() -> Option<Arc<Task>> { SCHED.lock().current.clone() }
pub fn current_pid_opt() -> Option<usize> { SCHED.lock().current.as_ref().map(|t| t.pid) }

pub fn scheduler_tick() {
    // Preemption: let current yield
    yield_current();
}

pub fn yield_current() {
    let cur = {
        let mut s = SCHED.lock();
        let cur = s.current.take();
        if let Some(ref t) = cur {
            if *t.state.lock() == TaskState::Running {
                *t.state.lock() = TaskState::Ready;
                s.ready.push_back(t.clone());
            }
        }
        cur
    };
    let _ = cur;
    run_next();
}

pub fn exit_current(code: i32) -> ! {
    {
        let s = SCHED.lock();
        if let Some(t) = &s.current {
            *t.state.lock() = TaskState::Zombie;
            *t.exit_code.lock() = code;
        }
    }
    crate::println!("[kernel] task exited with code {}", code);
    // For single-process demo: if no more ready tasks, shut down
    let has_more = {
        let s = SCHED.lock();
        !s.ready.is_empty()
    };
    if !has_more { crate::sbi::shutdown(code != 0); }
    SCHED.lock().current = None;
    run_next();
    unreachable!()
}

pub fn run_next() -> ! {
    loop {
        let next = SCHED.lock().ready.pop_front();
        if let Some(t) = next {
            *t.state.lock() = TaskState::Running;
            let cx_ptr = t.trap_cx_ptr();
            SCHED.lock().current = Some(t.clone());
            // activate page table
            t.memory.lock().activate();
            // set sscratch to the trap-context address so __alltraps can land on it
            unsafe {
                core::arch::asm!("csrw sscratch, {}", in(reg) cx_ptr as usize);
            }
            crate::trap::trap_return(cx_ptr as usize);
        } else {
            // no ready tasks
            unsafe { riscv::asm::wfi(); }
        }
    }
}

pub fn handle_page_fault(_stval: usize) -> bool { false }
