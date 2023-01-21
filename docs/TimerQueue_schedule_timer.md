Schedule a timer, either a one-shot timer, or a periodical timer.

# Arguments

* `due`: Due time to execute the task
* `period`: Time interval to repeat the task
* `hint`: Behavior hint of `handler`, which impacts how the task will be scheduled
* `handler`: The task to be called back

returns: Result<[`Timer`], [`TimerError`]>

# Examples

## Single-shot timer

```rust
# use std::thread;
# use std::time::Duration;
use native_timer::TimerQueue;

let my_queue = TimerQueue::new();
let mut called = 0;
let timer = my_queue.schedule_timer(Duration::from_millis(400), Duration::ZERO,
                                    None, || called += 1).unwrap();
thread::sleep(Duration::from_secs(1));
drop(timer);
assert_eq!(called, 1);

```

## Periodical timer

```rust
# use std::thread;
# use std::time::Duration;
use native_timer::TimerQueue;

let my_queue = TimerQueue::new();
let mut called = 0;
let duration = Duration::from_millis(300);
let t = my_queue.schedule_timer(duration, duration, None, || called += 1).unwrap();
thread::sleep(Duration::from_secs(1));
drop(t);
assert_eq!(called, 3);
```