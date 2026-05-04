pub mod archive_commands;
pub mod benchmark_commands;
pub mod file_manager_commands;
pub mod hash_commands;
pub mod settings_commands;
pub mod update_commands;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

type TaskId = u64;

static TASK_REGISTRY: OnceLock<Mutex<HashMap<TaskId, Arc<AtomicBool>>>> = OnceLock::new();

fn task_registry() -> &'static Mutex<HashMap<TaskId, Arc<AtomicBool>>> {
    TASK_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_task(id: TaskId) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    task_registry()
        .lock()
        .expect("Task registry lock poisoned")
        .insert(id, Arc::clone(&cancel));
    cancel
}

pub fn cancel_task(id: TaskId) -> bool {
    if let Some(cancel) = task_registry()
        .lock()
        .expect("Task registry lock poisoned")
        .get(&id)
    {
        cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        true
    } else {
        false
    }
}

pub fn unregister_task(id: TaskId) {
    task_registry()
        .lock()
        .expect("Task registry lock poisoned")
        .remove(&id);
}
