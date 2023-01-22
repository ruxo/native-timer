Schedule an one-shot of the task. Note that this `fire_oneshot` requires a `'static` lifetime closure.

# Arguments 

* `due`: Due time to execute the task
* `hint`: Behavior hint of `handler`, which impacts how the task will be scheduled
* `handler`: The task to be called back

returns: Result<(), [`TimerError`]> 

# Examples 

This example creates a new `TimerQueue` to show that the lifetime of the queue is irrelevant. But in actual usage, you
can simply use [`TimerQueue::default`].

```rust
# use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
# use std::thread;
# use std::time::Duration;
use native_timer::TimerQueue;

let flag = Arc::new(AtomicBool::new(false));
let flag_writer = flag.clone();
TimerQueue::new().fire_oneshot(Duration::from_millis(100), 
                                   None, move || flag_writer.store(true, Ordering::SeqCst)).unwrap();
thread::sleep(Duration::from_millis(200));
assert!(flag.load(Ordering::SeqCst));
```
