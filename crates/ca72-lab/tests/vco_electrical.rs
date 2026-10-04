//! Board 1's oscillator against its drawing: operating points and waveform levels
//! (Figure 9-3's annotations, section 2.16's text), and the factory calibration.

use ca72_lab::bench::{Solver, Supplies};
use ca72_lab::vco::{self, Controls, KEY_STEP, LOW_A, Procedure, Range, Trims};
use ca72_lab::work_dir;
use ca72_spice::for_test;

fn within(what: &str, got: f64, want: f64, tol: f64) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: {got:.4} (drawing: {want} +- {tol})"
    );
}

/// With the ramp held at -2 V (the loop broken), the DC levels the drawing and text give.
#[test]
fn operating_point_matches_the_drawing() {
    let Some(spice) = for_test("operating_point_matches_the_drawing") else {
        return;
    };
    let ctl = Controls {
        kbd: LOW_A,
        range: Range::R2,
        ..Controls::default()
    };
    let net = vco::netlist(
        &Trims::default(),
        &ctl,
        Supplies::default(),
        Solver::default(),
    );
    let held = format!("{net}vhold ramp 0 -2\n");
    let op = spice
        .run(&held, &["op"], &work_dir("vco-op"))
        .expect("operating point");
    let p = &op[0];
    // -5 V reference (IC9, Q38) and the "-4V" line (R166/R174).
    within("-5 V reference", p.scalar("m5"), -5.0, 0.002);
    // The divider's own value is -3.861 V; Q4's base current (through R49) loads it by a
    // few millivolts.
    within("-4 V line", p.scalar("m4"), -10.0 * 3.9 / 10.1, 0.01);
    // IC1's summing junction sits at its + input, -5 V.
    within("IC1 summing junction", p.scalar("sum"), -5.0, 0.001);
    // IC3 holds the reference collector at the -4 V line: 3.86 V / 39K through R50.
    within(
        "reference collector",
        p.scalar("x1.cref"),
        p.scalar("m4"),
        0.001,
    );
    // Section 2.16: 1 V at the keyboard input is about 20 mV at IC1's output; here
    // R20/R27 = 19.6 mV/V. Checked by the difference of two keys below.
    // Buffer: Vbuf = 2.036 Vramp + 3.91 V (R24, R25, R4), "+4V" at a ramp of 0 V.
    within(
        "ramp buffer at -2 V",
        p.scalar("buf"),
        2.036 * -2.0 + 3.91,
        0.03,
    );
    // The JFET pair's tail (Q4): (3.86 - Vbe - (-10)) ... about 0.45 mA through R48 12K.
    let tail = (p.scalar("x1.e4") + 10.0) / 12e3;
    within("buffer tail current (mA)", tail * 1e3, 0.45, 0.05);
}

#[test]
fn keyboard_gain_at_ic1_is_19_6_mv_per_volt() {
    let Some(spice) = for_test("keyboard_gain_at_ic1_is_19_6_mv_per_volt") else {
        return;
    };
    let v = |kbd: f64, tag: &str| {
        let ctl = Controls {
            kbd,
            range: Range::R2,
            ..Controls::default()
        };
        let net = vco::netlist(
            &Trims::default(),
            &ctl,
            Supplies::default(),
            Solver::default(),
        );
        let op = spice
            .run(&format!("{net}vhold ramp 0 -2\n"), &["op"], &work_dir(tag))
            .expect("operating point");
        op[0].scalar("x1.ic1o")
    };
    let d = v(1.0, "vco-gain-1") - v(0.0, "vco-gain-0");
    // R20 / R27 = 1K / 51.1K; IC1's finite gain and R42's feedback move it slightly.
    within(
        "IC1 output per keyboard volt (mV)",
        -d * 1e3,
        1e3 / 51.1,
        0.3,
    );
}

/// Waveform levels while running: the drawing's annotations (Figure 9-3, oscillator 1).
#[test]
fn waveforms_have_the_drawing_levels() {
    let Some(spice) = for_test("waveforms_have_the_drawing_levels") else {
        return;
    };
    let ctl = Controls {
        kbd: LOW_A,
        range: Range::R2,
        ..Controls::default()
    };
    let m = vco::measure(
        &spice,
        &work_dir("vco-levels"),
        &Trims::default(),
        &ctl,
        Solver::default(),
        4,
    )
    .expect("runs");
    let t = m.plot.vec("time");
    let t0 = t[t.len() - 1] - 3.0 / m.hz;
    let span = |name: &str| {
        let v = m.plot.vec(name);
        t.iter()
            .zip(v)
            .filter(|(t, _)| **t >= t0)
            .fold((f64::MAX, f64::MIN), |(lo, hi), (_, &x)| {
                (lo.min(x), hi.max(x))
            })
    };
    let (lo, hi) = span("ramp");
    within("ramp top (drawing 0V)", hi, 0.0, 0.05);
    within("ramp bottom (drawing -4V)", lo, -4.0, 0.2);
    let (lo, hi) = span("buf");
    within("buffer top (drawing +4V)", hi, 4.0, 0.2);
    within("buffer bottom (drawing -4V)", lo, -4.0, 0.3);
    let (lo, hi) = span("tri");
    // "TRIANGLE #1 (+1.7 To -1.7V)" is nominal: Q2's fold is not symmetric (its saturated
    // branch peaks at about 3.3 V, its inverting branch at about 3.5 V) and the output here
    // is unloaded. Check the swing and the middle.
    within("triangle swing", hi - lo, 3.4, 0.4);
    within("triangle middle", (hi + lo) / 2.0, 0.0, 0.2);
    let (lo, hi) = span("x1.tric");
    // Q2's collector: "+3.3V" and "-0.5V" on the drawing.
    within("Q2 collector bottom", lo, -0.5, 0.15);
    within("Q2 collector top", hi, 3.3, 0.3);
    let (lo, hi) = span("rect");
    // "RECTANGULAR #1 (3.5V)": 3.5 V loaded; unloaded, R22/(R13+R22) of 10 V is 4.31 V.
    within("rectangular top", hi, 0.0, 0.05);
    within("rectangular bottom (unloaded)", lo, -10.0 * 4.7 / 10.9, 0.1);
}

/// Folkman's 1973 procedure converges with the trimmers inside their ranges, and the octave
/// step it sets is the keyboard's scale carried through R43: 15K/51.1K of 1.0176 V
/// (the keyboard's 8.48 mA through 10 ohm per key).
#[test]
fn folkman_1973_calibration_converges() {
    let Some(spice) = for_test("folkman_1973_calibration_converges") else {
        return;
    };
    let c = vco::calibrate(
        &spice,
        &work_dir("vco-cal-folkman"),
        Trims::default(),
        Solver::default(),
        Procedure::Folkman1973,
    )
    .expect("calibrates");
    for (r, k, target, hz) in &c.points {
        let cents = 1200.0 * (hz / target).log2();
        assert!(
            cents.abs() < 1.0,
            "{r:?} key {k}: {hz} Hz, {cents:+.2} cents"
        );
    }
    assert!((0.0..1000.0).contains(&c.trims.r11), "{:?}", c.trims);
    assert!((0.0..1.0).contains(&c.trims.a8), "{:?}", c.trims);
    let expected = 15.0 / 51.1 * 12.0 * KEY_STEP;
    within(
        "octave step (V)",
        c.trims.octave_step,
        expected,
        expected * 0.005,
    );
}
