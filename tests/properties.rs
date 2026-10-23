// SPDX-License-Identifier: MIT
//! The randomised sweep from `docs/design.md`.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    reason = "exact landing is the contract under test, and update counts are far below f32's mantissa"
)]

use proptest::prelude::*;
use ramper::{Ramp, Shape};

const MAX_UPDATES: usize = 20_000;

fn expected_seconds(dist: f32, rate: f32, accel: f32, shape: Shape) -> f32 {
    match shape {
        Shape::Linear => dist / rate,
        Shape::SCurve if dist >= rate * rate / accel => dist / rate + rate / accel,
        Shape::SCurve => 2.0 * (dist / accel).sqrt(),
    }
}

fn shape() -> impl Strategy<Value = Shape> {
    prop_oneof![Just(Shape::Linear), Just(Shape::SCurve)]
}

proptest! {
    #[test]
    fn every_move_keeps_the_guarantees(
        start in -1000.0f32..1000.0,
        target in -1000.0f32..1000.0,
        rate in 0.5f32..1000.0,
        accel in 5.0f32..20_000.0,
        dt in 0.0001f32..0.02,
        shape in shape(),
    ) {
        let mut ramp = Ramp::builder(start, rate).accel(accel).shape(shape).build().unwrap();
        ramp.set_target(target).unwrap();
        let direction = (target - start).signum();
        let mut previous_out = start;
        let mut previous_rate = ramp.rate();
        let mut updates = 0;
        while !ramp.at_target() && updates < MAX_UPDATES {
            let out = ramp.update(dt);
            let current = ramp.rate();
            updates += 1;
            // Never overshoot: the remaining distance keeps its sign.
            prop_assert!((target - out) * direction >= 0.0, "overshoot at {out}");
            // Never retreat.
            prop_assert!((out - previous_out) * direction >= 0.0, "retreat");
            // Rate within its limit, in the direction of travel.
            prop_assert!(current.abs() <= rate * (1.0 + 1e-5), "rate {current} > {rate}");
            prop_assert!(current * direction >= 0.0);
            if shape == Shape::SCurve {
                let step = (current - previous_rate).abs();
                let accel_step = 2.0 * accel * dt;
                if ramp.at_target() {
                    // On the update that arrives the rate falls to zero from
                    // a small value bounded by the acceleration step, the
                    // envelope at one resolution step, or one step per dt.
                    let q = target.abs() * f32::EPSILON;
                    let arrival = accel_step.max((2.0 * accel * q).sqrt()).max(q / dt);
                    prop_assert!(step <= 1.5 * arrival, "arrival step {step} > {}", 1.5 * arrival);
                } else {
                    prop_assert!(step <= accel_step * (1.0 + 1e-4), "rate step {step} > {accel_step}");
                }
            }
            previous_out = out;
            previous_rate = current;
        }
        let expected = expected_seconds((target - start).abs(), rate, accel, shape);
        let budget = expected / dt + 5.0 + expected / dt * 0.01;
        if budget < MAX_UPDATES as f32 {
            prop_assert!(ramp.at_target(), "not at target after {updates} updates, expected about {}", expected / dt);
            prop_assert_eq!(ramp.output(), target);
            prop_assert_eq!(ramp.rate(), 0.0);
            prop_assert!(updates as f32 <= budget, "{updates} updates, budget {budget}");
        }
    }

    #[test]
    fn limits_are_never_left_and_a_clamped_target_is_landed_on(
        start in -1000.0f32..1000.0,
        target in -2000.0f32..2000.0,
        rate in 10.0f32..1000.0,
        accel in 100.0f32..20_000.0,
        dt in 0.001f32..0.02,
        shape in shape(),
    ) {
        let mut ramp = Ramp::builder(start, rate).accel(accel).limits(-500.0, 500.0).shape(shape).build().unwrap();
        prop_assert!((-500.0..=500.0).contains(&ramp.output()));
        ramp.set_target(target).unwrap();
        let effective = target.clamp(-500.0, 500.0);
        // Up to 1000 units at 10 units/s and 1 ms steps is 100 000 updates.
        let mut updates = 0;
        while !ramp.at_target() && updates < 250_000 {
            let out = ramp.update(dt);
            prop_assert!((-500.0..=500.0).contains(&out));
            updates += 1;
        }
        prop_assert!(ramp.at_target());
        prop_assert_eq!(ramp.output(), effective);
    }

    #[test]
    fn a_live_setpoint_never_makes_the_output_cross_its_target(
        start in -100.0f32..100.0,
        targets in prop::collection::vec(-100.0f32..100.0, 1..20),
        rate in 1.0f32..100.0,
        accel in 10.0f32..2000.0,
        dt in 0.001f32..0.01,
        shape in shape(),
    ) {
        let mut ramp = Ramp::builder(start, rate).accel(accel).shape(shape).build().unwrap();
        for target in &targets {
            ramp.set_target(*target).unwrap();
            for _ in 0..50 {
                let before = ramp.output();
                let after = ramp.update(dt);
                let direction = (target - before).signum();
                prop_assert!((target - after) * direction >= 0.0, "crossed {target}: {before} -> {after}");
            }
        }
        // The longest move is 200 units at 1 unit/s, or 200 000 updates at
        // the smallest step, so give the landing enough room.
        let last = targets[targets.len() - 1];
        let mut updates = 0;
        while !ramp.at_target() && updates < 250_000 {
            ramp.update(dt);
            updates += 1;
        }
        prop_assert_eq!(ramp.output(), last);
    }
}
