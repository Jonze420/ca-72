//! The plug-in's ENTROPY offsets in the voice (decisions.md; `Jacks::detune`, `Jacks::cutoff`): an
//! oscillator off its calibration by so many cents, whatever its controls (oscillator 3's
//! with OSC. 3 CONTROL off too), and the filter's cutoff by so many octaves. At 0 they are
//! the circuit as drawn (the threaded test and the plug-in's sound test hold that).

use ca72::voice::{FILTER_VOLTS_PER_OCTAVE, Jacks, Panel, Voice};

const SR: f64 = 48_000.0;

/// Oscillator `n`'s frequency over `seconds` of the voice under `jacks`, from its resets.
fn frequency(v: &mut Voice, n: usize, jacks: &Jacks, seconds: f64) -> f64 {
    for _ in 0..(0.02 * SR) as usize {
        v.tick_jacks(jacks);
    }
    let a = v.probe_resets(n);
    for _ in 0..(seconds * SR) as usize {
        v.tick_jacks(jacks);
    }
    let b = v.probe_resets(n);
    (b.1 - a.1) as f64 / (b.0 - a.0) * SR
}

fn cents(f: f64, to: f64) -> f64 {
    1200.0 * (f / to).log2()
}

#[test]
fn an_oscillators_detune_is_its_cents() {
    let mut panel = Panel::default();
    for o in &mut panel.osc {
        o.on = true;
    }
    // Oscillator 3 free of the keyboard (its drone or modulation use): still detuned.
    panel.osc3_control = false;
    let mut v = Voice::new(SR, panel);
    v.note(57, true);
    let at = |d: [f64; 3]| Jacks {
        detune: d,
        ..Jacks::default()
    };
    for n in 0..3 {
        let mut zero = v.clone();
        let f0 = frequency(&mut zero, n, &at([0.0; 3]), 0.2);
        for want in [-25.0, 10.0, 40.0] {
            let mut d = [0.0; 3];
            d[n] = want;
            let mut w = v.clone();
            let f = frequency(&mut w, n, &at(d), 0.2);
            let got = cents(f, f0);
            eprintln!(
                "oscillator {}: {want:+} cents asked, {got:+.3} measured",
                n + 1
            );
            assert!(
                (got - want).abs() < 0.2,
                "oscillator {}: {got} cents for {want}",
                n + 1
            );
        }
    }
}

#[test]
fn the_cutoffs_offset_is_its_octaves() {
    // The ladder's current I0 sets the cutoff, in proportion: an octave is twice the
    // current. Measured at three cutoffs (-3, 0, +3), with the key's tracking in: the
    // offset's octaves within 6 % (at the centre within 1 %).
    let mut out = String::new();
    for cutoff in [0.2, 0.5, 0.8] {
        let panel = Panel {
            cutoff,
            contour_amount: 0.0,
            ..Panel::default()
        };
        let v = Voice::new(SR, panel);
        let i0 = |octaves: f64| {
            let mut w = v.clone();
            let j = Jacks {
                cutoff: octaves,
                ..Jacks::default()
            };
            for _ in 0..200 {
                w.tick_jacks(&j);
            }
            w.filter_probe().0
        };
        let base = i0(0.0);
        for want in [-0.1, 0.04, 0.25] {
            let got = (i0(want) / base).log2();
            out.push_str(&format!(
                "  CUTOFF {:+.0}: {want:+} octave asked, {got:+.5} measured\n",
                cutoff * 10.0 - 5.0
            ));
            // (Within 6 %: the converter's curve over the knob's travel.)
            assert!(
                (got - want).abs() < 0.06 * want.abs(),
                "CUTOFF {cutoff}: {got} octaves for {want}"
            );
        }
    }
    eprintln!("{FILTER_VOLTS_PER_OCTAVE:.5} V an octave at R51\n{out}");
}
