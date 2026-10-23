// SPDX-License-Identifier: MIT
#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(
    clippy::float_cmp,
    reason = "exact f32 equality is the contract: a move lands exactly on its target and at_target compares exactly"
)]

use core::fmt;

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("ramper needs the `std` feature or the `libm` feature for the S-curve square root");

#[cfg(feature = "std")]
fn sqrt(x: f32) -> f32 {
    x.sqrt()
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
fn sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}

/// A clamp that passes NaN through instead of panicking, for values that
/// may be the result of an overflowed intermediate.
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// The `f32` adjacent to `x` in the direction of `y`.
///
/// `x` is finite, non-zero and differs from `y`. The linear step only calls
/// this when rounding carried `x` past `y` by more than the budget, which
/// cannot happen with `x` equal to `y` or exactly zero.
fn next_toward(x: f32, y: f32) -> f32 {
    let bits = x.to_bits();
    let away_from_zero = (y > x) == (x > 0.0);
    f32::from_bits(if away_from_zero { bits + 1 } else { bits - 1 })
}

/// The rate profile of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
    /// Constant rate; the rate steps at the start and end of a move.
    #[default]
    Linear,
    /// Rate bounded by an acceleration limit, so the output eases in and
    /// out of a move.
    SCurve,
}

/// What a ramp is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// Updates hold the output.
    Disabled,
    /// The output equals the effective target.
    AtTarget,
    /// The output is moving toward the effective target.
    Moving,
}

/// Why a configuration was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The initial or reset value is NaN or infinite.
    Value,
    /// A rate is NaN, infinite, zero or negative.
    Rate,
    /// An acceleration is NaN, infinite, zero or negative.
    Accel,
    /// A limit is NaN, or the minimum is above the maximum.
    Limits,
    /// The target is NaN.
    Target,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::Value => "value is not finite",
            Error::Rate => "rate is not finite and positive",
            Error::Accel => "acceleration is not finite and positive",
            Error::Limits => "limits are not ordered finite values",
            Error::Target => "target is NaN",
        })
    }
}

impl core::error::Error for Error {}

fn positive(x: f32) -> bool {
    x.is_finite() && x > 0.0
}

/// Whether `min` and `max` are comparable and in order. Infinite limits are
/// allowed; they mean no limit on that side.
fn ordered(min: f32, max: f32) -> bool {
    !min.is_nan() && !max.is_nan() && min <= max
}

/// Builds a [`Ramp`], validating everything once.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use = "a builder does nothing until `build` is called"]
pub struct Builder {
    initial: f32,
    rise_rate: f32,
    fall_rate: f32,
    rise_accel: f32,
    fall_accel: f32,
    min: f32,
    max: f32,
    shape: Shape,
}

impl Builder {
    /// Independent rise and fall rates, in output units per second.
    pub const fn rates(mut self, rise: f32, fall: f32) -> Self {
        self.rise_rate = rise;
        self.fall_rate = fall;
        self
    }

    /// One acceleration for both directions, in output units per second
    /// squared. Only the S-curve shape uses it. Defaults to ten times the
    /// rate given to [`Ramp::builder`].
    pub const fn accel(self, accel: f32) -> Self {
        self.accels(accel, accel)
    }

    /// Independent rise and fall accelerations.
    pub const fn accels(mut self, rise: f32, fall: f32) -> Self {
        self.rise_accel = rise;
        self.fall_accel = fall;
        self
    }

    /// Output limits. The output and the effective target are clamped to
    /// them. Defaults to the whole finite `f32` range, which is no limit.
    pub const fn limits(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// The rate profile. Defaults to [`Shape::Linear`].
    pub const fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Validate and build the ramp, at rest on the initial value clamped to
    /// the limits, with the target equal to the initial value.
    ///
    /// # Errors
    ///
    /// Returns the first of: a non-finite initial value; a rate that is not
    /// finite and positive; an acceleration that is not finite and
    /// positive; limits that are NaN or inverted.
    pub fn build(self) -> Result<Ramp, Error> {
        if !self.initial.is_finite() {
            return Err(Error::Value);
        }
        if !(positive(self.rise_rate) && positive(self.fall_rate)) {
            return Err(Error::Rate);
        }
        if !(positive(self.rise_accel) && positive(self.fall_accel)) {
            return Err(Error::Accel);
        }
        if !ordered(self.min, self.max) {
            return Err(Error::Limits);
        }
        Ok(Ramp {
            value: clamp(self.initial, self.min, self.max),
            target: self.initial,
            rise_rate: self.rise_rate,
            fall_rate: self.fall_rate,
            rise_accel: self.rise_accel,
            fall_accel: self.fall_accel,
            min: self.min,
            max: self.max,
            vel: 0.0,
            shape: self.shape,
            enabled: true,
        })
    }
}

/// A ramp generator: moves an output toward a target at a bounded rate,
/// one caller-timed step at a time, in any unit.
///
/// There is no plan and no move clock. Every update works from the current
/// output and rate only, which is what lets any parameter change between
/// updates without restarting the move.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    value: f32,
    target: f32,
    rise_rate: f32,
    fall_rate: f32,
    rise_accel: f32,
    fall_accel: f32,
    min: f32,
    max: f32,
    vel: f32,
    shape: Shape,
    enabled: bool,
}

