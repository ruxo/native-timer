### 0.3.3
- Split wait object code into `sync-wait-object` library.

### 0.3.2
- Unix: Remove thread spawning for Fast function timers since it should just use the common, single quick thread.

### 0.3.1
- Unix's TimerQueue now has `new()` function.

### 0.3.0

- Minor document fixes
- Break `TimerQueue::default()` signature compatibility. Now it returns a reference.
- Break `CallerHint::SlowFunction` signature to accept an "acceptable execution time", time that the handler is 
  expected to complete during the timer's destruction.
- Linux version now has dedicated threads for timer, and quick function dispatcher. For slow function dispatch, queue
  will spawn a new thread for every slow function.

### 0.2.0

- Linux platform is supported.

  However, Linux timers currently use the caller thread to execute the timer handler. So if the handler takes long execution time, it will
  impact the schedule for shared timers. However, a timer with `CallbackHint::SlowFunction` will have its own dedicated thread per timer
  instance, but it's still subjected to the time spent by the handler. _Next version_ will try to dispatch call to the slow function by
  separated threads.

### 0.1.0
- Only Windows platform is supported