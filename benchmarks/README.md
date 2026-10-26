# Version benchmarks

Run the same Ruby workloads and Rust harness against selected mruby/edge Git
revisions. Requires Python 3.11+, Git, Cargo/Rust (edition 2024), and the C build
tools required by `mruby-compiler2-sys`. Cargo needs registry access on the first
build. No Python packages or benchmark tools need installing.

```sh
# Compare tags, branches, full/abbreviated commit SHAs, or remote-tracking refs.
# The first revision is the baseline. Omitting --ref selects HEAD.
python3 benchmarks/run.py --ref v2.1.2 --ref HEAD

# Include local, uncommitted Rust changes.
python3 benchmarks/run.py --ref HEAD --ref working-tree

# Select workloads and shorten the run for a smoke check.
python3 benchmarks/run.py --ref HEAD --case instance_variables --case hash \
  --samples 3 --sample-ms 20 --warmup-ms 50

# Measure only repeated execution in an initialized VM, with explicit features.
python3 benchmarks/run.py --ref v2.1.2 --ref HEAD --mode warm \
  --no-default-features --features wasi,mruby-hash-fnv --output /tmp/bench.json

python3 benchmarks/run.py --list
```

Cargo default features are enabled unless `--no-default-features` is specified.
In current releases these include `mrubyedge-debug`, whose per-instruction debug
environment checks affect performance even in a release build. To compare without
those checks, pass `--no-default-features --features wasi` to the whole comparison.

Refs must exist locally; fetch missing tags/branches first with `git fetch origin
--tags`. Each committed revision is extracted into an isolated directory using
`git archive`; your current branch and files are left in place. `working-tree`
uses the current source directory, including uncommitted edits. Avoid editing
Rust sources while builds are running.

The harness depends directly on that revision's `mrubyedge/` directory. It does
not use the CLI, which can depend on a separately published VM version. Each
version gets a release executable, a standalone manifest, a Cargo.lock and a build
log. Compilation artifacts are reused under `target/benchmarks/build/`; source
snapshots and run artifacts live in a fresh `target/benchmarks/run-*/` directory.
Builds finish before timing begins. This requires no checkout, worktree, or
changes to historical commits.

The compiler version is inferred from each revision's `mruby-compiler2-sys`
dev-dependency and pinned to its declared release, so both mruby 3.x and 4.x
bytecode formats can be tested. Use `--compiler-version 0.5.0` to override it.
Old revisions without that dependency need an explicit compatible compiler.
Revisions must expose `rite::load`, `VM::open`, `VM::run`, `mrb_funcall`, and the
integer result conversion used in `runner.rs`; incompatible APIs, unsupported
Ruby behavior, or build failures stop the run with a nonzero exit status.
Version-specific bytecode/compiler changes are part of the comparison; this
does not force identical bytecode across incompatible VM versions.

## What is measured

This suite measures the **native host VM**. It does not measure WebAssembly,
WASI runtime overhead, CLI startup, or Ruby compilation speed.

Each workload defines `benchmark`, returns a deterministic integer, and creates
its own data on every invocation. The runner checks the result on every iteration
and fails on errors or mismatches. Ruby compilation happens once, before timing.

| Mode | Included in one timed iteration |
| --- | --- |
| `cold` | Bytecode loading, VM creation, top-level class/method definitions, one `benchmark` call, result check, destruction |
| `warm` | One `benchmark` call in an initialized VM, result check and result destruction |

`--mode both` (default) measures both separately. Each case/mode/version runs in a
separate process. Native calls into Ruby and workload object allocations are
included. Warm mode reuses the VM and its caches, but the workload resets its
own data. Cold mode includes initialization, so its result is not a pure execution
time. There is no process startup or build time in either measurement.

The runner warms up for 300 ms, calibrates an iteration count, then collects 15
batches targeting 100 ms each. An iteration is a **whole workload invocation**,
not a single Ruby operation. Very slow invocations can exceed the requested batch
duration; very fast ones are capped at 1,000,000 iterations per batch. Warmup and
calibration are discarded. Median, mean, minimum and sample standard deviation
use the elapsed time per iteration in each batch.

The Markdown comparison reports median milliseconds per invocation. `vs first`
is `baseline median / selected median`: **above 1.0x is faster**. These descriptive
ratios are not significance tests. Run on an idle machine with the same Rust
toolchain and feature flags, repeat comparisons, and reverse revision order to
check thermal/order effects before drawing conclusions.

## Results and reproduction

The default output is `results.json` plus `results.md` inside the run directory.
`--output PATH` selects another JSON path and writes Markdown beside it. JSON
records commit SHAs, working-tree dirty status, crate and compiler versions,
feature flags, Rust/Cargo versions, platform, environment flags, executable sizes,
runner/workload hashes, lockfile hashes, raw batch iteration counts and durations,
and summary statistics. Partial JSON is saved after each successful measurement;
a failed run exits nonzero and must not be treated as a completed comparison.
Only a report with `status: "complete"` represents a completed run.

Keep the entire run directory to retain source snapshots, workload files,
generated harnesses, lockfiles, executables, raw TSV samples and logs. Cargo's
standalone lockfiles preserve the actual dependency resolution for each build;
the historical workspace lockfile is not applied to the generated project.
For `working-tree`, record or commit the source separately: a dirty flag and HEAD
SHA do not capture uncommitted changes. The binary size is for the complete
native harness including the Ruby compiler; it is not the VM or Wasm size.

Results and build artifacts are ignored by Git under `target/`. Remove
`target/benchmarks/` when no longer needed.

## Workloads

| Name | Operations |
| --- | --- |
| `integer` | Integer arithmetic and a while loop |
| `method_calls` | Ruby method dispatch and counter updates |
| `instance_variables` | Multiple ivar slots, direct access and attr accessors |
| `hash` | String-key inserts and lookups with interpolation |
| `array` | Array growth, block calls and iteration |
| `string` | String concatenation, slicing and length |
| `cart` | Build carts with 100, 10 and 2 line items; compute category-dependent member discounts, coupons, tax and shipping |
| `html_render` | Build 100 article records; escape titles/authors and render 75 published articles into an HTML list |
| `request_routing` | Build and dispatch 300 requests through method/path lookup, authorization, controllers and response construction |

The application workloads include input generation and result validation in each
invocation, in both modes. `html_render` also computes a rolling checksum over the
complete output inside the timed region. `request_routing` covers successful
product, order and account requests, unauthorized requests, missing paths and
unsupported methods; it performs no network I/O. Their results describe the
whole workflow, including allocation and validation, rather than an isolated
rendering, pricing or dispatch operation.
These workloads use `while` loops to avoid historical block/upvar handling bugs
in v1.1.12 when blocks call into other Ruby methods.

Add a Ruby file under `workloads/` with a first line `# expected: INTEGER` and a
zero-argument `benchmark` method returning that integer. Avoid output, randomness,
external I/O, or persistent state that changes between calls. The runner lists
and discovers files automatically; `--case NAME` selects the filename stem.

Run orchestration checks with:

```sh
python3 -m unittest discover -s benchmarks -p 'test_*.py'
```
