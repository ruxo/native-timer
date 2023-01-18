use std::sync::RwLock;
use crate::{
    Result, platform::ManualResetEvent, CallbackHint
};

pub(crate) struct MutWrapper<'h> {
    hints: Option<CallbackHint>,
    f: Box<dyn FnMut() + 'h>,
    idling: ManualResetEvent,
    mark_deleted: RwLock<bool>
}

struct CriticalSection<'e> {
    idling: &'e mut ManualResetEvent
}

// ------------------------------ IMPLEMENTATIONS ------------------------------
impl<'h> MutWrapper<'h> {
    pub fn new<F>(hints: Option<CallbackHint>, handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> {
            hints,
            f: Box::new(handler),
            idling: ManualResetEvent::new_init(true),
            mark_deleted: RwLock::new(false)
        }
    }
    #[inline]
    pub fn hints(&self) -> &Option<CallbackHint> { &self.hints }
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

