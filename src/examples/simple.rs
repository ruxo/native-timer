use std::thread::sleep;
use std::time::Duration;
use native_timer::{CallbackHint, schedule_interval, schedule_oneshot};

fn main() {
    let t1 = schedule_interval(Duration::from_secs(1), None, || println!("Tick!")).unwrap();
    let t2 = schedule_interval(Duration::from_secs(1), Some(CallbackHint::QuickFunction), || println!("Tick fast!")).unwrap();
    let t3 = schedule_oneshot(Duration::from_millis(800), Some(CallbackHint::QuickFunction), || println!("One shot!")).unwrap();

    let mut lazy_count = 0;
    // let t4 = schedule_interval(Duration::from_secs(1), Some(CallbackHint::SlowFunction), || {
    //     lazy_count += 1;
    //     let my_id = lazy_count;
    //     println!("I'm lazy... #{my_id}");
    //     sleep(Duration::from_secs(3));
    //     println!("#{my_id} I may have been called with a dedicated thread");
    // }).unwrap();

    println!("Wait for timer working");
    sleep(Duration::from_secs(5));

    println!("Dropping timers");

    // Note that every dropping will block the thread because dropping timer needs to ensure the execution of timer function.
    drop(t1);
    drop(t2);
    drop(t3);
    // drop(t4);

    println!("That's it!");
}