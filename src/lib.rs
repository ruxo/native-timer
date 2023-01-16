#[cfg(windows)]
mod windows;

#[cfg(windows)]
use crate::windows as platform;

#[cfg(unix)]
mod unix;

#[cfg(unix)]
use crate::unix as platform;

mod timer;

pub use platform::{ TimerQueue, Timer };
pub use timer::*;