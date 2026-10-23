# AGENTS.md

---

## 1) Project-specific instructions

**Project:** `ramper`
**Primary goal:** A unit-agnostic linear and S-curve ramp generator for control loops, in `no_std`-capable Rust with no `unsafe` and no dependency beyond the optional `libm`.

### 1.1 Essential commands

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets --no-default-features --features libm -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test
cargo test --no-default-features --features libm
cargo build --target thumbv7em-none-eabihf --no-default-features --features libm
cargo llvm-cov --all-targets --fail-under-lines 100 --fail-under-regions 100
cargo deny check
cargo audit
```

### 1.2 Notes

- The README and `docs/*.md` are doctested through `include_str!` in `src/lib.rs`; a Rust fence there must compile and run.
- The crate is at `0.1.0` until its first release; do not bump the version.

## 2) Non-negotiable design invariants

- `update` is constant time: no loops, no work that depends on the distance or the rate.
- A move never overshoots the effective target, never leaves the limits, and lands exactly on the target (`at_target` is an exact comparison).
- The rate is continuous across a change of target, rate, acceleration or limits, and across a switch from linear to S-curve.
- Configuration errors are returned by `build` and the setters; `update` never fails and holds on a time step that is not finite and positive.
- The braking envelope is a clamp on the rate, never a target for the acceleration limiter (see design.md, "Clamp, not target").
- `no_std` without the `std` feature, no `alloc`, `unsafe_code = "forbid"`, `libm` is the only permitted dependency.

## 3) CI / source of truth

`.github/workflows/ci.yml` is the source of truth for the gates: format, clippy pedantic, rustdoc, tests on stable and the `rust-version` toolchain, a `thumbv7em-none-eabihf` build, 100% line and region coverage, `cargo deny` and `cargo audit`.

## 4) Docs / commit conventions

### Documentation layout

- `README.md` is a short landing page. Detail goes in `docs/`.
- `docs/integration.md` covers adding the dependency, MSRV and cross builds.
- Rustdoc is the API reference and changes with the interface.
- `docs/design.md` is the single design record; a behaviour change updates it and `tests/contract.rs` together, and a design choice goes under its Decisions section with the alternatives.
- Prose is British English, one line per paragraph, no hard wrapping, no em dashes joining clauses.

### Commits

Conventional Commits, subject under about 70 characters, body explains why. No tool attribution lines.

## 5) Rust style expectations

- Edition 2024, `rust-version = "1.85"`; do not use a feature newer than the MSRV.
- Clippy pedantic through `[lints]`; justify any `#[allow]` in a comment.
- Prefer `const fn` for accessors and field-only mutators.
- Validation lives in `Builder::build` and the live setters; the error enum is `#[non_exhaustive]`.
- Tests are named for the rule they falsify.
