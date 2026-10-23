// SPDX-License-Identifier: MIT
//! One test per rule in `docs/design.md`, named for the rule it falsifies.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    reason = "exact landing is the contract under test, and update counts are far below f32's mantissa"
)]

use ramper::{Error, Ramp, Shape, State};

const DT: f32 = 0.001;

/// Updates until the ramp is at its target, bounded so a stalled ramp fails
/// the test rather than hanging it.
fn run_to_target(ramp: &mut Ramp, dt: f32) -> usize {
    for n in 0..1_000_000 {
        if ramp.at_target() {
            return n;
        }
        ramp.update(dt);
    }
    panic!("ramp did not reach its target");
}

fn linear() -> Ramp {
    Ramp::builder(0.0, 50.0).build().unwrap()
}

fn seconds(updates: usize) -> f32 {
    updates as f32 * DT
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

#[test]
fn builder_defaults_and_accessors() {
    let ramp = Ramp::builder(3.0, 50.0).build().unwrap();
    assert_eq!(ramp.output(), 3.0);
    assert_eq!(ramp.target(), 3.0);
    assert_eq!(ramp.rates(), (50.0, 50.0));
    assert_eq!(ramp.accels(), (500.0, 500.0));
    assert_eq!(ramp.limits(), (f32::MIN, f32::MAX));
    assert_eq!(ramp.shape(), Shape::Linear);
    assert_eq!(Shape::default(), Shape::Linear);
    assert!(ramp.enabled());
    assert!(ramp.at_target());
    assert_eq!(ramp.state(), State::AtTarget);
    assert_eq!(ramp.rate(), 0.0);
    let custom = Ramp::builder(0.0, 1.0)
        .rates(2.0, 3.0)
        .accels(4.0, 5.0)
        .limits(-6.0, 7.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    assert_eq!(custom.rates(), (2.0, 3.0));
    assert_eq!(custom.accels(), (4.0, 5.0));
    assert_eq!(custom.limits(), (-6.0, 7.0));
    assert_eq!(custom.shape(), Shape::SCurve);
    assert_eq!(
        Ramp::builder(0.0, 1.0).accel(9.0).build().unwrap().accels(),
        (9.0, 9.0)
    );
}

#[test]
fn build_clamps_the_initial_value_to_the_limits() {
    let ramp = Ramp::builder(900.0, 50.0)
        .limits(0.0, 800.0)
        .build()
        .unwrap();
    assert_eq!(ramp.output(), 800.0);
    assert_eq!(ramp.target(), 900.0);
    assert!(ramp.at_target());
}

#[test]
fn build_rejects_a_bad_configuration_in_order() {
    assert_eq!(Ramp::builder(f32::NAN, 0.0).build(), Err(Error::Value));
    assert_eq!(Ramp::builder(f32::INFINITY, 1.0).build(), Err(Error::Value));
    for rate in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(Ramp::builder(0.0, rate).build(), Err(Error::Rate), "{rate}");
        assert_eq!(
            Ramp::builder(0.0, 1.0).rates(1.0, rate).build(),
            Err(Error::Rate),
            "{rate}"
        );
        assert_eq!(
            Ramp::builder(0.0, 1.0).accels(1.0, rate).build(),
            Err(Error::Accel),
            "{rate}"
        );
        assert_eq!(
            Ramp::builder(0.0, 1.0).accel(rate).build(),
            Err(Error::Accel),
            "{rate}"
        );
    }
    assert_eq!(
        Ramp::builder(0.0, 1.0).limits(1.0, 0.0).build(),
        Err(Error::Limits)
    );
    assert_eq!(
        Ramp::builder(0.0, 1.0).limits(f32::NAN, 0.0).build(),
        Err(Error::Limits)
    );
    assert_eq!(
        Ramp::builder(0.0, 1.0).limits(0.0, f32::NAN).build(),
        Err(Error::Limits)
    );
    assert!(
        Ramp::builder(0.0, 1.0)
            .limits(f32::NEG_INFINITY, f32::INFINITY)
            .build()
            .is_ok()
    );
}

#[test]
fn live_setters_reject_bad_values_and_keep_the_old_ones() {
    let mut ramp = linear();
    assert_eq!(ramp.set_target(f32::NAN), Err(Error::Target));
    assert_eq!(ramp.target(), 0.0);
    assert_eq!(ramp.set_rate(0.0), Err(Error::Rate));
    assert_eq!(ramp.set_rates(1.0, f32::NAN), Err(Error::Rate));
    assert_eq!(ramp.rates(), (50.0, 50.0));
    assert_eq!(ramp.set_accel(-1.0), Err(Error::Accel));
    assert_eq!(ramp.set_accels(f32::INFINITY, 1.0), Err(Error::Accel));
    assert_eq!(ramp.accels(), (500.0, 500.0));
    assert_eq!(ramp.set_limits(1.0, -1.0), Err(Error::Limits));
    assert_eq!(ramp.set_limits(f32::NAN, 1.0), Err(Error::Limits));
    assert_eq!(ramp.limits(), (f32::MIN, f32::MAX));
    assert_eq!(ramp.reset(f32::NAN), Err(Error::Value));
    assert_eq!(ramp.output(), 0.0);
    assert_eq!(ramp.set_rate(7.0), Ok(()));
    assert_eq!(ramp.rates(), (7.0, 7.0));
    assert_eq!(ramp.set_accel(8.0), Ok(()));
    assert_eq!(ramp.accels(), (8.0, 8.0));
}

#[test]
fn an_infinite_target_moves_the_ramp_to_the_limit() {
    let mut ramp = Ramp::builder(0.0, 50.0).limits(0.0, 10.0).build().unwrap();
    ramp.set_target(f32::INFINITY).unwrap();
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.output(), 10.0);
}

