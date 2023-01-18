# Native Timer for Rust

Provide a timer functionality which uses OS capabilities. Currently supports
Windows, Linux, and MacOS.

v0.3.0 Both Windows and Linux platforms are supported.

However, Linux timers is limited to x64 machine because the code that passes a pointer from signal function, assume x64
architecture when retrieving the passing pointer. I'll need some research for better solution.

## Examples

See `/src/examples/simple.rs` for usage.

## Credit

Linux and MacOS implementations are recovered from [Autd3 open source library](https://github.com/sssssssuzuki/rust-autd)
on tag `v1.10.0`, which is before being removed by commit afterwards.