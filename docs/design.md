# Design

`ramper` moves one `f32` output toward a target at a bounded rate, one caller-timed step at a time, in any unit. A linear profile moves at a constant rate. An S-curve profile also bounds how fast the rate changes, so the output eases in and out of a move. Either way there is no plan and no move clock: every update works from the current output and rate only, which is what lets any parameter change between updates without restarting the move.

## Goals

- Move one output toward a target at a bounded rate, one caller-timed step at a time, in any unit.
- Keep the output rate continuous when the target, rates, limits or profile change in the middle of a move, so a live setpoint can drive the ramp directly.
- Never overshoot the target, never leave the output limits, and land exactly on the target.
- Reject a bad configuration when it is set, and hold the output on a bad time step, so an update has no error path.
- Do bounded, constant work per update, with caller-owned storage and no allocation.

## Non-goals

- **Time source.** The caller measures time and passes the step.
- **Arbitrary profiles.** Only linear and S-curve shapes are provided; exponential or table-driven shaping belongs in the application.
- **Jerk limiting.** The S-curve bounds the rate and its rate of change, not the change in acceleration.
- **Thread safety.** A ramp shared across threads or with an interrupt handler needs the caller's mutual exclusion.
- **Persistence.** Saving and restoring ramp state is the application's concern. A `Ramp` is `Clone`.

## State

A `Ramp` holds its output, the unclamped target, per-direction rates and accelerations, the output limits, the shape, the enabled flag, and the output rate `vel`.

The **effective target** is the stored target clamped to the output limits. Every update moves toward it, and `at_target` and `state` compare against it. Keeping the stored target unclamped means that widening the limits later recovers the original intent. The direction of the move, toward the effective target, selects which rate and acceleration apply, including through a reversal.

## Linear profile

The linear profile moves at the configured rate for the direction of travel until it reaches the effective target, then stops. The rate steps from zero to the full rate at the start of a move and back to zero at the end.

When `rate * dt` is below the resolution of the output, a plain `value += rate * dt` would round to no movement at all and the ramp would stall. The profile instead adds each update's movement to a budget and moves the output by the whole budget. When rounding to a representable `f32` would spend more than the budget, it steps back one value toward the current output, and carries the unspent fraction to the next update. The visible output may hold for several updates, but the average rate over many updates is the configured rate and the move lands exactly.

## S-curve profile

A linear move steps its rate at both ends. For a physical actuator (a motor, a valve, a power supply) those steps are impulsive: they excite mechanical resonance, drive current into inductive loads, or trip a soft limit. The S-curve profile also bounds how fast the rate itself changes. The rate starts at zero, rises to the configured rate, holds through the middle of the move, and falls back to zero.

The cost is time. A move long enough to reach the configured rate takes `rate / accel` seconds longer than the linear move: half easing in and half easing out. With distance `d`, rate `r` and acceleration `a`, a move long enough to reach `r` takes `d / r + r / a`; a shorter move reaches only `sqrt(a d)` and takes `2 sqrt(d / a)`. The changeover is at `d = r² / a`. A move from 0 to 400 at 50 units/s and 200 units/s² takes 8.25 s, against 8 s for the linear move.

### Asymmetric legs

A rise and a fall often have different jobs: a DC bus is eased up under control and tripped down on a fault. Asymmetric rates alone are not enough, because a single acceleration limit then serves both legs: one gentle enough for the rise makes the trip take seconds, and one fast enough for the trip makes the rise step rather than ease. Setting the accelerations per direction resolves it. For a 400 V bus at 50 V/s up and 2000 V/s down:

| Configuration | Rise 0 to 400 | Trip 400 to 0 |
| --- | --- | --- |
| Symmetric acceleration 100 V/s² | 8.50 s | 4.00 s |
| Symmetric acceleration 20000 V/s² | 8.00 s | 0.30 s |
| 100 V/s² up, 40000 V/s² down | 8.50 s | 0.25 s |
| Linear | 8.00 s | 0.20 s |

### Update steps

The rate and acceleration below are the pair for the direction of the move. Each update does three things.

1. **Approach the cruise rate.** The rate moves toward the rate limit, in the direction of the target, by at most one acceleration step `accel * dt`.
2. **Stay inside the braking envelope.** The rate is clamped to the greatest speed from which the ramp can still stop exactly on the target without exceeding the acceleration limit.
3. **Advance the output.** `value += vel * dt`, snapping to the target if the step reaches or passes it.

### Braking envelope

The update applies the new rate to the value within the same tick. A ramp travelling at `v` that decelerates at `a` from here on covers `v² / (2a) + v dt / 2` before it stops. The envelope is the inverse: the greatest `v` whose stopping distance fits in the distance remaining, `d`. The positive root of `v² + a dt v - 2 a d = 0`, rationalised to avoid the cancellation the direct form suffers as `d` approaches zero, is `4 a d / (a dt + sqrt((a dt)² + 8 a d))`. The denominator is at least `2 a dt`, so the expression is defined for any positive `a` and `dt` and needs no special case at `d = 0`.

### Clamp, not target

