use std::{
    time, time::Duration,
    ops::Deref,
    sync::{Arc, Condvar, Mutex}
};
use crate::Result;

#[derive(Clone)]
pub struct ManualResetEvent(Arc<(Mutex<bool>, Condvar)>);

impl ManualResetEvent {
    #[inline] #[allow(dead_code)]
    pub fn new() -> Self { ManualResetEvent::new_init(false) }
    pub fn new_init(initial_state: bool) -> Self {
        Self(Arc::new((Mutex::new(initial_state), Condvar::new())))
    }

    #[inline] #[allow(dead_code)]
    pub fn wait_until_set(&self) -> Result<bool> {
        self.wait(Self::no_waiter())
    }

    #[inline] pub fn wait_one(&self, timeout: Duration) -> Result<bool> {
        self.wait(Self::waiter(timeout))
    }

    fn wait(&self, waiter: impl Fn() -> bool) -> Result<bool> {
        let (lock, cond) = self.0.deref();
        let mut state = lock.lock()?;
        while !*state && waiter() {
            state = cond.wait(state)?;
        }
        Ok(*state)
    }

    fn waiter(timeout: Duration) -> impl Fn() -> bool {
        let start = time::Instant::now();
        move || { (time::Instant::now() - start) < timeout }
    }
    fn no_waiter() -> impl Fn() -> bool { || true }

    #[inline]
    pub fn reset(&mut self) -> Result<()> {
        self.set_state(false)
    }

    #[inline]
    pub fn set(&mut self) -> Result<()> {
        self.set_state(true)
    }

    fn set_state(&mut self, new_state: bool) -> Result<()> {
        let (lock, cond) = self.0.deref();
        let mut state = lock.lock()?;
        *state = new_state;
        cond.notify_all();
        Ok(())
    }
}
