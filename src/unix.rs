use std::{
    time::Duration,
    sync::mpsc::{channel, Sender},
    ffi::{c_void, CString},
    process, ptr, mem, thread, sync
};
use libc::{c_int, sigaction, sigevent, sigval, sigemptyset, siginfo_t, size_t, strerror, SIGRTMIN, SIGEV_THREAD_ID,
           syscall, SYS_gettid, timer_create, itimerspec, timespec, c_long, timer_settime, timer_t, timer_delete, CLOCK_REALTIME};
use sync_wait_object::WaitEvent;
use crate::{
    CallbackHint, Result, TimerError,
    common::MutWrapper
};

// ------------------------------------- DATA STRUCTURE & MARKERS -------------------------------------
pub struct TimerQueueCore {
    timer_queue: Sender<TimerCreationUnsafeRequest>,
    quick_dispatcher: Sender<MutWrapperUnsafeRepr>
}

pub struct TimerQueue(sync::Arc<TimerQueueCore>);

pub struct Timer<'h> {
    handle: Option<timer_t>,
    callback: Box<MutWrapper<'h>>
}

type MutWrapperUnsafeRepr = usize;
type TimerHandleUnsafeRepr = usize;
type TimerHandleResult = Result<TimerHandleUnsafeRepr>;

struct TimerCreationUnsafeRequest {
    due: Duration,
    period: Duration,
    callback_ref: MutWrapperUnsafeRepr,
    signal: Sender<TimerHandleResult>
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

fn close_timer(handle: timer_t, callback: &MutWrapper){
    let acceptable_execution_time = match callback.hints {
        Some(CallbackHint::SlowFunction(d)) => d,
        _ => crate::DEFAULT_ACCEPTABLE_EXECUTION_TIME
    };
    match callback.mark_deleted.try_write_for(acceptable_execution_time) {
        None => {
            println!("ERROR: Wait for execution timed out! Timer handler is being executed while timer is also being destroyed! Program aborts!");
            process::abort();
        },
        Some(mut is_deleted) => {
            *is_deleted = true;
        }
    }
    unsafe {
        if timer_delete(handle) < 0 {
            let e = get_errno();
            println!("WARNING: an error occurred during timer destruction. Memory might leak. Error = {e:?}");
        }
    }

}

// ----------------------------------------- IMPLEMENTATIONS --------------------------------------------------
static DEFAULT_QUEUE_ONCE: sync::Once = sync::Once::new();
static mut DEFAULT_QUEUE: Option<TimerQueue> = None;

impl TimerQueue {
    /// Create a new TimerQueue
    pub fn new() -> Self {
        let (dispatcher, receiver) = channel::<TimerCreationUnsafeRequest>();
        thread::spawn(move || {
            for req in receiver {
                let timer = Self::schedule_signal_callback(req.due, req.period, req.callback_ref);
                let message = timer.map(|t| t as TimerHandleUnsafeRepr);
                req.signal.send(message).unwrap();
            }
        });

        let (quick_dispatcher, quick_queue) = channel();
        thread::spawn(move || {
            for ctx in quick_queue {
                Self::unsafe_call(ctx);
            }
        });
        TimerQueue(sync::Arc::new(TimerQueueCore{
            timer_queue: dispatcher, quick_dispatcher
        }))
    }

    /// Default OS common timer queue
    pub fn default() -> &'static TimerQueue {
        unsafe {
            DEFAULT_QUEUE_ONCE.call_once(|| {
                println!("Call once!");
                DEFAULT_QUEUE = Some(Self::new());
            });
            println!("Return ref!");
            DEFAULT_QUEUE.as_ref().unwrap()
        }
    }