impl Ramp {
    /// Start building a ramp that begins at `initial` and moves at `rate`
    /// output units per second in both directions.
    pub const fn builder(initial: f32, rate: f32) -> Builder {
        Builder {
            initial,
            rise_rate: rate,
            fall_rate: rate,
            rise_accel: rate * 10.0,
            fall_accel: rate * 10.0,
            min: f32::MIN,
            max: f32::MAX,
            shape: Shape::Linear,
        }
    }

    /// The stored target clamped to the limits: what the ramp moves toward.
    fn effective_target(&self) -> f32 {
        clamp(self.target, self.min, self.max)
    }

    /// The greatest rate from which the ramp can still stop exactly on a
    /// target `dist` away without exceeding `accel`, given that each update
    /// applies its new rate for `dt`.
    fn brake_envelope(dist: f32, accel: f32, dt: f32) -> f32 {
        let adt = accel * dt;
        let root = sqrt(adt * adt + 8.0 * accel * dist);
        (4.0 * accel * dist) / (adt + root)
    }

    fn scurve_step(&mut self, target: f32, rate: f32, accel: f32, dt: f32) -> f32 {
        let diff = target - self.value;
        let dist = diff.abs();
        let adt = accel * dt;
        let want = if diff >= 0.0 { rate } else { -rate };
        let delta = clamp(want - self.vel, -adt, adt);
        let envelope = Self::brake_envelope(dist, accel, dt);
        let vel = clamp(self.vel + delta, -envelope, envelope);
        self.vel = vel;
        let next = self.value + vel * dt;
        let reached = if diff >= 0.0 {
            next >= target
        } else {
            next <= target
        };
        if reached {
            self.vel = 0.0;
            return target;
        }
        if next == self.value {
            // Below the output's resolution. If the envelope is what binds
            // the rate it can only fall further, so land; otherwise the
            // rate is still rising and will clear the floor, so hold.
            if vel.abs() >= envelope {
                self.vel = 0.0;
                return target;
            }
            return self.value;
        }
        next
    }

    fn linear_step(&mut self, target: f32, rate: f32, dt: f32) -> f32 {
        let diff = target - self.value;
        let step = rate * dt;
        if (diff > 0.0 && self.vel < 0.0) || (diff < 0.0 && self.vel > 0.0) {
            self.vel = 0.0;
        }
        // The carried budget is what rounding left unspent last update, so
        // a step below the output's resolution still moves on average.
        let budget = self.vel.abs() + step;
        if diff.abs() <= budget {
            self.vel = 0.0;
            return target;
        }
        let mut next = if diff > 0.0 {
            self.value + budget
        } else {
            self.value - budget
        };
        let mut applied = (next - self.value).abs();
        if applied > budget {
            next = next_toward(next, self.value);
            applied = (next - self.value).abs();
        }
        self.vel = if diff > 0.0 {
            budget - applied
        } else {
            applied - budget
        };
        next
    }

    /// Advance the output by one time step of `dt` seconds and return it.
    ///
    /// A disabled ramp holds. So does a `dt` that is NaN, infinite, zero or
    /// negative: a time step is a measurement, and a bad one must not move
    /// the output.
    pub fn update(&mut self, dt: f32) -> f32 {
        if !self.enabled || !positive(dt) {
            return self.value;
        }
        let target = self.effective_target();
        let rising = target >= self.value;
        let rate = if rising {
            self.rise_rate
        } else {
            self.fall_rate
        };
        self.value = match self.shape {
            Shape::SCurve => {
                let accel = if rising {
                    self.rise_accel
                } else {
                    self.fall_accel
                };
                self.scurve_step(target, rate, accel, dt)
            }
            Shape::Linear => self.linear_step(target, rate, dt),
        };
        self.value
    }

    /// Set the target. It is stored unclamped, so widening the limits later
    /// recovers the original intent; the ramp moves toward the target
    /// clamped to the limits.
    ///
    /// # Errors
    ///
    /// A NaN target is rejected. An infinite target is accepted and moves
    /// the ramp to the corresponding limit.
    pub fn set_target(&mut self, target: f32) -> Result<(), Error> {
        if target.is_nan() {
            return Err(Error::Target);
        }
        self.target = target;
        Ok(())
    }

