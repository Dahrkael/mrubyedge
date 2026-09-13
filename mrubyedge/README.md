# mrubyedge (nextgen40)

A downstream fork of [mrubyedge](https://github.com/mrubyedge/mrubyedge): a
pure-Rust reimplementation of the mruby VM that keeps its core execution engine
`no_std`-friendly while striving for behavioral compatibility with upstream
mruby.

This is the **mruby 4.0 (RITE0400)** line of the fork, based on upstream
`v2.0.0`. It carries the same patch set as the 3.x line (`nextgen`), ported to
the 4.0 opcode table and exception machinery, plus the Ruby standard-library
compatibility layer.

- Base: upstream `v2.0.0`, with the fork's patches applied on top.
- Working branch: `nextgen40`; releases are tagged `v2.0.0-ng.N`.
- Divergences from upstream: [PATCHES.md](./PATCHES.md).
- Ruby compatibility coverage: [COVERAGE.md](./COVERAGE.md).

## Installation

This fork is consumed directly from git. Pin the release tag for reproducible
builds:

```toml
[dependencies]
mrubyedge = { git = "https://github.com/Dahrkael/mrubyedge", package = "mrubyedge", tag = "v2.0.0-ng.1", default-features = false, features = ["mruby-random", "mruby-hash-fnv"] }
```

Enable `ruby-compat` to build the Ruby standard-library compatibility layer:

```toml
mrubyedge = { git = "https://github.com/Dahrkael/mrubyedge", package = "mrubyedge", tag = "v2.0.0-ng.1", default-features = false, features = ["mruby-random", "mruby-hash-fnv", "ruby-compat"] }
```

## Usage

### Running Precompiled Bytecode

Load and execute a `*.mrb` produced by mruby 4.0's `mrbc`. A chunk whose header
does not say `RITE0400` is refused, so mruby 3.x bytecode has to be recompiled:

```rust
use mrubyedge::rite;
use mrubyedge::yamrb::vm;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script = std::fs::read("script.mrb")?;
    let mut rite = rite::load(&script)?;
    let mut vm = vm::VM::open(&mut rite);
    let value = vm.run()?;
    println!("{:?}", value);
    Ok(())
}
```

### Ruby Standard Library Compatibility Layer

The optional `ruby-compat` feature adds commonly used methods implemented in
Rust on top of the public VM API (more `Array`/`String`/`Hash`/`Integer`/
`Float`/`Range`/`Symbol` methods, `Math`, `Comparable`, extra `Enumerable`
methods, missing exception classes and `Kernel` conversions).

```rust
use mrubyedge::yamrb::vm::VM;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = VM::empty();
    mrubyedge::compat::register(&mut vm)?;
    Ok(())
}
```

See [COVERAGE.md](./COVERAGE.md) for the full method list.

## Use Cases

- **Embedded Systems**: Run Ruby in resource-constrained devices
- **WebAssembly Applications**: Deploy Ruby code in browsers and serverless environments
- **Edge Computing**: Lightweight Ruby runtime for edge nodes
- **Rust Integration**: Embed Ruby scripting in Rust applications

## CLI Tool

For a command-line interface to compile and run Ruby scripts, see [mrubyedge-cli](../mrubyedge-cli).

## Documentation

- [Patches and divergences](./PATCHES.md)
- [Ruby Compatibility Coverage](./COVERAGE.md)
- [GitHub Repository](https://github.com/Dahrkael/mrubyedge)
- [Upstream project](https://github.com/mrubyedge/mrubyedge)

## License

See the [LICENSE](../LICENSE) file in the repository root.
