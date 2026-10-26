# mruby/edge

mruby/edge is yet another mruby-compatible VM implementation, specialized for WebAssembly.

You can try what features mruby/edge have implemented in [Playground](https://mrubyedge.github.io/playground/).

## badges

- ![crates.io](https://img.shields.io/crates/v/mrubyedge.svg) ![crates.io](https://img.shields.io/crates/v/mrubyedge-cli.svg)
- ![docs.rs](https://docs.rs/mrubyedge/badge.svg) ![docs.rs](https://docs.rs/mrubyedge-cli/badge.svg)

## Benchmarks

Build and compare arbitrary Git tags, branches, commits, or the working tree with
the native VM [benchmark suite](./benchmarks/README.md):

```sh
python3 benchmarks/run.py --ref v2.1.2 --ref HEAD
```

The suite saves raw samples and environment metadata as JSON, and reports median
times relative to the first revision.

## crates

### [mrubyedge](./mrubyedge) [![crates.io](https://img.shields.io/crates/v/mrubyedge.svg)](https://crates.io/crates/mrubyedge) [![docs.rs](https://docs.rs/mrubyedge/badge.svg)](https://docs.rs/mrubyedge)


* mruby/edge core VM implementation
* [Ruby Compatibility Coverage](./mrubyedge/COVERAGE.md)

### [mrubyedge-cli](./mrubyedge-cli) [![crates.io](https://img.shields.io/crates/v/mrubyedge-cli.svg)](https://crates.io/crates/mrubyedge-cli) [![docs.rs](https://docs.rs/mrubyedge-cli/badge.svg)](https://docs.rs/mrubyedge-cli)
* CLI endpoint for mruby/edge - run, compile to wasm, etc.

### [mruby-serde-json](./mruby-serde-json) [![crates.io](https://img.shields.io/crates/v/mruby-serde-json.svg)](https://crates.io/crates/mruby-serde-json) [![docs.rs](https://docs.rs/mruby-serde-json/badge.svg)](https://docs.rs/mruby-serde-json)
* JSON serialization/deserialization for mruby/edge using serde_json

### mec [deprecated]

* Old versions of compiler
