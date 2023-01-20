use std::sync::RwLock;
use crate::{
    Result, platform, platform::ManualResetEvent, CallbackHint
};

#[allow(dead_code)]
pub(crate) struct MutWrapper<'q, 'h> {
    pub main_queue: &'q platform::TimerQueue,
    pub hints: Option<CallbackHint>,

    f: Box<dyn FnMut() + 'h>,
    pub mark_deleted: RwLock<bool>
}

struct CriticalSection<'e> {
    idling: &'e mut ManualResetEvent
}

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<'q,'h> MutWrapper<'q,'h> {
    pub fn new<F>(main_queue: &'q platform::TimerQueue, hints: Option<CallbackHint>, handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'q,'h> {
            main_queue,
            hints,
            f: Box::new(handler),
            mark_deleted: RwLock::new(false)
        }
    }
    pub fn call(&mut self) -> Result<()> {
        let is_deleted = self.mark_deleted.read().unwrap();
        if !*is_deleted {
            (self.f)();
        }
        Ok(())
    }
}
