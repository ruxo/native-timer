use std::sync;
use parking_lot::RwLock;
use crate::{
    Result, platform, CallbackHint
};

use platform::{TimerQueue, TimerQueueCore};

pub(crate) struct MutWrapper<'h> {
    pub hints: Option<CallbackHint>,
    pub mark_deleted: RwLock<bool>,

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
}
