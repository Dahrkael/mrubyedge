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

## What this fork adds

Everything below is on top of upstream `v2.0.0`. See
[PATCHES.md](./PATCHES.md) for the full list.

### Language flow and semantics

- `break value` from a block/iterator stops the yielding method and returns the
  value (landing pads anchored to the send that yielded).
- `return` unwinds to the enclosing method or the nearest lambda, raising
  `LocalJumpError` when unmatched.
- Comparison opcodes fall back to `<=>` dispatch instead of panicking on
  non-numeric operands.
- `Comparable` derives `< <= > >= == between? clamp` from `<=>` and is included
  in `Integer`, `Float`, `String` and `Symbol`.

### Definitions and object model

- Singleton classes are anchored to module/class identity: `def self.m`,
  `class << self` and explicit module receivers work, and duplicate wrappers
  share singleton state.
- `class`/`module` reopen reuses the canonical wrapper; same-named classes in
  different modules no longer collide.
- Constants are scoped to the defining module/class; top-level constants mirror
  into `Object`, so `Module::CONST` resolves.
- The fork's opcode implementations (`ARYPUSH`, `ARYSPLAT`, `HASHADD`,
  `HASHCAT`, `SETMCNST`, `GETCV`, `SETCV`) are mapped onto the 4.0 table.

### Diagnostics

- MRI-style backtraces with `Class#method:line` frames and a synthetic `<main>`.
- Deep recursion raises `SystemStackError` instead of a Rust panic, and native
  failures no longer leak breadcrumbs into later traces.

### Fixes

- `String#+` no longer mutates the left operand.
- `Array.new(size)` yields each index to a block and supports a default value;
  out-of-range `Array#[]`/`#at` return `nil`.
- `call_block` restores the caller's upper environment, so upvar chains survive
  native-to-Ruby calls.
- Globally unique irep ids across scripts, and a fixed per-frame register leak
  on method return.

### New feature: `ruby-compat`

An optional standard-library compatibility layer (`ruby-compat` feature) adds
`Math`, extra `Array`/`Hash`/`String`/`Integer`/`Float`/`Range`/`Symbol`
methods, `Enumerable` extras, `Kernel` conversions and the missing exception
classes. Enable the feature and call `mrubyedge::compat::register(&mut vm)`.
The full list lives in [COVERAGE.md](./COVERAGE.md).

## Performance

The fork keeps upstream's RITE0400 interpreter and replaces its hot paths:

- Unboxed `Value` immediates for numbers, booleans, symbols and `nil`, with the
  register file and containers storing values directly.
- Symbols interned to `u32` ids; methods, ivars and attribute caches keyed by id.
- Inline caches for method, attribute and constant dispatch, plus inline numeric
  send fast paths.
- Pooled call frames and a preallocated breadcrumb stack (no per-call `Rc`).
- Flyweight singletons for `nil`/`true`/`false` and small integers.
- Pristine `Hash#[]`/`Hash#[]=` run inline without deep-cloning the table.

On a 16-workload pure-Ruby benchmark suite the fork is **about 6.5x faster than
upstream** (geomean), and **within ~1.5x of mruby C** — faster than the C VM on
attribute-heavy and numeric-integration workloads.

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
