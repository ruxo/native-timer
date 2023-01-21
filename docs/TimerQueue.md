`TimerQueue` manages scheduling of timers. [`Timer`] can be created from `TimerQueue` only.

Implementation of `TimerQueue` in Windows OS uses Win32 API 
[Timer Queues](https://learn.microsoft.com/en-us/windows/win32/sync/timer-queues).

In Unix platforms, `TimerQueue` use POSIX [`timer_create`](https://pubs.opengroup.org/onlinepubs/007904975/functions/timer_create.html)
API to schedule signal in a dedicated thread and dispatch task executions depending on the task's hint. That is, for any
quick function handler, it will be called from another common, dedicated thread. For the slow function handler, each
execution will be on its own thread.

`TimerQueue` has a default queue which can be used right away. But if you need to have another set of working threads,
you can use [`TimerQueue::new`] too.

# Example

```rust
# use std::thread::sleep;
# use std::time::Duration;
use native_timer::TimerQueue;

let mut count = 0;
let default_queue = TimerQueue::default();  // you can use TimerQueue::new() too.
let t = default_queue.schedule_timer(Duration::from_millis(100), Duration::default(), 
                                     None, || count += 1);
sleep(Duration::from_millis(200));
drop(t);
assert_eq!(count, 1);
```
