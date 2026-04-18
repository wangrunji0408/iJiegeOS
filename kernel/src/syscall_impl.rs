use crate::syscall::*;
use crate::trap::TrapContext;

pub fn dispatch(id: usize, _args: [usize; 6], _cx: &mut TrapContext) -> isize {
    match id {
        SYS_EXIT | SYS_EXIT_GROUP => {
            crate::task::exit_current(0);
        }
        _ => {
            crate::println!("[kernel] unimpl syscall {}", id);
            -38 // ENOSYS
        }
    }
}
