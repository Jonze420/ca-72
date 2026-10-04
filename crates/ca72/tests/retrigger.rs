//! The contours retrigger only once the trigger contact has been open about 12 ms: after a
//! key is released Q20 holds the reset line until C7 has drained into its base (docs/circuit/board2.md,
//! "Trigger section"), so a key pressed sooner carries the contours on without a new attack,
//! as on the instrument. The plug-in's POLY (decisions.md) leans on it: a note that takes a voice whose
//! key is still down lifts that key and presses the new one `RETRIGGER_GAP` later
//! (13 ms), so the voice's contours retrigger. Checked at three rates in every mode.

use ca72::voice::{Panel, Quality, RETRIGGER_GAP, Voice};

/// The loudness contour's highest over the 80 ms after a second key, against its sustain
/// before, V: with `gap` s between the first key's release and the second's press.
fn rise(rate: f64, quality: Quality, gap: f64) -> f64 {
    let p = Panel {
        quality,
        ..Panel::default()
    };
    let mut v = Voice::new(rate, p);
    v.note(45, true);
    let mut sustain = 0.0;
    for _ in 0..(0.5 * rate) as usize {
        v.tick();
        sustain = v.envelopes().1;
    }
    v.note(45, false);
    for _ in 0..(gap * rate).round() as usize {
        v.tick();
    }
    v.note(52, true);
    let mut peak = f64::MIN;
    for _ in 0..(0.08 * rate) as usize {
        v.tick();
        peak = peak.max(v.envelopes().1);
    }
    peak - sustain
}

#[test]
fn the_contours_retrigger_after_the_trigger_has_been_open_twelve_ms() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for q in [
            Quality::NoCompromises,
            Quality::HighFidelity,
            Quality::Potato,
        ] {
            let (soon, gap) = (rise(rate, q, 0.011), rise(rate, q, RETRIGGER_GAP));
            eprintln!("{rate} Hz {q:?}: open 11 ms {soon:+.3} V, open 13 ms {gap:+.3} V");
            // (The attack peaks over a volt above the sustain; none at 11 ms.)
            assert!(
                soon < 0.1,
                "{rate} {q:?}: retriggered after 11 ms ({soon:+.3} V)"
            );
            assert!(
                gap > 0.8,
                "{rate} {q:?}: not retriggered after 13 ms ({gap:+.3} V)"
            );
        }
    }
}