#[test]
fn linear_move_takes_distance_over_rate_and_lands_exactly() {
    let mut ramp = linear();
    ramp.set_target(400.0).unwrap();
    assert_eq!(ramp.state(), State::Moving);
    let updates = run_to_target(&mut ramp, DT);
    assert_close(seconds(updates), 8.0, 0.01);
    assert_eq!(ramp.output(), 400.0);
    assert_eq!(ramp.state(), State::AtTarget);
}

#[test]
fn linear_move_never_overshoots() {
    let mut ramp = Ramp::builder(0.0, 30.0).build().unwrap();
    ramp.set_target(1.0).unwrap();
    let mut previous = 0.0;
    while !ramp.at_target() {
        let out = ramp.update(0.0077);
        assert!(out <= 1.0 && out >= previous);
        previous = out;
    }
}

#[test]
fn asymmetric_rates_apply_per_direction() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .rates(50.0, 2000.0)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    assert_eq!(ramp.rate(), 50.0);
    run_to_target(&mut ramp, DT);
    ramp.set_target(0.0).unwrap();
    assert_eq!(ramp.rate(), -2000.0);
    let updates = run_to_target(&mut ramp, DT);
    assert_close(seconds(updates), 0.2, 0.005);
    assert_eq!(ramp.output(), 0.0);
}

#[test]
fn linear_rate_steps_to_the_full_rate_and_back_to_zero() {
    let mut ramp = linear();
    assert_eq!(ramp.rate(), 0.0);
    ramp.set_target(1.0).unwrap();
    assert_eq!(ramp.rate(), 50.0);
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.rate(), 0.0);
}

#[test]
fn a_reversal_mid_move_drops_the_carried_budget() {
    let mut ramp = Ramp::builder(1.0e6, 1.0).build().unwrap();
    ramp.set_target(1.0e6 + 10.0).unwrap();
    for _ in 0..10 {
        ramp.update(DT);
    }
    ramp.set_target(1.0e6 - 10.0).unwrap();
    let before = ramp.output();
    let after = ramp.update(DT);
    assert!(after <= before);
    assert!(before - after <= 0.0625 + 0.001);
}

#[test]
fn the_effective_target_is_the_target_clamped_to_the_limits() {
    let mut ramp = Ramp::builder(0.0, 1000.0)
        .limits(0.0, 500.0)
        .build()
        .unwrap();
    ramp.set_target(1000.0).unwrap();
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.output(), 500.0);
    assert_eq!(ramp.target(), 1000.0);
    assert_eq!(ramp.state(), State::AtTarget);
    // Widening the limits recovers the original intent.
    ramp.set_limits(0.0, 2000.0).unwrap();
    assert_eq!(ramp.state(), State::Moving);
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.output(), 1000.0);
}

#[test]
fn narrowing_the_limits_moves_the_output_and_zeroes_the_rate() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    for _ in 0..1000 {
        ramp.update(DT);
    }
    assert!(ramp.rate() > 0.0);
    let before = ramp.output();
    ramp.set_limits(0.0, before / 2.0).unwrap();
    assert_eq!(ramp.output(), before / 2.0);
    assert_eq!(ramp.rate(), 0.0);
    assert!(ramp.at_target());
    // Limits that still contain the output leave it and its rate alone.
    ramp.set_limits(0.0, 400.0).unwrap();
    ramp.update(DT);
    let rate = ramp.rate();
    ramp.set_limits(-1.0, 401.0).unwrap();
    assert_eq!(ramp.rate(), rate);
}

