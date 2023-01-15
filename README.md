# Native Timer for Rust

Provide a timer functionality which uses OS capabilities. Currently supports
Windows, Linux, and MacOS.

v0.2.0 Both Windows and Linux platforms are supported

However, Linux timers currently use the caller thread to execute the timer handler. So if the handler takes long execution time, it will
impact the schedule for shared timers. However, a timer with `CallbackHint::SlowFunction` will have its own dedicated thread per timer
instance, but it's still subjected to the time spent by the handler. _Next version_ will try to dispatch call to the slow function by 
separated threads.

## Examples

See `/src/examples/simple.rs` for usage.

## Credit

Linux and MacOS implementations are recovered from [Autd3 open source library](https://github.com/sssssssuzuki/rust-autd)
on tag `v1.10.0`, which is before being removed by commit afterwards.