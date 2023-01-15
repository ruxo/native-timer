use std::fmt::{Display, Formatter};

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

impl Display for TimerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimerError::OsError(code, msg) => write!(f, "OS error {code}: {msg}")
        }
    }
}

impl std::error::Error for TimerError {}