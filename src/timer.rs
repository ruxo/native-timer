use std::{
    fmt::{Display, Formatter}, fmt, time::Duration
};
use crate::{ TimerQueue, Timer };

/// Scheduler hint about the callback function.
#[derive(Copy, Clone)]
pub enum CallbackHint {
    /// The callback function execution is quick. The scheduler may use a common timer thread for executing the function.
    QuickFunction,

    /// The callback function execution takes time. The schedule may create a dedicated thread for the function. The longest execution time
    /// expected should be supplied in the first parameter.
    SlowFunction(Duration)
}

#[derive(Debug)]
pub enum TimerError {
    OsError(isize, String),

    /// Meaning a sync object gets broken (or poisoned) due to panic!()
    SynchronizationBroken
}

pub type Result<T> = std::result::Result<T, TimerError>;

pub struct TimerHandle<'h>(Timer<'static, 'h>);

pub const DEFAULT_ACCEPTABLE_EXECUTION_TIME: Duration = Duration::from_secs(1);

//----------------------------- FUNCTIONS --------------------------------------


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
    TimerQueue::default().schedule_timer(interval, interval, hint, handler).map(|t| TimerHandle(t))
}

/// Schedule an one-shot task.
///
/// The details of parameters are similar to [`schedule_interval`] function.
pub fn schedule_oneshot<'h, F>(due: Duration, hint: Option<CallbackHint>, handler: F) -> Result<TimerHandle<'h>> where F: FnMut() + Send + 'h {
    TimerQueue::default().schedule_timer(due, Duration::ZERO, hint, handler).map(|t| TimerHandle(t))
}

impl Display for TimerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TimerError::OsError(code, msg) => write!(f, "OS error {code}: {msg}"),
            TimerError::SynchronizationBroken => write!(f, "A sync object is broken from a thread's panic!")
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for TimerError {
    fn from(_value: std::sync::PoisonError<T>) -> Self {
        Self::SynchronizationBroken
    }
}

impl std::error::Error for TimerError {}