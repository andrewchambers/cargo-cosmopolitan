# cargo-cosmopolitan

[![CI](https://github.com/andrewchambers/cargo-cosmopolitan/actions/workflows/ci.yml/badge.svg)](https://github.com/andrewchambers/cargo-cosmopolitan/actions/workflows/ci.yml)

Build `no_std + alloc` Rust programs as Cosmopolitan Actually Portable
Executables: one file containing x86-64 and ARM64 code, linked to Cosmopolitan
libc. Designed for applications using [lunacy](https://github.com/andrewchambers/lunacy).

The command uses an existing Cosmopolitan installation from `PATH`. It does not
download or bundle toolchains. Linux is the initial supported build host.

## Install

Install [Cosmopolitan 4.0.2](https://github.com/jart/cosmopolitan/releases/tag/4.0.2)
and add its `bin` directory to `PATH`. Then:

```sh
rustup toolchain install nightly-2026-01-21 --profile minimal --component rust-src
cargo install --git https://github.com/andrewchambers/cargo-cosmopolitan \
  cargo-cosmopolitan --locked
cargo cosmopolitan preflight
```

To install from a checkout, use `cargo install --path . --locked`.
The driver builds on Rust 1.85+; application builds use the pinned nightly.
`--toolchain NAME` selects another nightly, whose compatibility must be tested.

## Build a program

From an application's directory:

```sh
cargo cosmopolitan build --release
cargo cosmopolitan run --release -- arguments
```

`--bin`, `--example`, `--package`, and Cargo's common feature flags select targets.
`--tiny` selects Cosmopolitan's smaller runtime. Output goes into
`target/cosmopolitan/{debug,release}/<name>.com`.

From this repository, try the standalone demo:

```sh
cargo cosmopolitan run --release --manifest-path demo/Cargo.toml -- Andrew
```

The demo uses published lunacy and exercises arguments, allocation, errno,
pthreads, a mutex, a pipe, and a clock. Applications must use `no_std`, a C
`main`, an allocator, and aborting panics; `lunacy_main!` supplies the runtime.
Full Rust `std` and native Rust TLS are not supported.

## CI

Driver tests run with Rust 1.85 and stable; stable also checks formatting and
Clippy. A Linux x86-64 job builds debug, release, LTO/one-codegen-unit/strip,
and pthread-disabled configurations. The same four APEs are then executed on:

- Linux x86-64 and ARM64
- macOS Intel and ARM64
- Windows x86-64

Runtime jobs check the selected architecture, exit status, and demo output.
They do not rebuild the application. See the [workflow](.github/workflows/ci.yml)
for the current checks and their results via the badge above.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

[Build design and limitations](docs/design.md) · [MIT license](LICENSE)
