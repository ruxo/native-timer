use std::{
    marker::PhantomData, time::Duration, ffi::{c_void, CString}, ptr, mem, thread
};
use std::sync::{Arc, Condvar, Mutex, mpsc::{channel, Sender}};
use libc::{c_int, sigaction, sigevent, sigval, sigemptyset, siginfo_t, size_t, strerror, SIGRTMIN, SIGEV_THREAD_ID, syscall, SYS_gettid, timer_create,
           itimerspec, timespec, c_long, timer_settime, timer_t, timer_delete, CLOCK_MONOTONIC};
use crate::{ CallbackHint, Result, TimerError };

pub struct TimerQueue;

pub struct Timer<'q,'h> {
    handle: Option<timer_t>,
    wait: Option<Arc<(Mutex<bool>, Condvar)>>,
    _callback: Box<MutWrapper<'h>>,
    _life: PhantomData<&'q ()>
}

struct MutWrapper<'h> {
    f: Box<dyn FnMut() + 'h>
}

fn to_error(err_no: c_int) -> TimerError {
    assert_ne!(err_no, 0);
    let message = unsafe { CString::from_raw(strerror(err_no)) };
    TimerError::OsError(err_no as isize, message.into_string().unwrap())
}

#[inline] fn get_errno() -> TimerError {
    to_error(unsafe { *libc::__errno_location() })
}

fn to_result(ret: c_int) -> Result<()> {
    if ret == 0 { Ok(()) }
    else { Err(get_errno()) }
}

impl TimerQueue {
    /// Default OS common timer queue
    pub const fn default() -> Self { Self }

    pub fn schedule_timer<'q,'h, F>(&self, due: Duration, period: Duration, hints: Option<CallbackHint>, handler: F) -> Result<Timer<'q,'h>>
        where F: FnMut() + Send + 'h
    {
        let callback = Box::new(MutWrapper::new(handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as usize;

        let (sender, retriever) = channel();

        let wait = if let Some(CallbackHint::SlowFunction) = hints {
            let wait = Arc::new((Mutex::new(false), Condvar::new()));
            let thread_wait = Arc::clone(&wait);
            thread::spawn(move || {
                if let Err(e) = Self::create_timer(due, period, sender, callback_ref) {
                    println!("WARNING: Create timer error {e:?}");
                }
                let (lock, cvar) = &*thread_wait;
                let mut quited = lock.lock().unwrap();
                while !*quited {
                    quited = cvar.wait(quited).unwrap();
                }
            });
            Some(wait)
        } else {
            Self::create_timer(due, period, sender, callback_ref)?;
            None
        };

        let timer = retriever.recv().unwrap();

        timer.map(|t| Timer::<'q,'h> { handle: Some(t as timer_t), wait, _callback: callback, _life: PhantomData })
    }

    fn create_timer(due: Duration, period: Duration, sender: Sender<Result<usize>>, callback_ref: usize) -> Result<()>
    {
        unsafe {
            let mut sa_mask = mem::zeroed();
            to_result(sigemptyset(&mut sa_mask))?;
            let sa = sigaction {
                sa_flags: libc::SA_SIGINFO,
                sa_sigaction: Self::timer_callback as size_t,
                sa_mask,
                sa_restorer: None
            };
            to_result(sigaction(SIGRTMIN(), &sa, ptr::null_mut()))?;

            let mut sev: sigevent = mem::zeroed();
            sev.sigev_value = sigval {
                sival_ptr: callback_ref as *mut c_void
            };
            sev.sigev_signo = SIGRTMIN();
            sev.sigev_notify = SIGEV_THREAD_ID;
            sev.sigev_notify_thread_id = syscall(SYS_gettid) as i32;
            let mut timer = ptr::null_mut();
            to_result(timer_create(CLOCK_MONOTONIC, &mut sev, &mut timer))?;

            let interval = itimerspec {
                it_value: to_timespec(due),
                it_interval: to_timespec(period)
            };

            to_result(timer_settime(timer, 0, &interval, ptr::null_mut()))?;
            sender.send(Ok(timer as usize)).unwrap();
        }
        Ok(())
    }

    extern "C" fn timer_callback(_id: c_int, signal: *mut siginfo_t, _uc: *mut c_void){
        let ctx = unsafe { Self::get_ptr(signal) };
        let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
        if let Err(e) = wrapper.call() {
            println!("WARNING: Error occurred during timer callback: {e:?}");
        }
    }

    #[allow(deprecated)]
    unsafe extern "C" fn get_ptr(si: *mut siginfo_t) -> u64 {
        // TODO: This depends on the deprecated field of libc crate, and may only work on a specific platforms.
        let ptr_lsb = (*si)._pad[3];
        let ptr_msb = (*si)._pad[4];
        ((ptr_msb as u64) << 32) | (ptr_lsb as u64 & 0xFFFF_FFFF)
    }
}

fn to_timespec(value: Duration) -> timespec {
    let ns = value.as_nanos();
    let secs = ns / 1_000_000_000;
    let pure_ns = ns - (secs * 1_000_000_000);
    timespec { tv_sec: secs as c_long, tv_nsec: pure_ns as c_long }
}

impl<'q,'h> Drop for Timer<'q,'h> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if let Some(wait) = self.wait.take() {
                let (lock, cvar) = &*wait;
                let mut quited = lock.lock().unwrap();
                *quited = true;
                cvar.notify_one();
            }
            unsafe {
                if timer_delete(handle) < 0 {
                    let e = get_errno();
                    println!("WARNING: an error occurred during timer destruction. Memory might leak. Error = {e:?}");
                }
            }
        }
    }
}

impl<'h> MutWrapper<'h> {
    fn new<F>(handler: F) -> Self where F: FnMut() + Send + 'h {
        MutWrapper::<'h> { f: Box::new(handler) }
    }
    fn call(&mut self) -> Result<()> {
        (self.f)();
        Ok(())
    }
}