    pub fn schedule_timer<'h, F>(&self, due: Duration, period: Duration, hint: Option<CallbackHint>, handler: F) -> Result<Timer<'h>>
        where F: FnMut() + Send + 'h
    {
        let (timer_unsafe, callback) = self.create_timer(due, period, hint, handler)?;

        timer_unsafe.map(|t| Timer::<'h> {
            handle: Some(t as timer_t),
            callback
        })
    }

    pub fn fire_oneshot<F>(&self, due: Duration, hint: Option<CallbackHint>, mut handler: F) -> Result<()>
    where F: FnMut() + Send + 'static
    {
        let journal: WaitEvent<Option<(TimerHandleUnsafeRepr, MutWrapperUnsafeRepr)>> = WaitEvent::new_init(None);
        let mut journal_write = journal.clone();
        let wrapper = move || {
            handler();
            let (handle, callback_ptr) = journal.wait_reset(None, || None, |v| v.is_some()).unwrap().unwrap();

            // TODO use a common thread to clean up?
            thread::spawn(move || {
                let callback = unsafe { Box::from_raw(callback_ptr as *mut MutWrapper) };
                close_timer(handle as timer_t, &callback);
            });
        };

        let (timer_unsafe, callback) = self.create_timer(due, Duration::ZERO, hint, wrapper)?;

        let callback_ptr = Box::into_raw(callback) as MutWrapperUnsafeRepr;

        match timer_unsafe {
            Ok(handle) => journal_write.set_state(Some((handle, callback_ptr))).map_err(|e| e.into()),
            Err(e) => Err(e.into())
        }
    }

    #[inline]
    pub(crate) fn new_with_context(context: sync::Arc<TimerQueueCore>) -> Self {
        TimerQueue(context)
    }

    fn dispatch_quick_call(&self, ctx: MutWrapperUnsafeRepr) -> Result<()> {
        self.0.quick_dispatcher.send(ctx).map_err(|_| TimerError::SynchronizationBroken)
    }

    fn create_timer<'h, F>(&self, due: Duration, period: Duration, hint: Option<CallbackHint>, handler: F)
                              -> Result<(TimerHandleResult, Box<MutWrapper<'h>>)>
        where F: FnMut() + Send + 'h
    {
        let callback = Box::new( MutWrapper::<'h>::new(self.0.clone(), hint, handler));
        let callback_ref = callback.as_ref() as *const MutWrapper as MutWrapperUnsafeRepr;
        let (signal, timer_receiver) = channel();
        let unsafe_request = TimerCreationUnsafeRequest { due, period, callback_ref, signal };

        self.0.timer_queue.send(unsafe_request).unwrap();

        match timer_receiver.recv() {
            Err(_) => Err(TimerError::SynchronizationBroken),
            Ok(thm) => Ok((thm, callback))
        }
    }

    fn schedule_signal_callback(due: Duration, period: Duration, callback_ref: MutWrapperUnsafeRepr) -> Result<timer_t>
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
            to_result(timer_create(CLOCK_REALTIME, &mut sev, &mut timer))?;

            let interval = itimerspec {
                it_value: to_timespec(due),
                it_interval: to_timespec(period)
            };

            to_result(timer_settime(timer, 0, &interval, ptr::null_mut()))?;
            Ok(timer)
        }
    }

    fn unsafe_call(ctx: MutWrapperUnsafeRepr) {
        let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
        if let Err(e) = wrapper.call() {
            println!("WARNING: Error occurred during timer callback: {e:?}");
        }
    }

    extern "C" fn timer_callback(_id: c_int, signal: *mut siginfo_t, _uc: *mut c_void){
        let ctx = unsafe { (*signal).si_value().sival_ptr as MutWrapperUnsafeRepr };

        let wrapper = unsafe { &mut *(ctx as *mut MutWrapper) };
        match wrapper.hints {
            Some(CallbackHint::SlowFunction(_)) => { thread::spawn(move || { Self::unsafe_call(ctx) }); },
            _ => wrapper.timer_queue().dispatch_quick_call(ctx).unwrap()
        }
    }
}

fn to_timespec(value: Duration) -> timespec {
    let ns = value.as_nanos();
    let secs = ns / 1_000_000_000;
    let pure_ns = ns - (secs * 1_000_000_000);
    timespec { tv_sec: secs as c_long, tv_nsec: pure_ns as c_long }
}

impl<'h> Drop for Timer<'h> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            close_timer(handle, &self.callback);
        }
    }
}
#[cfg(test)]
mod test {
    use std::{
        thread::sleep,
        time::Duration
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use super::TimerQueue;

    #[test]
    fn test_singleshot(){
        let my_queue = TimerQueue::new();
        let mut called = 0;
        let timer = my_queue.schedule_timer(Duration::from_millis(400), Duration::ZERO, None, || called += 1).unwrap();
        sleep(Duration::from_secs(1));
        drop(timer);
        assert_eq!(called, 1);
    }

    #[test]
    fn test_period(){
        let my_queue = TimerQueue::new();
        let mut called = 0;
        let duration = Duration::from_millis(300);
        let t = my_queue.schedule_timer(duration, duration, None, || called += 1).unwrap();
        sleep(Duration::from_secs(1));
        drop(t);
        assert_eq!(called, 3);
    }

    #[test]
    fn test_oneshot() {
        let flag = Arc::new(AtomicBool::new(false));
        let shared_flag = flag.clone();
        TimerQueue::default().fire_oneshot(Duration::from_millis(100), None, move || shared_flag.store(true, Ordering::SeqCst)).unwrap();
        sleep(Duration::from_millis(200));
        assert!(flag.load(Ordering::SeqCst));
    }
}
