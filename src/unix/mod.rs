use std::{
    marker::PhantomData,
    time::Duration,
    sync::mpsc::{channel, Sender, Receiver},
    ffi::{c_void, CString},
    process, ptr, mem, thread, sync
};
use libc::{c_int, sigaction, sigevent, sigval, sigemptyset, siginfo_t, size_t, strerror, SIGRTMIN, SIGEV_THREAD_ID, syscall,
           SYS_gettid, timer_create,
           itimerspec, timespec, c_long, timer_settime, timer_t, timer_delete, CLOCK_MONOTONIC};
use crate::{
    CallbackHint, Result, TimerError,
    common::MutWrapper
};

mod waitable_objects;
pub(crate) use waitable_objects::ManualResetEvent;

// ----------------------------------------- DATA STRUCTURE --------------------------------------------------
pub struct TimerQueue {
    timer_queue: Sender<TimerCreationUnsafeRequest>,
    timer_receiver: Receiver<(usize, Result<usize>)>,
    quick_dispatcher: Sender<usize>
}

pub struct Timer<'q,'h> {
    handle: Option<timer_t>,
    callback: Box<MutWrapper<'q,'h>>,
    _life: PhantomData<&'q ()>
}

struct TimerCreationUnsafeRequest {
    due: Duration,
    period: Duration,
    callback_ref: usize
}

// ----------------------------------------- FUNCTIONS --------------------------------------------------
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

// ----------------------------------------- IMPLEMENTATIONS --------------------------------------------------
static DEFAULT_QUEUE_ONCE: sync::Once = sync::Once::new();
static mut DEFAULT_QUEUE: Option<TimerQueue> = None;

impl TimerQueue {
    /// Default OS common timer queue
    pub fn default() -> &'static TimerQueue {
        unsafe {
            DEFAULT_QUEUE_ONCE.call_once(|| {
                let (dispatcher, receiver) = channel::<TimerCreationUnsafeRequest>();
                let (result_sender, timer_receiver) = channel();
                thread::spawn(move || {
                    for req in receiver {
                        let timer = Self::create_timer(req.due, req.period, req.callback_ref);
                        result_sender.send((req.callback_ref, timer.map(|t| t as usize))).unwrap();
                    }
                });

                let (quick_dispatcher, quick_queue) = channel();
                thread::spawn(move || {
                    for ctx in quick_queue {
                        Self::unsafe_call(ctx);
                    }
                });
                DEFAULT_QUEUE = Some(TimerQueue { timer_queue: dispatcher, timer_receiver, quick_dispatcher });
            });
            DEFAULT_QUEUE.as_ref().unwrap()
        }
    }

    pub fn schedule_timer<'q,'h, F>(&'q self, due: Duration, period: Duration, hints: Option<CallbackHint>, handler: F) -> Result<Timer<'q,'h>>
        where F: FnMut() + Send + 'h
    {
        let callback = Box::new( MutWrapper::new(self, hints, handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as usize;
        let unsafe_request = TimerCreationUnsafeRequest { due, period, callback_ref };

        self.timer_queue.send(unsafe_request).unwrap();

        let (id, timer_unsafe) = self.timer_receiver.recv().unwrap();
        assert_eq!(id, callback_ref);

        timer_unsafe.map(|t| Timer::<'q,'h> {
            handle: Some(t as timer_t),
            callback,
            _life: PhantomData
        })
    }

    fn create_timer(due: Duration, period: Duration, callback_ref: usize) -> Result<timer_t>
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
            Ok(timer)
        }
    }

    fn unsafe_call(ctx: usize) {
        thread::spawn(move || {
            let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
            if let Err(e) = wrapper.call() {
                println!("WARNING: Error occurred during timer callback: {e:?}");
            }
        });
    }

    extern "C" fn timer_callback(_id: c_int, signal: *mut siginfo_t, _uc: *mut c_void){
        let ctx = unsafe { Self::get_ptr(signal) };
        let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
        match wrapper.hints {
            Some(CallbackHint::SlowFunction(_)) => Self::unsafe_call(ctx.try_into().unwrap()),
            _ => wrapper.main_queue.quick_dispatcher.send(ctx.try_into().unwrap()).unwrap()
        }
    }

    #[allow(deprecated)]
    unsafe extern "C" fn get_ptr(si: *mut siginfo_t) -> u64 {
        // TODO: Assume the running machine is x64 !
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
            let mut is_deleted = self.callback.mark_deleted.write().unwrap();
            *is_deleted = true;

            unsafe {
                if timer_delete(handle) < 0 {
                    let e = get_errno();
                    println!("WARNING: an error occurred during timer destruction. Memory might leak. Error = {e:?}");
                }
            }

            let acceptable_execution_time = match self.callback.hints {
                Some(CallbackHint::SlowFunction(d)) => d,
                _ => crate::DEFAULT_ACCEPTABLE_EXECUTION_TIME
            };
            if !self.callback.idling.wait_one(acceptable_execution_time).unwrap() {
                println!("ERROR: Wait for execution timed out! Timer handler is being executed while timer is also being destroyed! \
                Program aborts!");
                process::abort();
            }
        }
    }
}
