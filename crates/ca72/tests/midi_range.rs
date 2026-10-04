//! The full MIDI range (decisions.md, "The full MIDI range"): the 44 keys F
//! to C (MIDI 41 to 84) are the instrument's; the plug-in continues its key string beyond them,
//! the scale's volts a key unchanged, so every MIDI note plays where the keyboard's scale
//! puts it, as far as the oscillators follow. The lowest key held sounds, over all 128.

use ca72::voice::{Panel, Quality, Voice};

const SR: f64 = 48_000.0;

/// Oscillator 1's frequency over the next `seconds` (at least `periods` of `near` Hz), from
/// its resets' spacing.
fn frequency(v: &mut Voice, near: f64, seconds: f64, periods: f64) -> f64 {
    let n = (seconds.max(periods / near) * SR) as usize;
    let start = v.probe_resets(0);
    let mut first = None;
    for _ in 0..n {
        v.tick();
        let r = v.probe_resets(0);
        if first.is_none() && r.1 > start.1 {
            first = Some(r);
        }
    }
    let last = v.probe_resets(0);
    match first {
        Some(f) if last.1 > f.1 => (last.1 - f.1) as f64 / (last.0 - f.0) * SR,
        _ => 0.0,
    }
}

fn cents(f: f64, to: f64) -> f64 {
    1200.0 * (f / to).log2()
}

/// Every MIDI note alone on 8' (oscillator 1): the keyboard's voltage, and the pitch
/// against the scale the calibrated keys give (A3, MIDI 57, measured; each semitone a key).
/// The voltages continue the string's volts a key; the pitches follow the scale as far as
/// the oscillator does, and the figures beyond that are the oscillator's own (the limits
/// decisions.md records).
fn sweep(quality: Quality) -> Vec<(i32, f64, f64)> {
    let mut v = Voice::new(SR, Panel::default());
    v.panel.quality = quality;
    let mut rows = Vec::new();
    for n in 0..=127 {
        v.note(n, true);
        let near = 440.0 * 2f64.powf(f64::from(n - 69) / 12.0);
        // The keyboard circuit and the oscillator settled (GLIDE off: within milliseconds).
        for _ in 0..(0.03 * SR) as usize {
            v.tick();
        }
        let volts = v.probe().0;
        let f = frequency(&mut v, near, 0.05, 4.0);
        rows.push((n, volts, f));
        v.note(n, false);
        for _ in 0..(0.005 * SR) as usize {
            v.tick();
        }
    }
    assert_eq!(
        v.keyboard().failed + v.revsaw().failed + v.preamp_failed(),
        0,
        "failed solves"
    );
    rows
}

#[test]
fn every_midi_note_continues_the_keyboards_scale() {
    for quality in [Quality::NoCompromises, Quality::Potato] {
        let rows = sweep(quality);
        let at = |n: i32| rows[n as usize];
        // The string's volts a key, from the instrument's own keys (F to C).
        let step = (at(84).1 - at(41).1) / 43.0;
        let a3 = at(57).2;
        let mut worst_in = 0.0f64;
        let mut table = String::new();
        for &(n, volts, f) in &rows {
            let want_v = at(41).1 + step * f64::from(n - 41);
            let want_f = a3 * 2f64.powf(f64::from(n - 57) / 12.0);
            let c = cents(f, want_f);
            table.push_str(&format!(
                "  {n:3}: {volts:+.5} V ({:+.3} mV off the scale), {f:9.3} Hz ({c:+.2} cents)\n",
                (volts - want_v) * 1e3
            ));
            // The keyboard's voltage continues its scale over every note (the instrument's
            // own keys bend off a straight line by 0.8 mV; beyond them the extension adds
            // whole keys at its volts a key).
            assert!(
                (volts - want_v).abs() < 1e-3,
                "{quality:?}: MIDI {n} at {volts} V, the scale puts it at {want_v}"
            );
            if (41..=84).contains(&n) {
                worst_in = worst_in.max(c.abs());
            }
        }
        eprintln!(
            "{quality:?}: {step:.6} V a key; A3 {a3:.3} Hz; the instrument's keys within {worst_in:.2} cents\n{table}"
        );
        assert!(worst_in < 3.0, "{quality:?}: a key {worst_in:.2} cents off");
        // On 8' the oscillator follows the scale over all 128 notes, 8.2 Hz to 12.6 kHz
        // (within 4 cents: its own tracking, as on the instrument's keys).
        for n in 0..=127 {
            let want = a3 * 2f64.powf(f64::from(n - 57) / 12.0);
            let c = cents(at(n).2, want);
            assert!(
                c.abs() < 5.0,
                "{quality:?}: MIDI {n} {c:+.2} cents off the scale"
            );
        }
    }
}

