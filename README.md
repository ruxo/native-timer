# Native Timer for Rust

Provide a timer functionality which uses OS capabilities. Currently supports
Windows, Linux, and MacOS.

Currently, only both Windows and Linux platforms are supported.

## Features

* `tracker` (default) - Enable static callback tracker. It should minimize the native callback into an invalid timer
  context, that has been recently destroyed.

## Examples

To fire an one-shot task:

```rust
# use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
# use std::thread;
# use std::time::Duration;
use native_timer::fire_oneshot;

let flag = Arc::new(AtomicBool::new(false));
let shared_flag = flag.clone();
fire_oneshot(Duration::from_millis(100), None, move || {
    let _ = &shared_flag.store(true, Ordering::SeqCst);
}).unwrap();
thread::sleep(Duration::from_millis(200));
assert!(flag.load(Ordering::SeqCst));
```

For more usages, see `/src/examples/simple.rs`.

## Credit

Linux and MacOS implementations are recovered from [Autd3 open source library](https://github.com/sssssssuzuki/rust-autd)
on tag `v1.10.0`, which is before being removed by commits afterwards.