Represents an instance of scheduled timer for a specific task.

`Timer` instance owns[^own] the space for the task's closure. So when the timer is out of scope, it is forced to stop and
destroy the closure during `Drop` processing.

[^own]: However, `TimerQueue::fire_onshot` does not create `Timer` instance. That's why the function need a `'static`
closure lifetime.