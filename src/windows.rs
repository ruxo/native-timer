use std::{
    sync,
    time::Duration,
    ffi::c_void
};
use sync_wait_object::WaitEvent;
use windows::Win32::{
    Foundation::{HANDLE, BOOLEAN, ERROR_IO_PENDING, WIN32_ERROR, GetLastError},
    System::Threading::*,
};
use super::timer::{CallbackHint, Result, DEFAULT_ACCEPTABLE_EXECUTION_TIME};
use crate::common::MutWrapper;
use super::TimerError;

// ------------------ DATA STRUCTURE -------------------------------
pub struct TimerQueueCore(HANDLE);

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
pub struct TimerQueue(sync::Arc<TimerQueueCore>);

/// Windows Timer
pub struct Timer<'h> {
    queue: sync::Arc<TimerQueueCore>,
    handle: HANDLE,
    callback: Box<MutWrapper<'h>>,
    acceptable_execution_time: Duration
}

// ----------------------------------------- FUNCTIONS ------------------------------------------------
#[inline]
pub(crate) fn get_win32_last_error() -> WIN32_ERROR {
    unsafe { GetLastError() }
}

#[inline]
pub(crate) fn get_last_error() -> TimerError {
    get_win32_last_error().into()
}

pub(crate) fn to_result(ret: bool) -> Result<()> {
    if ret { Ok(()) }
    else { Err(get_last_error()) }
}

fn change_period(queue: HANDLE, timer: HANDLE, due: Duration, period: Duration) -> Result<()> {
    to_result(unsafe { ChangeTimerQueueTimer(queue, timer, due.as_millis() as u32, period.as_millis() as u32).as_bool() })
}

fn close_timer(queue: HANDLE, handle: HANDLE, acceptable_execution_time: Duration, callback: &MutWrapper) {
    callback.mark_delete(acceptable_execution_time);

    // ensure no callback during destruction
    change_period(queue, handle, Duration::default(), Duration::default()).unwrap();

    let result = unsafe { DeleteTimerQueueTimer(queue, handle, None).as_bool() };

    if !result {
        let e = get_win32_last_error();
        if e != ERROR_IO_PENDING {
            println!("WARNING: Delete timer failed with error {e:?}. No retry attempt. Memory might leak!");
        }
    }
}

static DEFAULT_QUEUE_INIT: sync::Once = sync::Once::new();
static mut DEFAULT_QUEUE: Option<TimerQueue> = None;

fn get_acceptable_execution_time(hint: Option<CallbackHint>) -> Duration {
    hint.map(|o| match o {
        CallbackHint::QuickFunction => DEFAULT_ACCEPTABLE_EXECUTION_TIME,
        CallbackHint::SlowFunction(t) => t
    }).unwrap_or(DEFAULT_ACCEPTABLE_EXECUTION_TIME)
}

