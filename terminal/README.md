# The EndBASIC programming language - terminal console

![Build and test](https://github.com/endbasic/endbasic/workflows/Build%20and%20test/badge.svg)
![Health checks](https://github.com/endbasic/endbasic/workflows/Health%20checks/badge.svg)
![Deploy to staging](https://github.com/endbasic/endbasic/workflows/Deploy%20to%20staging/badge.svg)
![Deploy to release](https://github.com/endbasic/endbasic/workflows/Deploy%20to%20release/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/endbasic-terminal.svg)](https://crates.io/crates/endbasic-terminal/)
[![Docs.rs](https://docs.rs/endbasic-terminal/badge.svg)](https://docs.rs/endbasic-terminal/)

EndBASIC is an interpreter for a BASIC-like language and is inspired by
Amstrad's Locomotive BASIC 1.1 and Microsoft's QuickBASIC 4.5.  Like the former,
EndBASIC intends to provide an interactive environment that seamlessly merges
coding with immediate visual feedback.  Like the latter, EndBASIC offers
higher-level programming constructs and strong typing.

EndBASIC offers a simplified and restricted environment to learn the foundations
of programming and focuses on features that can quickly reward the programmer.
These features include things like a built-in text editor, commands to
manipulate the screen, and commands to interact with the hardware of a Raspberry
Pi.  Implementing this kind of features has priority over others such as
performance or a much richer language.

EndBASIC is written in Rust and runs both on the web and locally on a variety of
operating systems and platforms, including macOS, Windows, and Linux.

EndBASIC is free software under the [Apache 2.0 License](LICENSE).

## What's in this crate?

`endbasic-terminal` provides an implementation of the EndBASIC console subsystem
using the `crossterm` library, which gives the interpreter textual capabilities
on desktop platforms (including macOS, Unix-like systems, and Windows).