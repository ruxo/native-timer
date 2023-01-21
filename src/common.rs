use std::marker::PhantomData;
use std::sync;
use parking_lot::RwLock;
use crate::{
    Result, platform, CallbackHint
};

use platform::{TimerQueue, TimerQueueCore};

pub(crate) struct MutWrapper<'q, 'h> {
    pub hints: Option<CallbackHint>,
    pub mark_deleted: RwLock<bool>,

    main_queue: sync::Arc<TimerQueueCore>,
    f: Box<dyn FnMut() + 'h>,
    // TODO going to remove 'q lifetime
    _queue_lifetime: PhantomData<&'q ()>
}

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<'q,'h> MutWrapper<'q,'h> {
    pub fn new<F>(main_queue: sync::Arc<TimerQueueCore>, hints: Option<CallbackHint>, handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'q,'h> {
            hints,
            mark_deleted: RwLock::new(false),
            main_queue,
            f: Box::new(handler),
            _queue_lifetime: PhantomData
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
