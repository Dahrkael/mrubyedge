# Patches and divergences

This branch (`nextgen40`) ports the fork's patch set onto upstream `v2.0.0`
(mruby 4.0 / RITE0400). The 3.x line lives on the `nextgen` branch; releases
here are tagged `v2.0.0-ng.N`.

This line interprets mruby 4.0 (RITE0400) bytecode; 3.x chunks are refused by
`rite::load`.

Most entries below were originally captured as individual commits on the
`nextgen` branch and are re-applied here against the 4.0 opcode table and
exception machinery. Run `git log` for the full history.

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
  caller's `upper` env.
- Unique irep ids across scripts; fixed an `env->proc` cycle that leaked one
  frame per return.

## Opcodes

- Implementations for opcodes left unimplemented upstream and used by the
  engine: `ARYPUSH`, `ARYSPLAT`, `HASHADD`, `HASHCAT`, `SETMCNST`, `GETCV`,
  `SETCV` (mapped onto the 4.0 table).

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
  classes in different modules do not collide.
- `op_setconst` is scoped to the defining module/class; bare reads fall back to
  the class of `self`; top-level constants mirror into `Object`.
- `Comparable` derives `< <= > >= == between? clamp` from `<=>`.
- Missing exception classes and real `Exception#message`/`to_s`/`inspect`.
- `String::try_from` renders flat values instead of `{:?}` over the cyclic
  class graph.

## Fixes

- `op_add` does not mutate the left `String` operand.
- `Array.new(size)` yields each index to a block and supports a default value.
- `Array#[]`/`#at` return nil for out-of-range indices.
- Overflow guards in integer/range/sprintf helpers.

## Ruby standard library compatibility layer

Behind the optional `ruby-compat` feature under `src/compat/`, registered with
`mrubyedge::compat::register(&mut vm)`.

## Performance

The 3.x line's interpreter optimizations are ported onto this branch: unboxed
`Value` immediates, symbol ids, method/attribute/constant inline caches, pooled
call frames, a preallocated breadcrumb stack, flyweight singletons and inline
`Hash#[]`/`Hash#[]=`.

## Known issues

- `instance_method_constant_assignment_is_not_lexically_scoped` is `#[ignore]`d:
  mruby 4.0's compiler rejects a dynamic constant assignment at compile time, so
  the runtime limitation it documented on the 3.x line cannot be exercised.
- The `rite` DBG-section parser (backtrace line numbers) is ported from the 3.x
  line. The fork's own test build uses an unpatched `mruby-compiler2-sys 0.5.0`,
  which does not emit DBG, so the parser is validated end to end only with the
  debug-info-enabled compiler used by the consuming engine.

## Building and testing

On musl hosts the dev dependencies need bindgen to load `libclang` dynamically:

```sh
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C target-feature=-crt-static" \
  cargo test -p mrubyedge
```
