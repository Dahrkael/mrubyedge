# mrubyedge (nextgen)

A downstream fork of [mrubyedge](https://github.com/mrubyedge/mrubyedge): a
pure-Rust reimplementation of the mruby VM that keeps its core execution engine
`no_std`-friendly while striving for behavioral compatibility with upstream
mruby.

This fork is maintained independently. It carries a batch of VM fixes,
opcode coverage, performance work, richer diagnostics (backtraces with source
lines) and a Ruby standard-library compatibility layer on top of the upstream
`v1.1.12` base, merged with upstream `master` up to the last mruby 3.x commit.

- Base: upstream `v1.1.12` (`8e803ca`), merged with upstream `master` up to `c7dd9ae` (the last 3.x commit; upstream `v2.0.0` and later are RITE0400 and tracked on a separate branch).
- Working branch: `nextgen`; releases are tagged `v1.1.12-ng.N`.
- Divergences from upstream: [PATCHES.md](./PATCHES.md).
- Ruby compatibility coverage: [COVERAGE.md](./COVERAGE.md).

## What this fork adds

Everything below is on top of the upstream base (`v1.1.12` + `master` through
`c7dd9ae`). See [PATCHES.md](./PATCHES.md) for the full list.

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
- Missing exception classes (`KeyError`, `IndexError`, `StopIteration`,
  `LocalJumpError`, `FrozenError`, `IOError`) plus real
  `Exception#message`/`to_s`/`inspect` storage.
- New opcodes: `ARYPUSH`, `ARYSPLAT`, `HASHADD`, `HASHCAT`, `SETMCNST`,
  `GETCV`, `SETCV` (large array/hash literals, `return *x` and `**` splats,
  module constants and class variables).

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

The fork keeps the same behavior while replacing the interpreter's hot paths:

- Unboxed `Value` immediates for numbers, booleans, symbols and `nil`, with the
  register file and containers storing values directly.
- Symbols interned to `u32` ids; methods, ivars and attribute caches keyed by id.
- Inline caches for method, attribute and constant dispatch, plus inline numeric
  send fast paths.
- Pooled call frames and a preallocated breadcrumb stack (no per-call `Rc`).
- Flyweight singletons for `nil`/`true`/`false` and small integers.
- Pristine `Hash#[]`/`Hash#[]=` run inline without deep-cloning the table.

On a 16-workload pure-Ruby benchmark suite (arithmetic, method dispatch,
attribute access, hashes, strings, grid reads/writes, `fib`), the fork is
**about 6.5x faster than upstream `master`** (geomean), and **within ~1.5x of
mruby C** — faster than the C VM on attribute-heavy and numeric-integration
workloads.

## Overview

mruby/edge is an mruby-compatible virtual machine implementation written in
Rust, specifically designed for WebAssembly environments and embedded systems.
It aims to provide:

- **WebAssembly-first design**: Optimized for running Ruby code in browsers and edge computing environments
- **Lightweight runtime**: Minimal footprint and binary size suitable for constrained environments
- **`no_std` core**: Can run in environments without standard library support
- **mruby compatibility**: Executes mruby bytecode (`.mrb` files) and Ruby source code
- **Rust safety**: Built with Rust for memory safety and reliability

## Installation

This fork is consumed directly from git. Pin the release tag for reproducible
builds:

```toml
[dependencies]
mrubyedge = { git = "https://github.com/Dahrkael/mrubyedge", package = "mrubyedge", tag = "v1.1.12-ng.1", default-features = false, features = ["mruby-random", "mruby-hash-fnv"] }
```

Enable `ruby-compat` to build the Ruby standard-library compatibility layer
described below:

```toml
mrubyedge = { git = "https://github.com/Dahrkael/mrubyedge", package = "mrubyedge", tag = "v1.1.12-ng.1", default-features = false, features = ["mruby-random", "mruby-hash-fnv", "ruby-compat"] }
```

## Usage

### Running Precompiled Bytecode

Load and execute a precompiled `*.mrb` file produced by `mrbc`:

```rust
use mrubyedge::rite;
use mrubyedge::yamrb::vm;

// Bundle the compiled script at build time
const SCRIPT: &[u8] = include_bytes!("./examples/simple.mrb");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rite = rite::load(SCRIPT)?;
    let mut vm = vm::VM::open(&mut rite);
    let value = vm.run()?;
    println!("{:?}", value);
    Ok(())
}
```

### Creating VMs Programmatically

You can also construct IREP (internal representation) structures directly:

```rust
use mrubyedge::yamrb::{op, vm, value::RSym};
use mrubyedge::rite::insn::{Fetched, OpCode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let irep = vm::IREP {
        __id: 0,
        nlocals: 0,
        nregs: 7,
        rlen: 0,
        code: vec![
            op::Op { code: OpCode::LOADI_1, operand: Fetched::B(1), pos: 0, len: 2 },
            op::Op { code: OpCode::LOADI_2, operand: Fetched::B(2), pos: 2, len: 2 },
            op::Op { code: OpCode::ADD, operand: Fetched::B(1), pos: 4, len: 2 },
            op::Op { code: OpCode::STOP, operand: Fetched::Z, pos: 6, len: 1 },
        ],
        syms: vec![],
        pool: Vec::new(),
        reps: Vec::new(),
        catch_target_pos: Vec::new(),
    };

    let mut vm = vm::VM::new_by_raw_irep(irep);
    let value = vm.run()?;
    println!("{:?}", value);
    Ok(())
}
```

### Ruby Standard Library Compatibility Layer

The VM prelude covers the core classes. The optional `ruby-compat` feature
adds a layer of commonly used methods implemented in Rust on top of the public
VM API (more `Array`/`String`/`Hash`/`Integer`/`Float`/`Range`/`Symbol`
methods, `Math`, `Comparable`, extra `Enumerable` methods, missing exception
classes and `Kernel` conversions).

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
