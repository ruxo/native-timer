use std::{
    time::Duration,
    sync::Once,
};
use windows::Win32::{
    Foundation::HANDLE,
    System::Threading::*,
};

pub fn schedule_interval<F>(interval: Duration, handler: F) where F: FnMut() {
    INIT.call_once(|| unsafe {
        TIMER_QUEUE = TimerQueue::new();
    });

}

static mut TIMER_QUEUE: TimerQueue = TimerQueue::default();
static INIT: Once = Once::new();

struct TimerQueue {
    handle: HANDLE
}

impl TimerQueue {
    fn new() -> Self {
        unsafe { Self { handle: CreateTimerQueue().unwrap() } }
    }
    const fn default() -> Self {
        Self { handle: HANDLE(0) }
    }
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