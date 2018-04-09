# hangups-rs

Google Hangouts client library for any language (prototype)

## About

This is a client library for [Google Hangouts](https://hangouts.google.com/)
instant messaging. It's written in [Rust](https://www.rust-lang.org), but
provides a C-style API that can be consumed by any programming language with a
C foreign function interface (FFI).

This is a prototype which may not be developed further.

Based on [hangups](https://github.com/tdryer/hangups) library for Python.

## What works

* Connecting to Hangouts using pre-provided authentication cookies
* Receiving `StateUpdate` messages
* Simple C library in `src/lib.rs`
* Python client for C library in `libhangups.py`
