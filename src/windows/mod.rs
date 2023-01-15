mod waitable_objects;

use std::{
    time::Duration,
    sync::{Once, RwLock},
    ffi::c_void,
    process::abort
};
use windows::Win32::{
    Foundation::{HANDLE, BOOLEAN},
    System::Threading::*,
};
use crate::timer::{CallbackHint, Result};
use waitable_objects::ManualResetEvent;

pub fn schedule_interval<F>(interval: Duration, handler: F) where F: FnMut() {
    INIT.call_once(|| unsafe {
        TIMER_QUEUE = TimerQueue::new();
    });

}

static mut TIMER_QUEUE: TimerQueue = TimerQueue::default();
static INIT: Once = Once::new();

pub struct TimerQueue {
    handle: HANDLE
}

/// Windows Timer
pub struct Timer<'q, 'h> {
    queue: &'q TimerQueue,
    handle: HANDLE,
    callback: Box<MutWrapper<'h>>
}

struct CriticalSection<'e> {
    event: &'e mut ManualResetEvent
}

struct MutWrapper<'h> {
    f: Box<dyn FnMut() + 'h>,
    executing: ManualResetEvent,
    mark_deleted: RwLock<bool>
}

impl<'e> CriticalSection<'e> {
    fn new(r#ref: &'e mut ManualResetEvent) -> Self {
        r#ref.reset().unwrap();
        Self { event: r#ref }
    }
}

impl<'e> Drop for CriticalSection<'e> {
    fn drop(&mut self) {
        self.event.set().unwrap();
    }
}

impl<'h> MutWrapper<'h> {
    fn new<F>(handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> {
            f: Box::new(handler),
            executing: ManualResetEvent::new_init(true),
            mark_deleted: RwLock::new(false)
        }
    }
    fn call(&mut self) -> Result<()> {
        let is_deleted = self.mark_deleted.read().unwrap();
        if !*is_deleted {
            let _ = CriticalSection::new(&mut self.executing);
            (self.f)();
        }
        Ok(())
    }
}

impl TimerQueue {
    pub fn new() -> Self {
        unsafe { Self { handle: CreateTimerQueue().unwrap() } }
    }

    /// Schedule a timer, either a one-shot timer, or a periodical timer.
    pub fn schedule_timer<'q, 'h, F>(&'q mut self, due: Duration, period: Option<Duration>, hints: Option<CallbackHint>, handler: F) -> Result<Timer<'q, 'h>>
        where F: FnMut() + Send + 'h
    {
        let period = period.map(|d| d.as_millis() as u32).unwrap_or(0);
        let option = hints.map(|o| match o {
            CallbackHint::QuickFunction => WT_EXECUTEINPERSISTENTTHREAD,
            CallbackHint::SlowFunction => WT_EXECUTELONGFUNCTION,
        }).unwrap_or(WT_EXECUTEDEFAULT);
        let option = if period == 0 { option | WT_EXECUTEONLYONCE } else { option };
        let mut timer_handle = HANDLE::default();
        let callback = Box::new(MutWrapper::new(handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as *const c_void;

        let create_timer_queue_timer_result = unsafe {
            CreateTimerQueueTimer(&mut timer_handle, self.handle, Some(timer_callback), Some(callback_ref),
                                  due.as_millis() as u32, period, option).as_bool()
        };
        if create_timer_queue_timer_result {
            Ok(Timer::<'q,'h> { queue: self, handle: timer_handle, callback })
        } else {
            Err(waitable_objects::get_last_error())
        }
    }

    const fn default() -> Self {
        Self { handle: HANDLE(0) }
    }
}

extern "system" fn timer_callback(ctx: *mut c_void, _: BOOLEAN) {
    let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
    wrapper.call();
}

impl Default for TimerQueue {
    fn default() -> Self {
        TimerQueue::default()
    }
}

impl Drop for TimerQueue {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            assert!(unsafe { DeleteTimerQueue(self.handle).as_bool() });
            self.handle = HANDLE::default();
        }
    }
}

const EXPECTED_EXECUTION_TIME: Duration = Duration::from_secs(2);

impl<'q, 'h> Drop for Timer<'q, 'h> {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            let mut is_deleted = self.callback.mark_deleted.write().unwrap();
            *is_deleted = true;

            if !self.callback.executing.wait_one(EXPECTED_EXECUTION_TIME).unwrap() {
                println!("ERROR: Wait for execution timed out! Timer handler is being executed while timer is also being destroyed! Program aborts!");
                abort();
            }

            let finish = ManualResetEvent::new();
            assert!(unsafe { DeleteTimerQueueTimer(self.queue.handle, self.handle, finish.handle).as_bool() });
            self.handle = HANDLE::default();

            if !finish.wait_one(Duration::from_secs(5)).unwrap() {
                println!("WARNING: Timed out! Something went wrong with DeleteTimerQueueTimer.");
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        thread::sleep,
        time::Duration
    };
    use std::ffi::c_void;
    use super::{ MutWrapper, TimerQueue };

    #[test]
    fn test_singleshot(){
        let mut my_queue = TimerQueue::new();
        let mut called = 0;
        {
            let timer = my_queue.schedule_timer(Duration::from_millis(400), None, None, || called += 1).unwrap();
            sleep(Duration::from_secs(1));

            let callback_ref = timer.callback.as_ref() as *const MutWrapper as *const c_void;
            println!("Check = {:?}", callback_ref);
        }
        assert_eq!(called, 1);
    }

    #[test]
    fn test_period(){
        let mut my_queue = TimerQueue::new();
        let mut called = 0;
        let duration = Duration::from_millis(300);
        {
            let timer = my_queue.schedule_timer(duration, Some(duration), None, || called += 1).unwrap();
            sleep(Duration::from_secs(1));
        }
        assert_eq!(called, 3);
    }
}