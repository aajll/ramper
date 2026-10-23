# ramper

[![CI](https://github.com/aajll/ramper/actions/workflows/ci.yml/badge.svg)](https://github.com/aajll/ramper/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

**A unit-agnostic ramp generator for control loops, with linear and acceleration-limited S-curve profiles, asymmetric rise and fall, and output clamping, for `no_std` control code.**

## Quick start

Add the crate as a git dependency; see the [integration guide](docs/integration.md) for the tag to pin, the `no_std` feature, and the other ways to depend on it.

```rust
use ramper::Ramp;

// Ramp a DC bus setpoint from 0 to 400 V at 50 V/s, never outside 0 to 800 V.
let mut vbus = Ramp::builder(0.0, 50.0).limits(0.0, 800.0).build().unwrap();
vbus.set_target(400.0).unwrap();

// Called from a 1 kHz control loop; the output reaches 400 V after 8 s.
let mut ticks = 0;
while !vbus.at_target() {
    vbus.update(0.001);
    ticks += 1;
}
assert_eq!(vbus.output(), 400.0);
assert!((7_990..=8_010).contains(&ticks));
```

## Description

`ramper` moves an `f32` output toward a target at a bounded rate, one caller-timed step at a time. The caller decides the unit (volts, hertz, amps, RPM) and supplies the time step, so the crate has no clock and no OS dependency.

- A linear profile at a constant rate, and an S-curve profile that also bounds how fast the rate changes, so the output eases in and out of a move
- Independent rise and fall rates and accelerations, so a slow controlled rise can be paired with a fast trip down
- The output rate is carried across a target, rate, limit or shape change, so a ramp driven by a live setpoint never restarts or steps
- Output clamping on every update, enable and disable, and live rate and state readout for feedforward and monitoring
- Configuration is validated when it is set, so an update has no error path and a bad time step holds the output
- Caller-owned state, no allocation, no `unsafe`, and bounded constant work per update
- `no_std` with the `libm` feature; tests cover every line and region, with a randomised sweep over thousands of moves asserting the guarantees

The crate does not measure time, offer profiles beyond linear and S-curve, limit jerk, or share state between threads.

## Validated toolchains

| Toolchain | Target | Validation |
| --- | --- | --- |
| Rust stable and 1.85 | `x86_64` Linux | Contract and property tests with `std` and with `libm`, clippy pedantic, coverage gate |
| Rust stable | `thumbv7em-none-eabihf` | `no_std` library build with `libm` |

See [cross builds](docs/integration.md#cross-builds).

## Documentation

- [Integration guide](docs/integration.md) — adding the dependency, features, minimum Rust version, and cross builds
- [API reference](https://docs.rs/ramper) — rustdoc for every public item, or `cargo doc --open`
- [Design](docs/design.md) — the linear and S-curve profiles, their guarantees, numerical range, decisions, and verification
- [Changelog](CHANGELOG.md) — release history

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you open a pull request. Report security issues as described in [SECURITY.md](.github/SECURITY.md).

## License

MIT. See [LICENSE](./LICENSE).
