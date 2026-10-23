# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Ramp`, a ramp generator with linear and acceleration-limited S-curve profiles, independent rise and fall rates and accelerations, output limits, an enable gate, and live rate and state readout.
- `Ramp::builder` and the live setters, which validate every rate, acceleration, limit and target at the call that sets it, so `update` has no error path.
- `Shape`, `State` and `Error`.
- A `std` feature (default) and a `libm` feature for the square root, so the crate builds for `no_std` targets.
- Contract tests for every guarantee in the design document and a randomised property sweep over moves, limits and live target changes, with CI gates on formatting, clippy pedantic, rustdoc, both toolchains and both features, a `thumbv7em-none-eabihf` build, `cargo deny`, `cargo audit` and 100% line and region coverage.
