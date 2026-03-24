use crate::trap::TrapContext;

pub fn syscall(id: usize, args: [usize; 6], cx: &mut TrapContext) -> isize {
    println!("[syscall] Unimplemented syscall: {}", id);
    -1
}