#[test]
fn the_lowest_key_held_sounds_over_all_128() {
    let mut v = Voice::new(SR, Panel::default());
    let hz = |n: i32| 110.0 * 2f64.powf(f64::from(n - 45) / 12.0);
    let play = |v: &mut Voice, n: i32| {
        for _ in 0..(0.03 * SR) as usize {
            v.tick();
        }
        let f = frequency(v, hz(n), 0.05, 4.0);
        (f, cents(f, hz(n)))
    };
    v.note(60, true);
    let (_, c60) = play(&mut v, 60);
    // A key below the keyboard, under C4: it sounds.
    v.note(29, true);
    let (_, c29) = play(&mut v, 29);
    // A key above the keyboard as well: nothing changes.
    v.note(100, true);
    let (_, c29b) = play(&mut v, 29);
    // The lowest let go: C4, the lowest left, sounds.
    v.note(29, false);
    let (_, c60b) = play(&mut v, 60);
    // C4 let go: the key above the keyboard, alone, sounds.
    v.note(60, false);
    let (_, c100) = play(&mut v, 100);
    // Two above the keyboard: the lower.
    v.note(90, true);
    let (_, c90) = play(&mut v, 90);
    eprintln!(
        "C4 {c60:+.2} cents; F1 under it {c29:+.2}; with MIDI 100 too {c29b:+.2}; F1 let go, C4 \
         {c60b:+.2}; C4 let go, MIDI 100 {c100:+.2}; MIDI 90 under it {c90:+.2}"
    );
    for (what, c) in [
        ("C4", c60),
        ("F1 under C4", c29),
        ("F1 under C4 and 100", c29b),
        ("C4 again", c60b),
        ("100 alone", c100),
        ("90 under 100", c90),
    ] {
        assert!(c.abs() < 10.0, "{what}: {c:+.2} cents");
    }
}

#[test]
fn glide_crosses_the_strings_ends() {
    // GLIDE at 5, legato from MIDI 29 (below the keyboard) to 60, and on to 100 (above it):
    // the keyboard's voltage slides the whole way, with no step where the string ends.
    let panel = Panel {
        glide: 0.5,
        glide_on: true,
        ..Panel::default()
    };
    let mut v = Voice::new(SR, panel);
    v.note(29, true);
    for _ in 0..(0.5 * SR) as usize {
        v.tick();
    }
    let from = v.probe().0;
    v.note(60, true);
    v.note(29, false);
    let mut last = from;
    let mut biggest = 0.0f64;
    let mut volts = Vec::new();
    for _ in 0..(1.0 * SR) as usize {
        v.tick();
        let now = v.probe().0;
        biggest = biggest.max((now - last).abs());
        last = now;
        volts.push(now);
    }
    let mid = volts[(0.03 * SR) as usize];
    // Up again, past the top.
    v.note(100, true);
    v.note(60, false);
    let at60 = last;
    for _ in 0..(1.5 * SR) as usize {
        v.tick();
        let now = v.probe().0;
        biggest = biggest.max((now - last).abs());
        last = now;
    }
    eprintln!(
        "glide: MIDI 29 at {from:.4} V, 30 ms later {mid:.4} V, 60 at {at60:.4} V, 100 at {last:.4} V; \
         the largest step a sample {:.3} mV",
        biggest * 1e3
    );
    assert!(
        from < mid && mid < at60 && at60 < last,
        "the glide did not slide"
    );
    // A key's 84 mV is many samples' slide at GLIDE 5.
    assert!(biggest < 5e-3, "a step of {biggest} V");
}

