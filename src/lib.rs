#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use crate::windows::{ TimerQueue, Timer };

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use crate::unix::{ TimerQueue, Timer };

mod timer;
pub use timer::*;