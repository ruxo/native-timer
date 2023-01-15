use std::{
    fmt::{Display, Formatter},
    time::Duration
};
use crate::{ TimerQueue, Timer };

/// Scheduler hint about the callback function.
pub enum CallbackHint {
    /// The callback function execution is quick. The scheduler may use a common timer thread for executing the function.
    QuickFunction,

    /// The callback function execution takes time. The schedule may create a dedicated thread for the function.
    SlowFunction
}

#[derive(Debug)]
pub enum TimerError {
    OsError(isize, String)
}

pub type Result<T> = std::result::Result<T, TimerError>;

pub struct TimerHandle<'h>(Timer<'static, 'h>);

/// Schedule an interval task.
///
/// The function `interval` cannot be negative (it will be converted to `u32`). [`CallbackHint`] gives the OS scheduler some hint about the callback
/// behavior. For example, if the handler takes long time to finish, the caller should hint with [`CallbackHint::SlowFunction`]. If no hint is specified,
/// it's up to the scheduler to decide.
///
/// Following example doesn't give a hint.
///
/// ```rust
/// # use std::thread::sleep;
/// # use std::time::Duration;
/// # use native_timer::schedule_interval;
/// let mut count = 0;
/// let t = schedule_interval(Duration::from_millis(200), None, || count += 1);
/// sleep(Duration::from_millis(500));
/// drop(t);
/// assert_eq!(count, 2);
/// ```
pub fn schedule_interval<'h, F>(interval: Duration, hint: Option<CallbackHint>, handler: F) -> Result<TimerHandle<'h>> where F: FnMut() + Send + 'h {
    DEFAULT_QUEUE.schedule_timer(interval, interval, hint, handler).map(|t| TimerHandle(t))
}

/// Schedule an one-shot task.
///
/// The details of parameters are similar to [`schedule_interval`] function.
pub fn schedule_oneshot<'h, F>(due: Duration, hint: Option<CallbackHint>, handler: F) -> Result<TimerHandle<'h>> where F: FnMut() + Send + 'h {
    DEFAULT_QUEUE.schedule_timer(due, Duration::ZERO, hint, handler).map(|t| TimerHandle(t))
}

const DEFAULT_QUEUE: &TimerQueue = &TimerQueue::default();

impl Display for TimerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimerError::OsError(code, msg) => write!(f, "OS error {code}: {msg}")
        }
    }
}

impl std::error::Error for TimerError {}