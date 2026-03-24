use spin::Mutex;

static PID_COUNTER: Mutex<usize> = Mutex::new(1);

pub fn alloc_pid() -> usize {
    let mut counter = PID_COUNTER.lock();
    let pid = *counter;
    *counter += 1;
    pid
}
