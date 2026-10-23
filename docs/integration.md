# Integration guide

How to add `ramper` to a Rust project. The [API reference](https://docs.rs/ramper) describes every public item and [design.md](design.md) explains the profiles and their guarantees.

## Requirements

- Rust 1.85 or later (edition 2024). The minimum is recorded as `rust-version` in `Cargo.toml` and checked in CI.
- A square root. The default `std` feature takes it from the standard library; the `libm` feature takes it from the [`libm`](https://crates.io/crates/libm) crate for `no_std` targets. One of the two must be enabled.

## Git dependency

Until the crate is published, depend on a release tag:

```toml
[dependencies]
ramper = { git = "https://github.com/aajll/ramper", tag = "v0.1.0" }
```

For a `no_std` target, turn off the default feature and enable `libm`:

```toml
[dependencies]
ramper = { git = "https://github.com/aajll/ramper", tag = "v0.1.0", default-features = false, features = ["libm"] }
```

Pinning a tag rather than a branch keeps builds reproducible and makes an upgrade a deliberate change.

## Published crate

Once published, use the version from crates.io:

```toml
[dependencies]
ramper = "0.1"
```

## Use

Build the ramp where the configuration is loaded, so a bad rate is a start-up error. Drive the target from the live setpoint and call `update` from the control loop with the measured step:

```rust
use ramper::{Ramp, Shape, State};

struct BusControl {
    setpoint: Ramp,
}

impl BusControl {
    fn new() -> Result<Self, ramper::Error> {
        // 50 V/s up with a gentle 100 V/s² ease; 2000 V/s down with a hard
        // 40 000 V/s² stop, so a trip takes a quarter of a second.
        let setpoint = Ramp::builder(0.0, 50.0)
            .rates(50.0, 2000.0)
            .accels(100.0, 40_000.0)
            .limits(0.0, 800.0)
            .shape(Shape::SCurve)
            .build()?;
        Ok(Self { setpoint })
    }

    fn tick(&mut self, requested: f32, dt: f32) -> f32 {
        // A NaN request is the only way this can fail; keep the last target.
        let _ = self.setpoint.set_target(requested);
        self.setpoint.update(dt)
    }
}

let mut bus = BusControl::new().unwrap();
let out = bus.tick(400.0, 0.001);
assert!(out > 0.0 && out < 1.0);
assert_eq!(bus.setpoint.state(), State::Moving);
```

`Ramp::rate` gives the current signed rate for feedforward, and `State` distinguishes a disabled ramp from one at rest on its target.

## Build and test

```sh
cargo test
cargo test --no-default-features --features libm
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
```

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the coverage, dependency and audit gates CI runs.

## Cross builds

CI builds the library for `thumbv7em-none-eabihf` with the `libm` feature to prove it stays `no_std`:

```sh
rustup target add thumbv7em-none-eabihf
cargo build --target thumbv7em-none-eabihf --no-default-features --features libm
```

Any target with `core` works. Tests run on the host.