// -------------------------------------- IMPLEMENTATIONS --------------------------------------------
impl TimerQueue {
    /// Default OS common timer queue
    #[inline]
    pub fn default() -> &'static TimerQueue {
        DEFAULT_QUEUE_INIT.call_once(|| unsafe {
            DEFAULT_QUEUE = Some(TimerQueue(sync::Arc::new(TimerQueueCore(HANDLE(0)))));
        });
        unsafe { DEFAULT_QUEUE.as_ref().unwrap() }
    }

    /// Create a new TimerQueue
    pub fn new() -> Self {
        let core = unsafe {  CreateTimerQueue().unwrap() };
        TimerQueue(sync::Arc::new(TimerQueueCore(core)))
    }

    /// Schedule a timer, either a one-shot timer, or a periodical timer.
    pub fn schedule_timer<'h, F>(&self, due: Duration, period: Duration, hint: Option<CallbackHint>, handler: F) -> Result<Timer<'h>>
        where F: FnMut() + Send + 'h
    {
        let acceptable_execution_time = get_acceptable_execution_time(hint);
        let (timer_handle, callback) = self.create_timer(due, period, hint, handler)?;
        Ok(Timer::<'h> { queue: self.0.clone(), handle: timer_handle, callback, acceptable_execution_time })
    }

    pub fn fire_oneshot<F>(&self, due: Duration, hint: Option<CallbackHint>, mut handler: F) -> Result<()> where F: FnMut() + Send {
        let journal: WaitEvent<Option<(HANDLE, usize)>> = WaitEvent::new_init(None);
        let mut journal_write = journal.clone();
        let queue_handle = self.0.0;

        let acceptable_execution_time = get_acceptable_execution_time(hint);

        let wrapper = move || {
            handler();
            let (handle, callback_ptr) = journal.wait_reset(None, || None, |v| v.is_some()).unwrap().unwrap();

            // Need to exit the timer thread before cleaning up the handle.
            // TODO use a common thread?
            std::thread::spawn(move || {
                let callback = unsafe { Box::from_raw(callback_ptr as *mut MutWrapper) };
                close_timer(queue_handle, handle, acceptable_execution_time, &callback);
            });
        };
        let (timer_handle, callback) = self.create_timer(due, Duration::ZERO, hint, wrapper)?;
        let callback_ptr = Box::into_raw(callback) as usize;
        journal_write.set_state(Some((timer_handle, callback_ptr))).map_err(|e| e.into())
    }

    #[allow(dead_code)]
    pub(crate) fn new_with_context(context: sync::Arc<TimerQueueCore>) -> Self {
        TimerQueue(context)
    }

    fn create_timer<'h, F>(&self, due: Duration, period: Duration, hint: Option<CallbackHint>, handler: F) -> Result<(HANDLE, Box<MutWrapper<'h>>)>
        where F: FnMut() + Send + 'h
    {
        let period = period.as_millis() as u32;
        let option = hint.map(|o| match o {
            CallbackHint::QuickFunction => WT_EXECUTEINPERSISTENTTHREAD,
            CallbackHint::SlowFunction(_) => WT_EXECUTELONGFUNCTION
        }).unwrap_or(WT_EXECUTEDEFAULT);
        let option = if period == 0 { option | WT_EXECUTEONLYONCE } else { option };

        let mut timer_handle = HANDLE::default();
        let callback = Box::new(MutWrapper::new(self.0.clone(), hint, handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as *const c_void;

        let create_timer_queue_timer_result = unsafe {
            CreateTimerQueueTimer(&mut timer_handle, self.0.0, Some(timer_callback), Some(callback_ref),
                                  due.as_millis() as u32, period, option).as_bool()
        };
        if create_timer_queue_timer_result {
            Ok((timer_handle, callback))
        } else {
            Err(get_last_error())
        }
    }
}

extern "system" fn timer_callback(ctx: *mut c_void, _: BOOLEAN) {
    let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
    if let Err(e) = wrapper.call() {
        println!("WARNING: Error occurred during timer callback: {e:?}");
    }
}

impl Drop for TimerQueueCore {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            assert!(unsafe { DeleteTimerQueue(self.0).as_bool() });
            self.0 = HANDLE::default();
        }
    }
}

impl<'h> Timer<'h> {
    pub fn change_period(&self, due: Duration, period: Duration) -> Result<()> {
        change_period(self.queue.0, self.handle, due, period)
    }
}

impl<'h> Drop for Timer<'h> {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            close_timer(self.queue.0, self.handle, self.acceptable_execution_time, &self.callback);
            self.handle = HANDLE::default();
        }
    }
}

impl From<WIN32_ERROR> for TimerError {
    fn from(value: WIN32_ERROR) -> Self {
        TimerError::OsError(value.0 as isize, value.to_hresult().message().to_string())
    }
}

// ------------------ TESTS -------------------------------
#[cfg(test)]
mod test {
    use std::{
        thread::sleep,
        time::Duration
    };
    use std::sync::atomic::{AtomicBool, Ordering};
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

    #[test]
    fn test_oneshot() {
        let flag = AtomicBool::new(false);
        TimerQueue::default().fire_oneshot(Duration::from_millis(100), None, || flag.store(true, Ordering::SeqCst)).unwrap();
        sleep(Duration::from_millis(200));
        assert!(flag.load(Ordering::SeqCst));
    }
}