use std::{
    time::Duration
};
use windows::Win32::{
    Foundation::{ HANDLE, CloseHandle, GetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT, WAIT_FAILED },
    System::Threading::{ CreateEventA, WaitForSingleObject, ResetEvent, SetEvent },
    System::WindowsProgramming::INFINITE
};
use windows::Win32::Foundation::WIN32_ERROR;
use crate::timer::{ TimerError, Result };

pub struct ManualResetEvent {
    pub(crate) handle: HANDLE
}

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

impl From<WIN32_ERROR> for TimerError {
    fn from(value: WIN32_ERROR) -> Self {
        TimerError::OsError(value.0 as isize, value.to_hresult().message().to_string())
    }
}

impl ManualResetEvent {
    #[inline] #[allow(dead_code)]
    pub fn new() -> Self { ManualResetEvent::new_init(false) }
    pub fn new_init(initial_state: bool) -> Self {
        let handle = unsafe { CreateEventA(None, true, initial_state, None).unwrap() };
        Self { handle }
    }

    #[inline] #[allow(dead_code)]
    pub fn wait_until_set(&self) -> Result<bool> {
        self.wait(INFINITE)
    }

    #[inline] pub fn wait_one(&self, timeout: Duration) -> Result<bool> {
        self.wait(timeout.as_millis() as u32)
    }

    fn wait(&self, timeout: u32) -> Result<bool> {
        let ret = unsafe { WaitForSingleObject(self.handle, timeout) };
        match ret {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            WAIT_FAILED => Err(get_last_error()),
            _ => unreachable!()
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        to_result(unsafe { ResetEvent(self.handle).as_bool() })
    }
    pub fn set(&mut self) -> Result<()> {
        to_result(unsafe { SetEvent(self.handle).as_bool() })
    }

}

impl Drop for ManualResetEvent {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe { CloseHandle(self.handle); }
            self.handle = HANDLE::default();
        }
    }
}