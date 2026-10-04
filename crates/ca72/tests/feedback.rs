//! The plug-in's FEEDBACK (decisions.md R8): the voice's own output patched back into its
//! EXTERNAL INPUT, a sample late, as players cable it. With the MIXER's EXTERNAL INPUT off nothing changes
//! (the cable is not heard); on, the preamplifier and the mixer are driven: OVERLOAD lights
//! and the sound grows harmonics, in every quality mode, and at the controls' ends it stays
//! finite and within the rails.

use ca72::voice::{Panel, Quality, Voice};

const SR: f64 = 48_000.0;

/// `seconds` of A2 held: the samples and the OVERLOAD lamp's brightest.
fn play(quality: Quality, feedback: f64, f: impl Fn(&mut Panel)) -> (Vec<f64>, f64) {
    let mut p = Panel {
        quality,
        ..Panel::default()
    };
    f(&mut p);
    let mut v = Voice::new(SR, p);
    v.feedback = feedback;
    v.note(45, true);
    let y: Vec<f64> = (0..(0.6 * SR) as usize).map(|_| v.tick()).collect();
    (y, v.take_overload_peak())
}

/// The share of the signal's power above its fundamental's first few harmonics' band:
/// how much the spectrum has spread (a crude brightness: power above 1 kHz over all).
fn above_1k(y: &[f64]) -> f64 {
    // A one-pole high-pass at 1 kHz.
    let a = libm::exp(-std::f64::consts::TAU * 1000.0 / SR);
    let (mut lp, mut hi, mut all) = (0.0, 0.0, 0.0);
    for &x in &y[y.len() / 3..] {
        lp = a * lp + (1.0 - a) * x;
        hi += (x - lp).powi(2);
        all += x * x;
    }
    hi / all.max(1e-30)
}

#[test]
fn feedback_is_not_heard_with_the_external_input_off() {
    for q in [Quality::HighFidelity, Quality::Potato] {
        let (a, _) = play(q, 0.0, |_| {});
        let (b, lamp) = play(q, 1.0, |_| {});
        assert!(
            a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "{q:?}: FEEDBACK heard with the mixer's EXTERNAL INPUT off"
        );
        assert_eq!(lamp, 0.0, "{q:?}");
    }
}

/// At 30 % (the loop driven hard, not yet running away) every mode grows harmonics and lights
/// OVERLOAD, and Potato (its preamplifier then solved as High Fidelity's) sounds as High
/// Fidelity does: level within 10 %, brightness within 20 %. Further up the loop runs into
/// self-oscillation, where the modes' small differences grow.
#[test]
fn feedback_overdrives_the_preamplifier_and_the_mixer() {
    let ext = |p: &mut Panel| {
        p.ext_on = true;
        p.ext_volume = 0.6;
        p.cutoff = 0.8;
    };
    let rms = |y: &[f64]| {
        (y[y.len() / 3..].iter().map(|x| x * x).sum::<f64>() / (y.len() - y.len() / 3) as f64)
            .sqrt()
    };
    let mut hf = (0.0, 0.0);
    for q in [
        Quality::NoCompromises,
        Quality::HighFidelity,
        Quality::Potato,
    ] {
        let (dry, dry_lamp) = play(q, 0.0, ext);
        let (wet, wet_lamp) = play(q, 0.3, ext);
        let (b0, b1) = (above_1k(&dry), above_1k(&wet));
        eprintln!(
            "{q:?}: OVERLOAD {dry_lamp:.2} -> {wet_lamp:.2}, power above 1 kHz {:.1} % -> {:.1} %, level {:.3} -> {:.3}",
            100.0 * b0,
            100.0 * b1,
            rms(&dry),
            rms(&wet)
        );
        assert!(wet.iter().all(|y| y.is_finite()), "{q:?}");
        assert!(
            dry_lamp < 0.05 && wet_lamp > 0.5,
            "{q:?}: OVERLOAD {dry_lamp} -> {wet_lamp}"
        );
        assert!(b1 > 2.0 * b0, "{q:?}: no brighter ({b0} -> {b1})");
        match q {
            Quality::HighFidelity => hf = (rms(&wet), b1),
            Quality::Potato => {
                assert!(
                    (rms(&wet) / hf.0 - 1.0).abs() < 0.1,
                    "Potato's level {} against {}",
                    rms(&wet),
                    hf.0
                );
                assert!(
                    (b1 / hf.1 - 1.0).abs() < 0.2,
                    "Potato's brightness {b1} against {}",
                    hf.1
                );
            }
            Quality::NoCompromises => {}
        }
    }
}

/// Every control that feeds the loop at its end (FEEDBACK, EXTERNAL INPUT's VOLUME, all three
/// oscillators and the noise at full volume, EMPHASIS at 10): finite, within the output's
/// rails, and the voice plays a note normally again once FEEDBACK is off.
#[test]
fn feedback_at_its_ends_stays_within_the_rails() {
    for q in [Quality::HighFidelity, Quality::Potato] {
        let mut p = Panel {
            quality: q,
            ext_on: true,
            ext_volume: 1.0,
            emphasis: 1.0,
            cutoff: 1.0,
            noise_on: true,
            noise_volume: 1.0,
            ..Panel::default()
        };
        for o in &mut p.osc {
            o.on = true;
            o.volume = 1.0;
        }
        let mut v = Voice::new(SR, p);
        v.feedback = 1.0;
        v.note(45, true);
        let mut most = 0.0f64;
        for i in 0..(1.0 * SR) as usize {
            let y = v.tick();
            assert!(y.is_finite(), "{q:?}: sample {i}");
            most = most.max(y.abs());
        }
        assert!(most < 20.0, "{q:?}: {most}");
        v.feedback = 0.0;
        v.panel = Panel {
            quality: q,
            ..Panel::default()
        };
        v.note(45, false);
        v.note(57, true);
        let peak = (0..(0.3 * SR) as usize)
            .map(|_| v.tick().abs())
            .fold(0.0, f64::max);
        assert!(peak > 0.05 && peak.is_finite(), "{q:?}: {peak}");
        eprintln!("{q:?}: at the ends, at most {most:.2}; then A3 at {peak:.2}");
    }
}

/// FEEDBACK's knob through its taper: nothing at 0, 30 % of the output at 60 % of the travel,
/// all of it at the top, rising all the way (a gentle onset in the lower half).
#[test]
fn the_knob_is_tapered() {
    use ca72::voice::feedback_law;
    assert_eq!(feedback_law(0.0), 0.0);
    assert!(
        (feedback_law(0.6) - 0.3).abs() < 1e-9,
        "{}",
        feedback_law(0.6)
    );
    assert!((feedback_law(1.0) - 1.0).abs() < 1e-12);
    assert!(feedback_law(1.0 / 3.0) < 0.08 && feedback_law(0.5) < 0.2);
    let mut last = 0.0;
    for k in 1..=100 {
        let a = feedback_law(f64::from(k) / 100.0);
        assert!(a > last, "{k}: {a}");
        last = a;
    }
}
