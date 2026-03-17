use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Mutex;
use super::{Task, Pid};
use lazy_static::lazy_static;

pub struct TaskManager {
    tasks: BTreeMap<i32, Arc<Task>>,
    ready_queue: alloc::collections::VecDeque<Arc<Task>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            ready_queue: alloc::collections::VecDeque::new(),
        }
    }

    pub fn add(&mut self, task: Arc<Task>) {
        self.tasks.insert(task.pid.0, task.clone());
        self.ready_queue.push_back(task);
    }

    pub fn remove(&mut self, pid: i32) {
        self.tasks.remove(&pid);
    }

    pub fn get(&self, pid: i32) -> Option<Arc<Task>> {
        self.tasks.get(&pid).cloned()
    }

    pub fn pop_ready(&mut self) -> Option<Arc<Task>> {
        self.ready_queue.pop_front()
    }

    pub fn push_ready(&mut self, task: Arc<Task>) {
        self.ready_queue.push_back(task);
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());
}

pub fn init() {
    // 初始化完成
}

pub fn add_task(task: Arc<Task>) {
    TASK_MANAGER.lock().add(task);
}

pub fn remove_task(pid: i32) {
    TASK_MANAGER.lock().remove(pid);
}

pub fn get_task(pid: i32) -> Option<Arc<Task>> {
    TASK_MANAGER.lock().get(pid)
}