#[test]
fn reset_places_the_output_at_rest_within_the_limits() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .limits(0.0, 100.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(100.0).unwrap();
    ramp.update(DT);
    ramp.reset(250.0).unwrap();
    assert_eq!(ramp.output(), 100.0);
    assert_eq!(ramp.target(), 250.0);
    assert_eq!(ramp.rate(), 0.0);
    assert!(ramp.at_target());
}

#[test]
fn a_disabled_ramp_holds_and_reports_no_rate() {
    let mut ramp = linear();
    ramp.set_target(400.0).unwrap();
    ramp.update(DT);
    let held = ramp.output();
    ramp.set_enabled(false);
    assert!(!ramp.enabled());
    assert_eq!(ramp.state(), State::Disabled);
    assert_eq!(ramp.rate(), 0.0);
    assert_eq!(ramp.update(DT), held);
    ramp.set_enabled(true);
    assert!(ramp.update(DT) > held);
}

#[test]
fn disabling_an_scurve_move_zeroes_its_rate() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    for _ in 0..100 {
        ramp.update(DT);
    }
    ramp.set_enabled(false);
    ramp.set_enabled(true);
    assert_eq!(ramp.rate(), 0.0);
}

#[test]
fn a_bad_time_step_holds_the_output() {
    let mut ramp = linear();
    ramp.set_target(400.0).unwrap();
    for dt in [0.0, -0.001, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(ramp.update(dt), 0.0, "{dt}");
    }
    assert!(ramp.update(DT) > 0.0);
}

#[test]
fn linear_steps_below_the_output_resolution_still_move_on_average() {
    // At 1e6 the spacing of f32 values is 0.0625; a step of 0.001 would
    // round to nothing without the carried budget.
    let mut ramp = Ramp::builder(1.0e6, 1.0).build().unwrap();
    ramp.set_target(1.0e6 + 10.0).unwrap();
    for _ in 0..1000 {
        ramp.update(DT);
    }
    assert_close(ramp.output() - 1.0e6, 1.0, 0.13);
    let updates = run_to_target(&mut ramp, DT);
    assert_close(seconds(updates), 9.0, 0.2);
    assert_eq!(ramp.output(), 1.0e6 + 10.0);
}

#[test]
fn scurve_trapezoid_move_takes_distance_over_rate_plus_rate_over_accel() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    let updates = run_to_target(&mut ramp, DT);
    assert_close(seconds(updates), 8.25, 0.01);
    assert_eq!(ramp.output(), 400.0);
}

#[test]
fn scurve_triangle_move_takes_twice_root_distance_over_accel() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(4.0).unwrap();
    let updates = run_to_target(&mut ramp, DT);
    assert_close(seconds(updates), 2.0 * (4.0f32 / 200.0).sqrt(), 0.005);
    assert_eq!(ramp.output(), 4.0);
}

#[test]
fn asymmetric_accelerations_give_the_documented_durations() {
    let cases = [
        ((100.0, 100.0), 8.50, 4.00),
        ((20_000.0, 20_000.0), 8.00, 0.30),
        ((100.0, 40_000.0), 8.50, 0.25),
    ];
    for ((rise_accel, fall_accel), rise_s, trip_s) in cases {
        let mut ramp = Ramp::builder(0.0, 50.0)
            .rates(50.0, 2000.0)
            .accels(rise_accel, fall_accel)
            .shape(Shape::SCurve)
            .build()
            .unwrap();
        ramp.set_target(400.0).unwrap();
        assert_close(seconds(run_to_target(&mut ramp, DT)), rise_s, 0.01);
        ramp.set_target(0.0).unwrap();
        assert_close(seconds(run_to_target(&mut ramp, DT)), trip_s, 0.01);
        assert_eq!(ramp.output(), 0.0);
    }
}

#[test]
fn scurve_rate_never_exceeds_the_limit_and_changes_by_at_most_one_step() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    let mut previous_rate = 0.0f32;
    let mut previous_out = 0.0f32;
    while !ramp.at_target() {
        let out = ramp.update(DT);
        let rate = ramp.rate();
        assert!(rate <= 50.0 * (1.0 + 1e-5));
        assert!((rate - previous_rate).abs() <= 2.0 * 200.0 * DT);
        assert!(out >= previous_out && out <= 400.0);
        previous_rate = rate;
        previous_out = out;
    }
    assert_eq!(ramp.rate(), 0.0);
}

