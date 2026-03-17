/// 内核任务上下文（调度器切换时保存）
/// 注意：这与 TrapContext 不同，TrapContext 保存用户态寄存器
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskContext {
    /// ra: 返回地址（切换回来时跳转到哪里）
    pub ra: usize,
    /// sp: 内核栈指针
    pub sp: usize,
    /// 被调用者保存寄存器 s0-s11
    pub s: [usize; 12],
}

impl TaskContext {
    pub fn new_empty() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// 创建一个新任务的上下文，ra 指向 trap_return
    pub fn goto_trap_return(kernel_sp: usize) -> Self {
        extern "C" {
            fn trap_return();
        }
        Self {
            ra: trap_return as usize,
            sp: kernel_sp,
            s: [0; 12],
        }
    }
}
