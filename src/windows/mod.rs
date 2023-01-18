use std::{
    time::Duration,
    sync::RwLock,
    ffi::c_void,
    process::abort
};
use windows::Win32::{
    Foundation::{HANDLE, BOOLEAN, ERROR_IO_PENDING},
    System::Threading::*,
};
use super::timer::{CallbackHint, Result, DEFAULT_ACCEPTABLE_EXECUTION_TIME};
use waitable_objects::{get_last_error, get_win32_last_error, to_result};

mod waitable_objects;
pub(crate) use waitable_objects::ManualResetEvent;

// ------------------ DATA STRUCTURE -------------------------------
/// Wrapper of Windows Timer Queue API
///
/// Windows OS provides a default Timer Queue which can be retrieved by using [`TimerQueue::default`]. The default queue has `'static` lifetime, and can
/// be assigned as a constant.
///
/// ```rust
/// # use std::thread::sleep;
/// # use std::time::Duration;
/// # use native_timer::TimerQueue;
/// let mut count = 0;
/// let default_queue = TimerQueue::default();
/// let t = default_queue.schedule_timer(Duration::from_millis(100), Duration::default(), None, || count += 1);
/// sleep(Duration::from_millis(200));
/// drop(t);
/// assert_eq!(count, 1);
/// ```
pub struct TimerQueue {
    handle: HANDLE
}

/// Windows Timer
pub struct Timer<'q, 'h> {
    queue: &'q TimerQueue,
    handle: HANDLE,
    callback: Box<MutWrapper<'h>>,
    acceptable_execution_time: Duration
}

// ------------------ IMPLEMENTATIONS -------------------------------
static DEFAULT_QUEUE: TimerQueue = TimerQueue { handle: HANDLE(0) };

impl TimerQueue {
    pub fn new() -> Self {
        unsafe { Self { handle: CreateTimerQueue().unwrap() } }
    }

    /// Schedule a timer, either a one-shot timer, or a periodical timer.
    pub fn schedule_timer<'q, 'h, F>(&'q self, due: Duration, period: Duration, hints: Option<CallbackHint>, handler: F) -> Result<Timer<'q, 'h>>
        where F: FnMut() + Send + 'h
    {
        let period = period.as_millis() as u32;
        let (option, acceptable_execution_time) = hints.map(|o| match o {
            CallbackHint::QuickFunction => (WT_EXECUTEINPERSISTENTTHREAD, DEFAULT_ACCEPTABLE_EXECUTION_TIME),
            CallbackHint::SlowFunction(t) => (WT_EXECUTELONGFUNCTION, t)
        }).unwrap_or((WT_EXECUTEDEFAULT, DEFAULT_ACCEPTABLE_EXECUTION_TIME));
        let option = if period == 0 { option | WT_EXECUTEONLYONCE } else { option };

        let mut timer_handle = HANDLE::default();
        let callback = Box::new(MutWrapper::new(hints, handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as *const c_void;

        let create_timer_queue_timer_result = unsafe {
            CreateTimerQueueTimer(&mut timer_handle, self.handle, Some(timer_callback), Some(callback_ref),
                                  due.as_millis() as u32, period, option).as_bool()
        };
        if create_timer_queue_timer_result {
            Ok(Timer::<'q,'h> { queue: self, handle: timer_handle, callback, acceptable_execution_time })
        } else {
            Err(get_last_error())
        }
    }

    /// Default OS common timer queue
    #[inline]
    pub fn default() -> &'static TimerQueue {
        &DEFAULT_QUEUE
    }
}

extern "system" fn timer_callback(ctx: *mut c_void, _: BOOLEAN) {
    let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
    if let Err(e) = wrapper.call() {
        println!("WARNING: Error occurred during timer callback: {e:?}");
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

impl<'q,'h> Timer<'q,'h> {
    pub fn change_period(&self, due: Duration, period: Duration) -> Result<()> {
        to_result(unsafe { ChangeTimerQueueTimer(self.queue.handle, self.handle, due.as_millis() as u32, period.as_millis() as u32).as_bool() })
    }
}

impl<'q, 'h> Drop for Timer<'q, 'h> {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            let mut is_deleted = self.callback.mark_deleted.write().unwrap();
            *is_deleted = true;

            // ensure no callback during destruction
            self.change_period(Duration::default(), Duration::default()).unwrap();

            if !self.callback.idling.wait_one(self.acceptable_execution_time).unwrap() {
                println!("ERROR: Wait for execution timed out! Timer handler is being executed while timer is also being destroyed! Program aborts!");
                abort();
            }

            let result = unsafe { DeleteTimerQueueTimer(self.queue.handle, self.handle, None).as_bool() };

            if !result {
                let e = get_win32_last_error();
                if e != ERROR_IO_PENDING {
                    println!("WARNING: Delete timer failed with error {e:?}. No retry attempt. Memory might leak!");
                }
            }
            self.handle = HANDLE::default();
        }
    }
}

// ------------------ TESTS -------------------------------
#[cfg(test)]
mod test {
    use std::{
        thread::sleep,
        time::Duration
    };
    use super::TimerQueue;

    #[test]
    fn test_singleshot(){
        let my_queue = TimerQueue::new();
        let mut called = 0;
        let timer = my_queue.schedule_timer(Duration::from_millis(400), Duration::ZERO, None, || called += 1).unwrap();
        sleep(Duration::from_secs(1));
        drop(timer);
        assert_eq!(called, 1);
    }

    #[test]
    fn test_period(){
        let my_queue = TimerQueue::new();
        let mut called = 0;
        let duration = Duration::from_millis(300);
        let t = my_queue.schedule_timer(duration, duration, None, || called += 1).unwrap();
        sleep(Duration::from_secs(1));
        drop(t);
        assert_eq!(called, 3);
    }
}