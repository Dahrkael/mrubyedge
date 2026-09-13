# Patches and divergences

This fork carries its own patch line on top of upstream `v1.1.12`, merged with
upstream `master` up to `c7dd9ae` (the last mruby 3.x commit, before upstream
`v2.0.0`/RITE0400). This document lists what diverges from upstream. The
working branch is `nextgen`; releases are tagged `v1.1.12-ng.N`.

This line interprets mruby 3.x (RITE0300) bytecode and refuses any other RITE
version (`rite::Error::UnsupportedVersion`). The mruby 4.0 line lives on the
separate `nextgen40` branch.

Most entries below were originally captured as individual commits on the
`nextgen` branch; run `git log` for the full history.

## Runtime and diagnostics

- Breadcrumb frames are labeled `Class#method`, and `last_error_stack` snapshots
  the deepest raise before unwinding.
- `do_op_send` pops its breadcrumb when a native function fails, so rescued
  errors do not leave phantom frames in later backtraces.
- RITE parsing reads the DBG section into per-irep line maps; `Breadcrumb`
  records the caller irep and pc and maps an opcode to its source line.
- Raised exceptions capture a MRI-style backtrace with `<main>` and shifted
  call-site lines.
- `check_frame_window` raises `SystemStackError` instead of panicking when the
  fixed register window would overflow.
- Register windows give each callee its own frame, and `call_block` restores the
  caller's `upper` env (upvar chains survive native-to-Ruby calls).
- Unique irep ids across scripts; fixed an `env->proc` cycle that leaked one
  frame per return.

## Opcodes

- `ARYPUSH`, `ARYSPLAT`, `HASHADD`, `HASHCAT`, `SETMCNST`, `GETCV`, `SETCV`.
- These unblock large array/hash literals, `return *x`, `**` splats and module
  constants / class variables that previously panicked as unimplemented.

## Control flow

- `OP_BREAK` records its nearest landing pad and the unwinder delivers the break
  value there; `times`/`each`/`loop`/`Range#each` and the compat iterators stop
  cleanly and return it.
- `return` unwinds to the enclosing method or nearest lambda, raising
  `LocalJumpError` when unmatched.
- Comparison opcodes fall back to `<=>` dispatch instead of panicking on
  non-numerics.

## Classes, modules and constants

- Singleton classes are anchored to `RModule`/class identity, so duplicate
  wrappers share state; `def self.m`, `class << self` and module receivers work.
- `op_module`/`op_class` reuse the canonical wrapper on reopen, and same-named
  classes in different modules no longer collide.
- `op_setconst` is scoped to the defining module/class; bare reads fall back to
  the class of `self`; top-level constants mirror into `Object`.
- `Comparable` derives `< <= > >= == between? clamp` from `<=>` and is included
  in `Integer`/`Float`/`String`.
- Missing exception classes (`KeyError`, `IndexError`, `StopIteration`,
  `LocalJumpError`, `FrozenError`, `IOError`, ...) and real
  `Exception#message`/`to_s`/`inspect` storage.
- `String::try_from` renders flat values instead of `{:?}` over the cyclic
  class graph, which used to overflow the stack.

## Performance

- Unboxed `Value` immediates for numbers, booleans, symbols and nil, with
  containers and the register file storing `Value` directly.
- Symbol interning to `u32` ids, keyed methods/ivars/attr caches by id.
- Inline caches for methods, attributes and constants; inline numeric send fast
  paths.
- Pooled call frames, a preallocated breadcrumb stack, and flyweight singletons.
- Hash `[]`/`[]=` run inline without deep-cloning the table.

## Fixes

- `op_add` no longer mutates the left `String` operand.
- `Array.new(size)` yields each index to a block and supports a default value.
- `Array#[]`/`#at` return nil for out-of-range indices.
- `x..y` / `x...y` handling in `first`/`last`/`step` and enumerable walks.
- Overflow guards in integer/range/sprintf helpers.

## Ruby standard library compatibility layer

Behind the optional `ruby-compat` feature under `src/compat/`. It is
implemented on top of the public VM API and registered explicitly:

```rust
let mut vm = mrubyedge::yamrb::vm::VM::empty();
mrubyedge::compat::register(&mut vm)?;
```

## Known issues

A few `ruby-compat` regression tests are marked `#[ignore]` because they
exercise VM defects that this fork does not fix yet:

- `for x in ...`/`next` block-value handling
  (`compat::tests::control_flow::{for_loop_iterates, next_for_skips,
  break_for_exits, next_block_yields_value}`).

Tests that depend on `Random` are gated behind the `mruby-random` feature.

## Building and testing

The crate is a member of the upstream workspace. On musl hosts the dev
dependencies need bindgen to load `libclang` dynamically:

```sh
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C target-feature=-crt-static" \
  cargo test -p mrubyedge
```