Using the envelope as the rate to aim for, with the acceleration limiter walking toward it, is unstable at the end of a move: the tracking error grows without bound as the envelope falls to zero, and `f32` rounding alone introduces an error. Clamping removes the feedback. The state is always on or inside the envelope, so there is no error to accumulate, and the envelope only ever reduces the rate.

### Landing

A move ends in one of two ways. Normally the step reaches or passes the target and the ramp snaps to it, which is what makes `at_target` exact.

The second case is numerical. Once a step falls below the resolution of the output, the value stops changing however many updates pass. When the envelope is the constraint binding the rate, the rate can only fall further and the target can never be reached by accumulation, so the ramp lands on it. When another limit is binding, the rate is still rising and will clear the resolution floor, so the ramp holds and carries on.

### Guarantees

For any rate, acceleration and `dt` greater than zero, and whatever the caller does to the target, rates, limits or profile between updates:

- The output rate never exceeds the rate limit for its direction.
- The output never overshoots the effective target and never leaves the limits.
- The output rate is continuous across a change of target, rate, acceleration or limits, and across a switch from the linear profile.
- The output arrives exactly on the effective target.
- The output rate changes by at most `accel * dt` per update in real arithmetic. In `f32`, the envelope clamp rounds past this in the last few resolution steps before the target; the property tests assert twice `accel * dt` on every update over a randomised sweep of moves spanning rates of 0.5 to 1000, accelerations of 5 to 20000 and steps of 0.1 to 20 ms.
- On the update that arrives, the rate falls to zero from a small value. With `q` the spacing of `f32` values at the target, that value is bounded by 1.5 times the largest of `2 a dt`, `sqrt(2 a q)` and `q / dt`, and the sweep asserts it. The first term dominates unless `a dt²` is small next to `q`.

One exception applies to the acceleration bound. If the target moves to a point inside the current braking distance, no bounded deceleration can stop there. The generator prioritises not overshooting: it decelerates as hard as the envelope demands and stops on the target. An application that would rather overshoot and come back than exceed the acceleration limit should keep the target outside the braking distance `v² / (2a)`, which `rate` provides the inputs for.

### Profile changes

Switching from linear to S-curve seeds the S-curve with the rate the linear ramp was running at, so the rate is continuous. Switching from S-curve to linear resumes at the full configured rate, which is the step every linear move has. `reset`, disabling, and a limit change that displaces the output all set the rate to zero, because the output is then at rest or was moved rather than ramped.

## Decisions

### Configuration is validated when set

The C reference treats setter preconditions as contracts and has `update` hold the output when it finds a non-positive rate or acceleration. Here `Builder::build` and every live setter return `Result`, so a rate of zero, a NaN limit or an inverted pair is an error at the call that made it, and `update` has no configuration check at all. `set_target` is the hot path and rejects only NaN; an infinite target is accepted and moves the ramp to a limit.

### A bad time step holds

A time step is a measurement, not a configuration: two ticks with the same timestamp give zero, and a clock glitch gives a negative or absurd value. Holding the output for a step that is not finite and positive is the right control behaviour, so `update` does that rather than return an error the control loop would have to handle every tick.

### A ramp has no default rate

The C reference starts every ramp at a configured default rate. A ramp without a rate is a trap, so `Ramp::builder` takes the rate with the initial value and derives the default acceleration from it. The default limits are the whole finite `f32` range, so clamping is opt-in.

### Square root from `std` or `libm`

The S-curve envelope needs a square root, which `core` does not provide on the minimum supported Rust. The default `std` feature uses the standard library; the `libm` feature uses the `libm` crate and is the only dependency the crate can have. A fixed-iteration Newton step in `core` would remove the dependency, and may replace `libm` once a measured error bound exists for it.

## Numerical range

The output, target, rates and accelerations are `f32`. Intermediate products such as `8 * accel * dist` and `rate * dt` must stay finite: with the product of acceleration and distance below about 4 × 10³⁷ the envelope is well defined. The linear profile's budget is an `f32` too, so a rate below the resolution of the budget at the output's magnitude cannot be carried. Keep values in units that leave them a few orders of magnitude inside `f32`.

## Cost

An update is a few compares, multiplies and adds, plus one square root for the S-curve. There are no loops and no work that depends on the distance or the rate. A `Ramp` is nine `f32` values, a shape and a flag.

## Verification

- `tests/contract.rs` holds one test per rule in this document, named for the rule it falsifies, including every `Error` variant and the move durations in the table above.
- `tests/properties.rs` holds the randomised sweep: for moves spanning rates of 0.5 to 1000, accelerations of 5 to 20000 and steps of 0.1 to 20 ms, every update keeps the rate within its limit, changes it by at most twice `accel * dt`, never overshoots, never leaves the limits, and lands exactly.
- Doc examples in the README and the guides compile and run as doctests.
- CI gates on `cargo fmt`, clippy pedantic with warnings denied, `cargo doc` with warnings denied, the stable and minimum toolchains with both the `std` and `libm` features, a `no_std` target build, `cargo deny`, `cargo audit` and 100% line and region coverage of `src/`.
