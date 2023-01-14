use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum TimerError {
    OsError(isize, String)
}

impl Display for TimerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimerError::OsError(code, msg) => write!(f, "OS error {code}: {msg}")
        }
    }
}

impl std::error::Error for TimerError {}