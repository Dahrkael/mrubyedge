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

## Generate a compatibility report

Python 3 (standard library only) and Cargo are required. From the workspace root:

```sh
# Markdown is the default; writes target/mruby-spec-report.md
python3 scripts/mruby_spec_report.py

# Alternative formats
python3 scripts/mruby_spec_report.py --format json
python3 scripts/mruby_spec_report.py --format html

# Select files, Cargo features, and a destination
python3 scripts/mruby_spec_report.py --file array --file ensure \
  --features mruby-random,mruby-regexp --output target/selected-specs.md
```

The report contains a file summary and every reached `assert` block, including
its result, failure details, and parent ID for nested assertions. It counts
`assert` blocks, not individual `assert_equal` or `assert_true` calls. Results
are `pass`, `fail`, `error`, `warn` (no assertions), `skip`, `stress` (disabled),
or `interrupted` (started but did not finish).

File status distinguishes `passed`, `failed` (completed with assertion failures
or rescued errors), `compile_error`, `runtime_error`, `panic`, `aborted`,
`timeout`, `premature_return`, and `incomplete`. After an interrupted file,
remaining tests are **unevaluated**, with no guessed count. Conditional tests
that were never entered are not counted as skips.

Each report links to raw logs and append-only JSONL events in a unique sibling
`mruby-spec-logs-*` directory. Keep this directory with the report. Markdown
contains all assertion rows; HTML groups them in expandable file sections.
JSON uses `schema_version: 1`, with `metadata`, `files`, `file_counts`, and
`assert_counts`; file entries include assertion records and log paths. All
formats record the commit, dirty state, selected features, upstream revision,
and generation time. `--no-default-features` and `--timeout SECONDS` are also
supported (30 seconds per file by default).

The command writes a report even when compatibility tests fail, then exits
with **1**. It exits with **0** only if all selected files pass, and **2** for
build or reporting errors. This keeps failures visible to CI. The existing
`cargo test` command remains available and does not write report files.

Reporter self-tests:

```sh
python3 -m unittest discover -s scripts -p 'test_mruby_spec_report.py'
cargo test -p mrubyedge --test mruby_spec
```

The upstream tests are explicitly ignored by default because this is a
compatibility suite with known failures, not a passing regression baseline.
A failing assertion, unhandled error, compiler failure, VM panic, premature
VM return, or timeout produces a failed Rust test and a nonzero exit status.
Each file runs in its own child process with a fresh VM and a 30-second limit.
The runner appends an end-of-file marker in memory to detect premature returns;
the vendored files are never rewritten.

`helpers.rb` implements assertions, exception checks, floating-point comparison,
reporting, skip, and stress handling. Native test-only helpers supply printing
and glob matching. Missing core APIs are not monkey-patched. Nested assertion failures
propagate through a failure counter; reporting is intentionally simpler than
mruby's numbered assertion tree. Stress cases remain disabled by default.

This imports the Ruby core suite, not mruby's C tests, mrbgem tests, or binary
tests. Tests requiring mruby C-internal fixtures (for example `__env_*`, GC
internals, and `mrb_vformat`) currently report errors. These are not equivalent
to Ruby language compatibility failures and are not replaced with success stubs.
An abort or top-level error prevents the remainder of that file from running,
but does not prevent the other files from running.

After the block argument and return-unwinding fixes, the default-feature run
has 9 files passing and 39 failing. Passing files:
`bs_literal`, `iterations`, `lang`, `localjumperror`, `rangeerror`, `regexperror`, `runtimeerror`,
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
