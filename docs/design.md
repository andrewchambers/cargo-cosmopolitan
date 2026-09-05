# Build design

Preflight discovers `cosmocc` through `PATH` and checks its sibling tools and
loaders. It compiles and links a host Rust program and compiles, archives, and
links a pthread C program for each target architecture.

The driver derives custom targets from the selected nightly's architecture
specifications. These rebuild `core`, `alloc`, and compiler builtins with:

- `target_os = "cosmopolitan"`, the Unix family, and static linkage
- aborting panics and no native Rust TLS
- no x86-64 red zone, and reserved ARM64 registers x18 and x28
- frame pointers suitable for Cosmopolitan's runtime

Target-specific `CC` and `AR` variables direct C build dependencies to
Cosmopolitan. Host build scripts and procedural macros use the host toolchain.
A linker adapter delegates startup objects, linker scripts, and `fixupobj` to
the architecture-specific Cosmopolitan compiler drivers.

Cargo's JSON messages identify each architecture's executables. `apelink`
combines matching targets; `pecheck` validates the Windows image before the
output replaces the previous APE. Intermediate ELF files remain available for
debugging. Symbol stripping is deferred until packaging because `fixupobj`
requires the ELF symbol table even when a Cargo profile requests `strip = true`.

On Linux, `run` invokes Cosmopolitan's native APE loader explicitly, avoiding
WSL's Windows executable interception. Build tools which are themselves APEs
receive shell launchers; no system-wide binfmt registration is required.

## Limitations

This is an early implementation for Linux build hosts. Dynamic linking, Rust
`std`, native Rust TLS, and `cargo test` for the custom targets are unsupported.
Dependencies must understand the Cosmopolitan target or remain platform-neutral.
The normal Rust `libc` crate does not support this target. Cargo also compiles
development dependencies when building examples, so examples in a package with
a `libc` development dependency need a separate application package.

Cosmopolitan 4.0.2 and nightly-2026-01-21 are the tested toolchain pair. The custom
JSON target schema is unstable; overriding the nightly can require changes to
the generated specification. Runtime validation uses the distributed APE itself,
not only intermediate ELF images or successful structural checks.

References:

- [Cosmopolitan compiler drivers](https://github.com/jart/cosmopolitan/tree/master/tool/cosmocc)
- [APE ABI](https://github.com/jart/cosmopolitan/blob/master/ape/specification.md)
- [Cargo build-std](https://doc.rust-lang.org/cargo/reference/unstable.html#build-std)
