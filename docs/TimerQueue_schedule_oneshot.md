Schedule an one-shot timer. This is different from `schedule_timer` that this function's handler is `FnOnce`. Parameter
`period` is also not needed here.

# Arguments

* `due`: Due time to execute the task
* `hint`: Behavior hint of `handler`, which impacts how the task will be scheduled
* `handler`: The task to be called back

returns: Result<[`Timer`], [`TimerError`]>

# Examples

```rust
# use std::thread;
# use std::time::Duration;
use native_timer::TimerQueue;

let my_queue = TimerQueue::new();
let mut called = 0;
let timer = my_queue.schedule_oneshot(Duration::from_millis(400), None,
                                   || called += 1).unwrap();
thread::sleep(Duration::from_secs(1));
drop(timer);
assert_eq!(called, 1);

```