    /// One rate for both directions.
    ///
    /// # Errors
    ///
    /// The rate must be finite and positive.
    pub fn set_rate(&mut self, rate: f32) -> Result<(), Error> {
        self.set_rates(rate, rate)
    }

    /// Independent rise and fall rates.
    ///
    /// # Errors
    ///
    /// Both rates must be finite and positive.
    pub fn set_rates(&mut self, rise: f32, fall: f32) -> Result<(), Error> {
        if !(positive(rise) && positive(fall)) {
            return Err(Error::Rate);
        }
        self.rise_rate = rise;
        self.fall_rate = fall;
        Ok(())
    }

    /// One acceleration for both directions.
    ///
    /// # Errors
    ///
    /// The acceleration must be finite and positive.
    pub fn set_accel(&mut self, accel: f32) -> Result<(), Error> {
        self.set_accels(accel, accel)
    }

    /// Independent rise and fall accelerations.
    ///
    /// # Errors
    ///
    /// Both accelerations must be finite and positive.
    pub fn set_accels(&mut self, rise: f32, fall: f32) -> Result<(), Error> {
        if !(positive(rise) && positive(fall)) {
            return Err(Error::Accel);
        }
        self.rise_accel = rise;
        self.fall_accel = fall;
        Ok(())
    }

    /// Set the output limits. If the output lies outside the new limits it
    /// is clamped at once and the rate is set to zero, because it was moved
    /// rather than ramped.
    ///
    /// # Errors
    ///
    /// The limits must not be NaN and `min` must not exceed `max`.
    pub fn set_limits(&mut self, min: f32, max: f32) -> Result<(), Error> {
        if !ordered(min, max) {
            return Err(Error::Limits);
        }
        self.min = min;
        self.max = max;
        let clamped = clamp(self.value, min, max);
        if clamped != self.value {
            self.value = clamped;
            self.vel = 0.0;
        }
        Ok(())
    }

    /// Change the rate profile. Switching to the S-curve seeds it with the
    /// rate the linear ramp was running at, so the rate is continuous.
    /// Switching to linear resumes at the full configured rate.
    pub fn set_shape(&mut self, shape: Shape) {
        match (self.shape, shape) {
            (Shape::Linear, Shape::SCurve) => self.vel = self.rate(),
            (Shape::SCurve, Shape::Linear) => self.vel = 0.0,
            _ => {}
        }
        self.shape = shape;
    }

    /// Enable or disable updates. Disabling sets the rate to zero and holds
    /// the output; enabling resumes the move from rest.
    pub const fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.vel = 0.0;
        }
        self.enabled = enabled;
    }

    /// Place the output at `value`, clamped to the limits, at rest, with the
    /// target equal to `value`.
    ///
    /// # Errors
    ///
    /// The value must be finite.
    pub fn reset(&mut self, value: f32) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::Value);
        }
        self.value = clamp(value, self.min, self.max);
        self.target = value;
        self.vel = 0.0;
        Ok(())
    }

    /// The current output.
    #[must_use]
    pub const fn output(&self) -> f32 {
        self.value
    }

    /// The stored, unclamped target.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// The rate the output is moving at, signed, in output units per
    /// second: zero when disabled or at the target, the signed configured
    /// rate for a linear move, and the current rate for an S-curve move.
    #[must_use]
    pub fn rate(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        if self.shape == Shape::SCurve {
            return self.vel;
        }
        let target = self.effective_target();
        if self.value == target {
            0.0
        } else if target > self.value {
            self.rise_rate
        } else {
            -self.fall_rate
        }
    }

    /// Whether the output equals the effective target.
    #[must_use]
    pub fn at_target(&self) -> bool {
        self.value == self.effective_target()
    }

    /// What the ramp is doing.
    #[must_use]
    pub fn state(&self) -> State {
        if !self.enabled {
            State::Disabled
        } else if self.at_target() {
            State::AtTarget
        } else {
            State::Moving
        }
    }

    /// Whether updates move the output.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// The rate profile.
    #[must_use]
    pub const fn shape(&self) -> Shape {
        self.shape
    }

    /// The rise and fall rates.
    #[must_use]
    pub const fn rates(&self) -> (f32, f32) {
        (self.rise_rate, self.fall_rate)
    }

    /// The rise and fall accelerations.
    #[must_use]
    pub const fn accels(&self) -> (f32, f32) {
        (self.rise_accel, self.fall_accel)
    }

    /// The output limits.
    #[must_use]
    pub const fn limits(&self) -> (f32, f32) {
        (self.min, self.max)
    }
}

#[cfg(doctest)]
#[doc = include_str!("../docs/integration.md")]
mod doc_integration {}

#[cfg(doctest)]
#[doc = include_str!("../docs/design.md")]
mod doc_design {}
