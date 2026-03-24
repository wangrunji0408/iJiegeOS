use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;
use crate::mm::{MemorySet, VirtAddr, PhysAddr, PTEFlags};
use crate::trap::TrapContext;
use crate::config::*;
use super::switch::TaskContext;
use super::pid::alloc_pid;
use crate::fs::FileDescriptor;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
    Zombie,
    Sleeping,
}

pub struct Process {
    pub pid: usize,
    pub ppid: usize,
    pub state: ProcessState,
    pub exit_code: i32,
    pub memory_set: MemorySet,
    pub task_cx: TaskContext,
    pub trap_cx: TrapContext,
    pub kernel_stack: usize,
    pub brk: usize,
    pub brk_start: usize,
    pub fd_table: Vec<Option<Arc<Mutex<FileDescriptor>>>>,
    pub cwd: String,
    pub children: Vec<usize>,
    pub heap_bottom: usize,
    pub mmap_top: usize,
    // Signal handling
    pub pending_signals: u64,
    pub signal_mask: u64,
}

impl Process {
    pub fn new_empty() -> Self {
        let memory_set = MemorySet::new_bare();
        let mut fd_table: Vec<Option<Arc<Mutex<FileDescriptor>>>> = Vec::new();

        // Create stdin, stdout, stderr
        fd_table.push(Some(Arc::new(Mutex::new(FileDescriptor::Stdin))));
        fd_table.push(Some(Arc::new(Mutex::new(FileDescriptor::Stdout))));
        fd_table.push(Some(Arc::new(Mutex::new(FileDescriptor::Stderr))));

        Self {
            pid: 0,
            ppid: 0,
            state: ProcessState::Ready,
            exit_code: 0,
            memory_set,
            task_cx: TaskContext::default(),
            trap_cx: TrapContext {
                x: [0; 32],
                sstatus: 0,
                sepc: 0,
                kernel_satp: 0,
                kernel_sp: 0,
                trap_handler: 0,
            },
            kernel_stack: 0,
            brk: 0,
            brk_start: 0,
            fd_table,
            cwd: String::from("/"),
            children: Vec::new(),
            heap_bottom: 0,
            mmap_top: 0x3f_ffff_f000, // Just below the top of user space
            pending_signals: 0,
            signal_mask: 0,
        }
    }

    pub fn alloc_fd(&mut self) -> usize {
        for (i, fd) in self.fd_table.iter().enumerate() {
            if fd.is_none() {
                return i;
            }
        }
        self.fd_table.push(None);
        self.fd_table.len() - 1
    }

    pub fn get_fd(&self, fd: usize) -> Option<Arc<Mutex<FileDescriptor>>> {
        self.fd_table.get(fd).and_then(|f| f.clone())
    }

    pub fn close_fd(&mut self, fd: usize) -> bool {
        if fd < self.fd_table.len() && self.fd_table[fd].is_some() {
            self.fd_table[fd] = None;
            true
        } else {
            false
        }
    }

    pub fn token(&self) -> usize {
        self.memory_set.token()
    }
}
