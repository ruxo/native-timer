use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;
use native_timer::TimerQueue;

fn main() {
    let queue = TimerQueue::new();
    let (qs, qr) = channel::<TimerQueue>();

    let flag = Arc::new(AtomicBool::new(false));
    let shared_flag = flag.clone();
    queue.fire_oneshot(Duration::from_millis(100), None, move || {
        println!("Just fired!");
        let _ = &shared_flag.store(true, Ordering::SeqCst);
        drop(qr.recv().unwrap());
        println!("Dropped!");
    }).unwrap();

    qs.send(queue).unwrap();
    sleep(Duration::from_millis(200));
    assert!(flag.load(Ordering::SeqCst));
}
