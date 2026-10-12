# Upstream mruby compatibility tests

`upstream/t/` contains all 48 Ruby files from
[mruby/mruby test/t at 891d1d9](https://github.com/mruby/mruby/tree/891d1d9dc90f1578eedd1003d645250d94eb07e9/test/t).
They are copied byte-for-byte, including upstream comments and conditional
tests. The full source revision is `891d1d9dc90f1578eedd1003d645250d94eb07e9`.
`upstream/LICENSE` is mruby's MIT license. `upstream/assert.rb` is retained as
the reference assertion API; the runner uses `helpers.rb` instead.

Run from the workspace root:

```sh
# Driver self-tests (also included in normal cargo test)
cargo test -p mrubyedge --test mruby_spec

# All upstream files, with readable sequential output
cargo test -p mrubyedge --test mruby_spec -- --ignored --test-threads=1

# One file
cargo test -p mrubyedge --test mruby_spec spec_array -- --ignored --exact
```

The upstream tests are explicitly ignored by default because this is a
compatibility suite with known failures, not a passing regression baseline.
A failing assertion, unhandled error, compiler failure, VM panic, premature
VM return, or timeout produces a failed Rust test and a nonzero exit status.
Each file runs in its own child process with a fresh VM and a 30-second limit.
The runner appends an end-of-file marker in memory to detect premature returns;
the vendored files are never rewritten.

`helpers.rb` implements assertions, exception checks, floating-point comparison,
reporting, skip, and stress handling. Native test-only helpers supply printing,
glob matching, and block capture around the VM's current optional/rest argument
limitation. Missing core APIs are not monkey-patched. Nested assertion failures
propagate through a failure counter; reporting is intentionally simpler than
mruby's numbered assertion tree. Stress cases remain disabled by default.

This imports the Ruby core suite, not mruby's C tests, mrbgem tests, or binary
tests. Tests requiring mruby C-internal fixtures (for example `__env_*`, GC
internals, and `mrb_vformat`) currently report errors. These are not equivalent
to Ruby language compatibility failures and are not replaced with success stubs.
An abort or top-level error prevents the remainder of that file from running,
but does not prevent the other files from running.

On the initial default-feature run, 8 files passed and 40 failed. Passing files:
`bs_literal`, `iterations`, `lang`, `rangeerror`, `regexperror`, `runtimeerror`,
`standarderror`, and `typeerror`. This is a file-level snapshot, not a count of
individual assertions or a guarantee for other feature configurations.

To verify the imported test contents:

```sh
cd mrubyedge/tests/mruby_spec/upstream
shasum -a 256 -c SHA256SUMS
```

When updating upstream, replace the entire `upstream/t/` directory from an
explicit commit, refresh the license and assertion reference, update
`SHA256SUMS` and the revision above, and keep the file list in
`../mruby_spec.rs` in sync. Do not edit imported test bodies to make them pass.
