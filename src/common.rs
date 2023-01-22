use std::{ sync, sync::atomic, time::Duration, sync::atomic::Ordering };
use sync_wait_object::{WaitEvent};
use crate::{
    Result, platform, CallbackHint
};

use platform::{TimerQueue, TimerQueueCore};

// ------------------------------------- DATA STRUCTURES & MARKERS ------------------------------------
enum FType<'h> {
    None,
    Mut(Box<dyn FnMut() + 'h>),
    Once(Box<dyn FnOnce() + 'h>)
}
unsafe impl<'h> Send for FType<'h> {}
unsafe impl<'h> Sync for FType<'h> {}

#[allow(dead_code)]
pub(crate) struct MutWrapper<'h> {
    pub hint: Option<CallbackHint>,

    idle: IdleWaitType,
    mark_deleted: atomic::AtomicBool,
    main_queue: sync::Arc<TimerQueueCore>,
    f: FType<'h>
}

pub(crate) trait MutCallable {
    fn call(&mut self) -> Result<()>;
    fn wait_idle(&self, acceptable_execution_time: Duration) -> Result<()>;
}

pub(crate) type MutWrapperUnsafeRepr = usize;

type IdleWaitType = WaitEvent<i32>;
struct CriticalSection<'a>(&'a mut IdleWaitType);

// ------------------------------------------ FUNCTIONS -----------------------------------------------
#[cfg(tracker)]
pub(crate) use with_tracker::*;

#[cfg(tracker)]
mod with_tracker {
    use std::{collections::HashSet, sync};
    use parking_lot::RwLock;
    use super::MutWrapperUnsafeRepr;

    static TRACKER_INIT: sync::Once = sync::Once::new();
    static mut TRACKER: Option<RwLock<HashSet<MutWrapperUnsafeRepr>>> = None;

    pub(crate) fn is_mutwrapper_unsafe_repr_valid(key: MutWrapperUnsafeRepr) -> bool { tracker().read().get(&key).is_some() }
    pub(crate) fn save_mutwrapper_unsafe_repr(key: MutWrapperUnsafeRepr) { tracker().write().insert(key); }
    pub(crate) fn remove_mutwrapper_unsafe_repr(key: MutWrapperUnsafeRepr) { tracker().write().remove(&key); }

    fn tracker() -> &'static RwLock<HashSet<MutWrapperUnsafeRepr>> {
        unsafe {
            TRACKER_INIT.call_once(|| {
                TRACKER = Some(RwLock::new(HashSet::new()));
            });
            TRACKER.as_ref().unwrap()
        }
    }
}

#[cfg(not(tracker))]
pub(crate) use without_tracker::*;

#[cfg(not(tracker))]
mod without_tracker {
    use super::MutWrapperUnsafeRepr;

    pub(crate) fn is_mutwrapper_unsafe_repr_valid(_key: MutWrapperUnsafeRepr) -> bool { true }
    pub(crate) fn save_mutwrapper_unsafe_repr(_key: MutWrapperUnsafeRepr) { }
    pub(crate) fn remove_mutwrapper_unsafe_repr(_key: MutWrapperUnsafeRepr) { }
}

// --------------------------------------- IMPLEMENTATIONS --------------------------------------------
impl<'h> MutWrapper<'h> {
    pub fn new<F>(main_queue: sync::Arc<TimerQueueCore>, hint: Option<CallbackHint>, handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> {
            hint,
            idle: IdleWaitType::new_init(0),
            mark_deleted: atomic::AtomicBool::new(false),
            main_queue,
            f: FType::Mut(Box::new(handler))
        }
    }
    pub fn new_once<F>(main_queue: sync::Arc<TimerQueueCore>, hints: Option<CallbackHint>, handler: F) -> Self where F: FnOnce() + Send + 'h {
        MutWrapper::<'h> {
            hint: hints,
            idle: IdleWaitType::new_init(0),
            mark_deleted: atomic::AtomicBool::new(false),
            main_queue,
            f: FType::Once(Box::new(handler))
        }
    }
    #[allow(dead_code)]
    pub fn timer_queue(&self) -> TimerQueue {
        TimerQueue::new_with_context(self.main_queue.clone())
    }
    pub(crate) fn mark_delete(&self) {
        self.mark_deleted.store(true, Ordering::SeqCst);
    }
}

impl<'h> MutCallable for MutWrapper<'h> {
    fn call(&mut self) -> Result<()> {
        let section = CriticalSection::start(&mut self.idle);
        let is_deleted = self.mark_deleted.load(Ordering::SeqCst);
        if !is_deleted {
            match &mut self.f {
                FType::Once(_) => {
                    if let FType::Once(f) = std::mem::replace(&mut self.f, FType::None) {
                        f();
                    }
                }
                FType::Mut(ref mut f) => { (*f)(); }
                FType::None => ()
            }
        }
        drop(section);
        Ok(())
    }

    fn wait_idle(&self, acceptable_execution_time: Duration) -> Result<()> {
        match self.idle.wait(Some(acceptable_execution_time), |v| *v == 0) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into())
        }
    }
}

impl<'a> CriticalSection<'a> {
    fn start(idle: &'a mut IdleWaitType) -> Self {
        idle.set_state_func(|v| *v + 1).unwrap();
        Self(idle)
    }
}

impl<'a> Drop for CriticalSection<'a> {
    fn drop(&mut self) {
        self.0.set_state_func(|v|{
            let new_value = *v - 1;
            assert!(new_value >= 0);
            new_value
        }).unwrap();
    }
}