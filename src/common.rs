use std::{ time, process, sync };
use parking_lot::RwLock;
use crate::{
    Result, platform, CallbackHint
};

use platform::{TimerQueue, TimerQueueCore};

#[allow(dead_code)]
pub(crate) struct MutWrapper<'h> {
    pub hints: Option<CallbackHint>,

    mark_deleted: RwLock<bool>,
    main_queue: sync::Arc<TimerQueueCore>,
    f: Box<dyn FnMut() + 'h>
}

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<'h> MutWrapper<'h> {
    pub fn new<F>(main_queue: sync::Arc<TimerQueueCore>, hints: Option<CallbackHint>, handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> {
            hints,
            mark_deleted: RwLock::new(false),
            main_queue,
            f: Box::new(handler)
        }
    }
    #[allow(dead_code)]
    pub fn timer_queue(&self) -> TimerQueue {
        TimerQueue::new_with_context(self.main_queue.clone())
    }
    pub fn call(&mut self) -> Result<()> {
        let is_deleted = self.mark_deleted.read();
        if !*is_deleted {
            (self.f)();
        }
        Ok(())
    }
    pub(crate) fn mark_delete(&self, acceptable_execution_time: time::Duration) {
        match self.mark_deleted.try_write_for(acceptable_execution_time) {
            None => {
                println!("ERROR: Wait for execution timed out! Timer handler is being executed while timer is also being destroyed! Program aborts!");
                process::abort();
            },
            Some(mut is_deleted) => {
                *is_deleted = true;
            }
        }
    }
}
