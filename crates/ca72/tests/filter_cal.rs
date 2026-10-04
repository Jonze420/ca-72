//! The filter's factory calibration (Folkman 1973) run on the real-time filter, its result
//! against the committed trims, and the calibrated filter against the service manual's
//! later checks (5.13, 5.15, 5.18).

use ca72::filter_cal::{FACTORY, FilterTrims, folkman, inputs, oscillation, regenerates};
use ca72::keyboard::{Keyboard, KeyboardCircuit};

const SR: f64 = 48_000.0;

#[test]
fn filter_calibration() {
    let t = folkman(SR);
    eprintln!(
        "Folkman's procedure: R39 at {:.6}, R49 {:.4} ohm, R73 at {:.6}",
        t.r39, t.r49, t.r73_pos
    );
    let f = t;
    // Checks on the calibrated filter.
    let kb = Keyboard::new(KeyboardCircuit::default(), SR);
    let key = |k: usize| kb.static_out(k).expect("key");
    // Range: 440 Hz at CUTOFF -1 with the keyboard off; Scale: low A 440, third A 1760.
    let range = oscillation(SR, &f, &inputs(0.4, false, false, 0.0));
    // 5.13: fully clockwise "at least 16kHz"; counterclockwise "less than 300Hz before
    // regeneration dies out".
    let top = oscillation(SR, &f, &inputs(1.0, false, false, 0.0));
    let mut lowest = None;
    for step in (0..=40).rev() {
        let c = step as f64 / 100.0;
        let hz = oscillation(SR, &f, &inputs(c, false, false, 0.0));
        if hz <= 0.0 {
            break;
        }
        lowest = Some(hz);
    }
    // 5.15: KEYBOARD CONTROL 1 alone, low A tuned to 440 Hz with CUTOFF: high A "880 +- 50Hz".
    let find = |k: usize, kb1: bool, kb2: bool, hz: f64| {
        let (mut a, mut b) = (0.05, 0.85);
        for _ in 0..30 {
            let m = 0.5 * (a + b);
            if oscillation(SR, &f, &inputs(m, kb1, kb2, key(k))) < hz {
                a = m;
            } else {
                b = m;
            }
        }
        0.5 * (a + b)
    };
    let c15 = find(4, true, false, 440.0);
    let high = oscillation(SR, &f, &inputs(c15, true, false, key(40)));
    // 5.18: from 440 Hz, "approximately 2 divisions" of CUTOFF (0.1 of its travel each) up
    // to 1760 Hz.
    let c18 = find(0, false, false, 440.0);
    let c18_up = find(0, false, false, 1760.0);
    let divisions = (c18_up - c18) * 10.0;
    let two = oscillation(SR, &f, &inputs(c18 + 0.2, false, false, 0.0));
    // Scale: three octaves of keys at KEYBOARD CONTROL 1 and 2.
    let c3 = find(28, true, true, 1760.0);
    let scale_low = oscillation(SR, &f, &inputs(c3, true, true, key(4)));
    let scale_high = oscillation(SR, &f, &inputs(c3, true, true, key(40)));
    // Regeneration starts between EMPHASIS 7.5 and a hair above.
    let ins = inputs(0.4, false, false, 0.0);
    let (at_74, at_76) = (
        regenerates(SR, &f, &ins, 0.74),
        regenerates(SR, &f, &ins, 0.76),
    );
    eprintln!(
        "calibrated: CUTOFF -1 {range:.2} Hz (440 before the scale was set); fully clockwise {top:.0} Hz (5.13: at least 16 kHz); lowest before regeneration dies {lowest:?} Hz \
         (5.13: under 300); KEYBOARD CONTROL 1 from low A at 440 Hz: high A {high:.1} Hz (5.15: 880 +- 50); two divisions from 440 Hz: \
         {two:.0} Hz, 1760 Hz {divisions:.2} divisions up (5.18: about 2); with 1 and 2 and the third A at 1760: low A {scale_low:.2} Hz, high A {scale_high:.1} Hz (3520); \
         regenerates at EMPHASIS 7.4: {at_74}, 7.6: {at_76}"
    );
    let same = |a: &FilterTrims, b: &FilterTrims| {
        (a.r39 - b.r39).abs() < 1e-4
            && (a.r49 - b.r49).abs() < 0.05
            && (a.r73_pos - b.r73_pos).abs() < 1e-3
    };
    assert!(
        same(&t, &FACTORY),
        "the procedure gives {t:?}: update filter_cal::FACTORY"
    );
    // "Filter Range" is set before "Filter Scale", which moves it (the procedure does not
    // return to it): reported, not checked.
    let _ = range;
    assert!((scale_low - 440.0).abs() < 1.0, "Filter Scale: {scale_low}");
    assert!(top >= 16e3, "5.13: {top}");
    assert!(lowest.is_some_and(|l| l < 300.0), "5.13: {lowest:?}");
    assert!((high - 880.0).abs() <= 50.0, "5.15: {high}");
    assert!(!at_74 && at_76, "Regeneration Cal.");
    assert!((divisions - 2.0).abs() < 0.5, "5.18: {divisions} divisions");
    assert!(
        (scale_high / scale_low - 8.0).abs() < 0.4,
        "the filter tracks three octaves: {scale_low} to {scale_high}"
    );
}
