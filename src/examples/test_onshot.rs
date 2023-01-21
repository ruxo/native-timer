use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use native_timer::TimerQueue;

fn main() {
    let flag = Arc::new(AtomicBool::new(false));
    let shared_flag = flag.clone();
    TimerQueue::new().fire_oneshot(Duration::from_millis(100), None, move || {
        let _ = &shared_flag.store(true, Ordering::SeqCst);
    }).unwrap();
    sleep(Duration::from_millis(200));
    assert!(flag.load(Ordering::SeqCst));
}
