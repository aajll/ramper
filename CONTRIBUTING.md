# Contributing to ramper

`ramper` is a ramp generator for control loops. A change must keep every update constant time with no loops, keep the guarantees in the design document (no overshoot, limits respected, exact landing, continuous rate across parameter changes), keep configuration errors at the call that made them, and stay `no_std`-capable and `unsafe`-free. See [docs/design.md](docs/design.md) for the rules the tests enforce.

## Getting started

These are the commands CI runs. Run them locally before opening a pull request.

```sh
# Format, lint and document with warnings denied
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --all-targets --no-default-features --features libm -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Tests with each square-root source, on stable and on the minimum supported Rust version
cargo test
cargo test --no-default-features --features libm
cargo +1.85 test
cargo +1.85 test --no-default-features --features libm

# Prove the library stays no_std
rustup target add thumbv7em-none-eabihf
cargo build --target thumbv7em-none-eabihf --no-default-features --features libm

# Coverage gate: 100% line and region coverage of src/
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
cargo llvm-cov --all-targets --fail-under-lines 100 --fail-under-regions 100

# Dependency policy and advisories
cargo install cargo-deny cargo-audit --locked
cargo deny check
cargo audit
```

## Source style

- `rustfmt` with the default configuration; CI checks `cargo fmt --check`.
- Clippy `pedantic` is enabled through the `[lints]` table in `Cargo.toml` and CI denies warnings. An `#[allow]` carries a comment explaining why.
- Every source file starts with an SPDX licence tag.

## Language rules

- `no_std` without the `std` feature, with no `alloc`. The only runtime dependency is the optional `libm`, for the square root the S-curve needs; do not add another. A dev-dependency is acceptable when it earns its place in the tests.
- `unsafe_code` is forbidden through `[lints.rust]`.
- Public items have rustdoc with an example where the behaviour is not obvious, and `missing_docs` is a warning that CI turns into an error.
- Configuration is validated in `Builder::build` and in each live setter, which return `Result`; `update` has no error path and holds on a bad time step.
- Exact `f32` equality is deliberate where it appears: a move lands exactly on its target. The crate-level `allow(clippy::float_cmp)` carries that reason.

## Tests and coverage

- `src/lib.rs` holds the implementation; unit tests belong there only for private helpers.
- `tests/contract.rs` holds one test per rule in `docs/design.md`, named for the rule it falsifies. A behaviour change updates the design document and the test together.
- `tests/properties.rs` holds the randomised sweep over moves, limits and live target changes.
- The README and the guides under `docs/` are compiled and run as doctests through `include_str!` in `src/lib.rs`, so an example that stops compiling fails `cargo test`. Mark a fenced block that is not runnable Rust with its real language (`toml`, `sh`, `text`).
- Coverage is gated at 100% line and region coverage of `src/`. A new branch comes with the test that reaches it.

## Documentation

| File | Purpose |
| --- | --- |
| `README.md` | Landing page: quick start, description, validated toolchains, and links. Keep it short. |
| `docs/integration.md` | Adding the dependency, the `std` and `libm` features, the minimum Rust version, build and cross builds. |
| rustdoc (`cargo doc`) | Reference for every public item. Changes with the interface. |
| `docs/design.md` | The profiles, the braking envelope, guarantees, decisions, numerical range, cost and verification. |
| `CHANGELOG.md` | User-visible changes, under `[Unreleased]` until a release. |

Update rustdoc together with the interface. Update `docs/design.md` when behaviour changes, and record a design choice with its alternatives under its Decisions section. Write prose in British English, one line per paragraph, with no hard wrapping.

## Commits

Use Conventional Commits:

- `feat: ...` new feature
- `fix: ...` bug fix
- `docs: ...` documentation only
- `test: ...` test-only changes
- `chore: ...` build, CI, release work
- `refactor: ...` a change that neither fixes a bug nor adds a feature

Keep the subject under about 70 characters. Use the body to explain why the change is needed.

## Pull requests

- Open an issue first for non-trivial changes so the design can be agreed before implementation.
- Keep pull requests focused: one feature or one fix.
- All CI checks must pass.