#[test]
fn scurve_rate_is_continuous_across_a_target_reversal() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    for _ in 0..2000 {
        ramp.update(DT);
    }
    let before = ramp.rate();
    assert_close(before, 50.0, 0.01);
    ramp.set_target(-400.0).unwrap();
    ramp.update(DT);
    assert_close(ramp.rate(), before - 200.0 * DT, 1e-4);
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.output(), -400.0);
}

#[test]
fn scurve_rate_is_continuous_across_a_rate_change() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    for _ in 0..2000 {
        ramp.update(DT);
    }
    ramp.set_rate(10.0).unwrap();
    ramp.update(DT);
    assert_close(ramp.rate(), 50.0 - 200.0 * DT, 1e-3);
}

#[test]
fn a_target_inside_the_braking_distance_stops_on_it_without_overshoot() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .accel(200.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    for _ in 0..4000 {
        ramp.update(DT);
    }
    let here = ramp.output();
    let target = here + 0.5;
    ramp.set_target(target).unwrap();
    // At 50 units/s the move would take 10 updates; the envelope brakes
    // harder than the acceleration limit allows and stops on the target.
    let mut updates = 0;
    while !ramp.at_target() {
        assert!(ramp.update(DT) <= target);
        updates += 1;
        assert!(updates <= 100);
    }
    assert!(updates > 10, "{updates}");
    assert_eq!(ramp.output(), target);
}

#[test]
fn switching_linear_to_scurve_keeps_the_rate() {
    let mut ramp = linear();
    ramp.set_target(400.0).unwrap();
    ramp.update(DT);
    ramp.set_shape(Shape::SCurve);
    assert_eq!(ramp.shape(), Shape::SCurve);
    assert_eq!(ramp.rate(), 50.0);
    ramp.update(DT);
    assert_close(ramp.rate(), 50.0, 1e-4);
}

#[test]
fn switching_scurve_to_linear_resumes_at_the_full_rate() {
    let mut ramp = Ramp::builder(0.0, 50.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(400.0).unwrap();
    ramp.update(DT);
    assert!(ramp.rate() < 50.0);
    ramp.set_shape(Shape::Linear);
    assert_eq!(ramp.rate(), 50.0);
    ramp.set_shape(Shape::Linear);
    assert_eq!(ramp.rate(), 50.0);
    ramp.set_shape(Shape::SCurve);
    ramp.set_shape(Shape::SCurve);
    assert_eq!(ramp.rate(), 50.0);
}

#[test]
fn scurve_lands_when_the_envelope_binds_below_the_output_resolution() {
    // One f32 step away from the target at 1e6, the braking envelope is
    // reached while each step still rounds to no movement, so the ramp
    // lands rather than stalling.
    let target = 1.0e6 + 0.0625;
    let mut ramp = Ramp::builder(1.0e6, 5.0)
        .accel(10.0)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(target).unwrap();
    let updates = run_to_target(&mut ramp, DT);
    assert!(updates > 1 && updates < 500, "{updates}");
    assert_eq!(ramp.output(), target);
}

#[test]
fn scurve_holds_below_the_output_resolution_while_the_rate_still_rises() {
    let mut ramp = Ramp::builder(1.0e6, 1000.0)
        .accel(1.0e4)
        .shape(Shape::SCurve)
        .build()
        .unwrap();
    ramp.set_target(1.0e6 + 100.0).unwrap();
    // The rate rises by 10 per update; a step clears the 0.0625 spacing at
    // 1e6 once the rate passes about 31, on the fourth update.
    assert_eq!(ramp.update(DT), 1.0e6);
    assert!(ramp.rate() > 0.0);
    let mut held = 1;
    while ramp.update(DT) == 1.0e6 {
        held += 1;
        assert!(held < 10);
    }
    assert_eq!(held, 3);
    run_to_target(&mut ramp, DT);
    assert_eq!(ramp.output(), 1.0e6 + 100.0);
}

#[test]
fn errors_display_and_implement_error() {
    let errors = [
        (Error::Value, "value is not finite"),
        (Error::Rate, "rate is not finite and positive"),
        (Error::Accel, "acceleration is not finite and positive"),
        (Error::Limits, "limits are not ordered finite values"),
        (Error::Target, "target is NaN"),
    ];
    for (error, text) in errors {
        assert_eq!(error.to_string(), text);
        let dynamic: &dyn core::error::Error = &error;
        assert!(dynamic.source().is_none());
    }
}

#[test]
fn types_are_plain_data() {
    let ramp = linear();
    let copy = ramp.clone();
    assert_eq!(ramp, copy);
    assert!(format!("{ramp:?}").contains("Ramp"));
    assert!(format!("{:?}", Ramp::builder(0.0, 1.0)).contains("Builder"));
    assert!(format!("{:?}", State::Moving).contains("Moving"));
}
