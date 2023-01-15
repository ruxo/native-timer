use std::{
    time::Duration,
    sync::{ Once },
    ffi::c_void
};
use windows::Win32::{
    Foundation::{HANDLE, BOOLEAN},
    System::Threading::*,
};
use windows::Win32::Foundation::GetLastError;
use crate::timer::{CallbackHint, Result, TimerError};

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

pub struct Timer<'q, 'h> {
    queue: &'q TimerQueue,
    handle: HANDLE,
    callback: Box<MutWrapper<'h>>
}

struct MutWrapper<'h> {
    f: Box<dyn FnMut() + 'h>
}

impl<'h> MutWrapper<'h> {
    fn new<F>(handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> { f: Box::new(handler) }
    }
    fn call(&mut self){
        (self.f)();
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
            Ok(Timer::<'q,'h> { queue: self, handle: HANDLE::default(), callback })
        } else {
            let info = unsafe { GetLastError() };
            Err(TimerError::OsError(info.0 as isize, info.to_hresult().message().to_string()))
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

impl<'q, 'h> Drop for Timer<'q, 'h> {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            assert!(unsafe { DeleteTimerQueueTimer(self.queue.handle, self.handle, None).as_bool() });
            self.handle = HANDLE::default();
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        thread::sleep,
        time::Duration
    };
    use super::TimerQueue;

    #[test]
    fn test_singleshot(){
        let mut my_queue = TimerQueue::new();
        let mut called = 0;
        {
            my_queue.schedule_timer(Duration::from_millis(400), None, None, || called += 1).unwrap();
            sleep(Duration::from_secs(1));
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