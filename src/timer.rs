use std::{
    fmt::{Display, Formatter}, fmt,
    time::Duration
};
use sync_wait_object::WaitObjectError;
use crate::{TimerQueue, Timer};

/// Scheduler hint about the callback function.
#[derive(Copy, Clone, Debug)]
pub enum CallbackHint {
    /// The callback function execution is quick. The scheduler may use a common timer thread for executing the function.
    QuickFunction,

    /// The callback function execution takes time. The schedule may create a dedicated thread for the function. The longest execution time
    /// expected should be supplied in the first parameter.
    SlowFunction(Duration)
}

#[derive(Debug)]
pub enum TimerError {
    /// An error code from OS API call with its meaning.
    OsError(isize, String),

    /// A sync object gets broken (or poisoned) due to panic!()
    SynchronizationBroken
}

pub type Result<T> = std::result::Result<T, TimerError>;

pub const DEFAULT_ACCEPTABLE_EXECUTION_TIME: Duration = Duration::from_secs(1);

//----------------------------- FUNCTIONS --------------------------------------
/// Schedule an interval task on the default [`TimerQueue`].
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
pub fn schedule_interval<'h, F>(interval: Duration, hint: Option<CallbackHint>, handler: F) -> Result<Timer<'h>> where F: FnMut() + Send + 'h {
    TimerQueue::default().schedule_timer(interval, interval, hint, handler)
}

/// Schedule an one-shot task on the default [`TimerQueue`].
///
/// The details of parameters are similar to [`schedule_interval`] function.
pub fn schedule_oneshot<'h, F>(due: Duration, hint: Option<CallbackHint>, handler: F) -> Result<Timer<'h>> where F: FnOnce() + Send + 'h {
    TimerQueue::default().schedule_oneshot(due, hint, handler)
}

/// Schedule an one-shot background task on the default [`TimerQueue`]. Note that, unlike other `schedule_*` functions, this `fire_oneshot`
/// requires a `'static` lifetime closure.
///
/// # Arguments
///
/// * `due`: Due time to execute the task
/// * `hint`: Behavior hint of `handler`, which impacts how the task will be scheduled
/// * `handler`: The task to be called back
///
/// returns: Result<(), [`TimerError`]>
///
/// # Examples
///
/// ```
/// # use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
/// # use std::thread;
/// # use std::time::Duration;
/// use native_timer::fire_oneshot;
///
/// let flag = Arc::new(AtomicBool::new(false));
/// let shared_flag = flag.clone();
/// fire_oneshot(Duration::from_millis(100), None, move || {
///     let _ = &shared_flag.store(true, Ordering::SeqCst);
/// }).unwrap();
/// thread::sleep(Duration::from_millis(200));
/// assert!(flag.load(Ordering::SeqCst));
/// ```
#[inline]
pub fn fire_oneshot<F>(due: Duration, hint: Option<CallbackHint>, handler: F) -> Result<()> where F: FnMut() + Send + 'static {
    TimerQueue::default().fire_oneshot(due, hint, handler)
}

// ----------------------------------------- IMPLEMENTATIONS ------------------------------------------
impl<'h> Drop for Timer<'h> {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            println!("WARNING: an error occurred during timer destruction. Memory might leak. Error = {e:?}");
        }
    }
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

impl From<WaitObjectError> for TimerError {
    fn from(value: WaitObjectError) -> Self {
        match value {
            WaitObjectError::OsError(v, s) => TimerError::OsError(v, s),
            WaitObjectError::SynchronizationBroken => TimerError::SynchronizationBroken,
            WaitObjectError::Timeout => TimerError::SynchronizationBroken
        }
    }
}

impl std::error::Error for TimerError {}