/// Where the oscillator stops following: the top notes on 2' (up to 50 kHz) and the bottom
/// on LO (down to a sixteenth of a hertz), in each quality mode. Printed for decisions.md's
/// limits; held to what they are now.
#[test]
fn the_oscillators_limits_at_the_ranges_ends() {
    use ca72::tuning::Range;
    for quality in [
        Quality::NoCompromises,
        Quality::HighFidelity,
        Quality::Potato,
    ] {
        let mut lines = String::new();
        for (range, notes, octaves) in [
            (Range::R2, [96, 108, 115, 120, 124, 127], 2.0),
            (Range::Lo, [0, 12, 24, 36, 0, 0], -7.0),
        ] {
            let mut panel = Panel::default();
            panel.osc[0].range = range;
            panel.quality = quality;
            let mut v = Voice::new(SR, panel);
            for (i, &n) in notes.iter().enumerate() {
                if range == Range::Lo && i > 3 {
                    break;
                }
                let want = 440.0 * 2f64.powf(f64::from(n - 69) / 12.0 + octaves);
                v.note(n, true);
                for _ in 0..(0.03 * SR) as usize {
                    v.tick();
                }
                let f = frequency(&mut v, want, 0.05, 3.0);
                v.note(n, false);
                let c = cents(f, want);
                lines.push_str(&format!(
                    "  {range:?} MIDI {n}: {f:.4} Hz against {want:.4} ({c:+.1} cents)\n"
                ));
                // Held to what they are (2026-10-01): on 2' sharp by 2.6 cents at 8.4 kHz
                // up to 52.6 at 50 kHz, on LO flat by 3.7 to 10.5 cents.
                let limit = if range == Range::Lo { 12.0 } else { 60.0 };
                assert!(
                    c.abs() < limit,
                    "{quality:?} {range:?} MIDI {n}: {c:+.1} cents"
                );
            }
        }
        eprintln!("{quality:?}:\n{lines}");
    }
}

/// Far above the instrument's range the converter's model runs away (above about 1e-2 A);
/// the timing current is held at `TIMING_MAX` there. The case a fuzz of the plug-in found:
/// MIDI 126 held and let go on oscillator 1 at 2', oscillators modulated (OSC. MOD. on, MOD.
/// MIX and the MODULATION wheel up), the PITCH wheel up, then oscillator 2 switched from LO
/// to 2' with FREQUENCY at its top: its solve went to NaN within 0.1 s and the voice stayed
/// silent for good. Every mode plays on, finite, no oscillator past the ceiling, and a key
/// in the instrument's range afterwards sounds again.
#[test]
fn the_converter_is_held_at_its_ceiling_far_above_the_keys() {
    use ca72::tuning::Range;
    use ca72::vco::TIMING_MAX;
    use ca72::voice::Waveform;
    let rate = 44_100.0;
    for quality in [
        Quality::NoCompromises,
        Quality::HighFidelity,
        Quality::Potato,
    ] {
        let mut p = Panel {
            quality,
            osc3_control: true,
            cutoff: 1.0,
            emphasis: 0.0,
            contour_amount: 1.0,
            keyboard_control_1: true,
            keyboard_control_2: true,
            glide: 0.0,
            glide_on: true,
            noise_on: true,
            noise_volume: 0.0,
            mod_mix: 1.0,
            osc_mod: true,
            pitch_wheel: 1.0,
            mod_wheel: 1.0,
            tune: -1.0,
            ..Panel::default()
        };
        p.osc[0].range = Range::R2;
        p.osc[0].waveform = Waveform::Triangle;
        p.osc[0].volume = 0.0;
        p.osc[0].freq = 1.0;
        p.osc[1].range = Range::Lo;
        p.osc[1].waveform = Waveform::Triangle;
        p.osc[1].volume = 1.0;
        p.osc[1].freq = 0.0;
        p.osc[2].range = Range::Lo;
        p.osc[2].on = false;
        p.filter_contour.attack = 0.0;
        p.filter_contour.decay = 0.0;
        p.filter_contour.sustain = 1.0;
        p.loudness_contour.attack = 0.0;
        p.loudness_contour.decay = 1.0;
        p.loudness_contour.sustain = 0.0;
        let mut v = Voice::new(rate, p);
        v.note(126, true);
        for _ in 0..2048 {
            v.tick();
        }
        v.note(126, false);
        for _ in 0..2048 {
            v.tick();
        }
        v.panel.osc[1].range = Range::R2;
        v.panel.osc[1].freq = 1.0;
        v.panel.osc[1].volume = 0.0;
        let mut most = 0.0f64;
        for i in 0..(0.5 * rate) as usize {
            let y = v.tick();
            assert!(y.is_finite(), "{quality:?}: sample {i} is {y}");
            most = most.max(v.probe_pitch().1.into_iter().fold(0.0, f64::max));
        }
        assert!(most <= TIMING_MAX, "{quality:?}: {most} A");
        // Back in the instrument's range, it sounds.
        v.panel = Panel {
            quality,
            ..Panel::default()
        };
        v.note(57, true);
        let mut peak = 0.0f64;
        for _ in 0..(0.3 * rate) as usize {
            peak = peak.max(v.tick().abs());
        }
        assert!(peak > 0.05, "{quality:?}: {peak}");
        eprintln!("{quality:?}: the timing current at most {most:.3e} A, then A3 at {peak:.3}");
    }
}
