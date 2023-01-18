use std::sync::RwLock;
use crate::{
    Result, platform, platform::ManualResetEvent, CallbackHint
};

pub(crate) struct MutWrapper<'q, 'h> {
    pub main_queue: &'q platform::TimerQueue,
    pub hints: Option<CallbackHint>,
    f: Box<dyn FnMut() + 'h>,
    pub idling: ManualResetEvent,
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
            idling: ManualResetEvent::new_init(true),
            mark_deleted: RwLock::new(false)
        }
    }
    pub fn call(&mut self) -> Result<()> {
        let is_deleted = self.mark_deleted.read().unwrap();
        if !*is_deleted {
            let cs = CriticalSection::new(&mut self.idling);
            (self.f)();
            drop(cs);
        }
        Ok(())
    }
}

impl<'e> CriticalSection<'e> {
    fn new(idling: &'e mut ManualResetEvent) -> Self {
        idling.reset().unwrap();
        Self { idling }
    }
}

impl<'e> Drop for CriticalSection<'e> {
    fn drop(&mut self) {
        self.idling.set().unwrap();
    }
}

