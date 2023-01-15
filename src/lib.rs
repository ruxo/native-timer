mod windows;
mod timer;

#[cfg(windows)]
pub use crate::windows::{
    schedule_interval, schedule_oneshot,
    TimerQueue, Timer
};

pub use timer::*;