# clapback

[![CI](https://github.com/drmartinwimmer/clapback/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/clapback/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/clapback.svg)](https://crates.io/crates/clapback)
[![Docs.rs](https://docs.rs/clapback/badge.svg)](https://docs.rs/clapback)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

Declarative CLI command builder and runner for Rust with `clap`-style derive macros and composable command wrappers.

## Overview

While `clap` parses incoming command-line arguments into Rust structs, `clapback` does the reverse: you define outbound CLI invocations as strongly typed Rust structs and enums using familiar `#[command(...)]` and `#[arg(...)]` attributes, compose wrapper commands (`fence`, `direnv`, `ssh`, `sudo`), and execute or parse their output directly through methods on the struct.

- **Clap-Style Derive Macros**: Define commands, subcommands, flags, options, positional arguments, and environment bindings declaratively.
- **Ergonomic Execution**: Construct `std::process::Command` instances, run synchronously with inherited I/O, capture raw output, or deserialize structured JSON stdout.
- **First-Class Composition**: Wrap inner commands inside outer wrapper commands (e.g. `fence -- direnv exec . agy ...`) while preserving type safety and dry-run formatting.

## Installation

```bash
cargo add clapback
```

## License

MIT
