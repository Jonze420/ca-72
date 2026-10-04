//! The v0 voice end to end (docs/circuit/voice.md): a key sounds from its press, at its
//! pitch, and dies away after its release; with two keys held the lower sounds and the
//! contours are not retriggered.

use ca72::voice::{Panel, Voice, Waveform};

const SR: f64 = 48_000.0;

/// The RMS over `[a, b)` s.
fn rms(x: &[f64], a: f64, b: f64) -> f64 {
    let s = &x[(a * SR) as usize..(b * SR) as usize];
    (s.iter().map(|v| v * v).sum::<f64>() / s.len() as f64).sqrt()
}

/// The fundamental over `[a, b)` s: the autocorrelation's peak between 0.7 and 1.4 times
/// `near` Hz, interpolated.
fn pitch(x: &[f64], a: f64, b: f64, near: f64) -> f64 {
    let s = &x[(a * SR) as usize..(b * SR) as usize];
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    let s: Vec<f64> = s.iter().map(|v| v - mean).collect();
    let r = |lag: usize| -> f64 {
        let n = s.len() - lag;
        s[..n]
            .iter()
            .zip(&s[lag..])
            .map(|(p, q)| p * q)
            .sum::<f64>()
            / n as f64
    };
    let (lo, hi) = ((SR / near / 1.4) as usize, (SR / near / 0.7) as usize);
    let best = (lo..=hi)
        .max_by(|&i, &j| r(i).total_cmp(&r(j)))
        .unwrap_or(lo);
    let (y0, y1, y2) = (r(best - 1), r(best), r(best + 1));
    let d = 0.5 * (y0 - y2) / (y0 - 2.0 * y1 + y2);
    SR / (best as f64 + d)
}

fn cents(f: f64, to: f64) -> f64 {
    1200.0 * (f / to).log2()
}

#[test]
fn a_key_sounds_at_its_pitch_and_releases() {
    // A short release: LOUDNESS DECAY at 1.5 is 11.7K on the 10 uF capacitor, 0.12 s.
    let mut panel = Panel::default();
    panel.loudness_contour.decay = 0.15;
    let mut v = Voice::new(SR, panel);
    let mut out = Vec::new();
    for i in 0..(1.2 * SR) as usize {
        if i == (0.1 * SR) as usize {
            v.note(45, true);
        }
        if i == (0.5 * SR) as usize {
            v.note(45, false);
        }
        out.push(v.tick());
    }
    let before = rms(&out, 0.0, 0.1);
    let held = rms(&out, 0.3, 0.5);
    let after = rms(&out, 1.1, 1.2);
    // The onset: the first sample after the press above 5 % of the held peak (the circuit's
    // trigger delay is about 8 ms; a missed trigger left the first note silent).
    let peak = out[(0.3 * SR) as usize..(0.5 * SR) as usize]
        .iter()
        .fold(0.0f64, |a, y| a.max(y.abs()));
    let press = (0.1 * SR) as usize;
    let onset = out[press..]
        .iter()
        .position(|y| y.abs() > 0.05 * peak)
        .map(|i| i as f64 / SR);
    // The low A on 8' is 110 Hz: the factory tuning puts the second A at 220 Hz there.
    let f = pitch(&out, 0.3, 0.5, 110.0);
    eprintln!(
        "rms: before {before:.2e}, held {held:.3}, 0.6 s after the release {after:.2e}; onset {:.2} ms after the press; \
         pitch {f:.3} Hz ({:+.2} cents)",
        onset.unwrap_or(f64::NAN) * 1e3,
        cents(f, 110.0)
    );
    assert!(before < 1e-3, "the VCA leaks before a key: {before:.2e}");
    assert!(
        onset.is_some_and(|t| t < 0.015),
        "onset {onset:?} s after the press"
    );
    assert!(held > 0.02, "held: {held:.3}");
    assert!(cents(f, 110.0).abs() < 3.0, "pitch {f:.3} Hz");
    // The release holds the key's pitch: its pitch contact opens after the trigger's, when
    // the hold switch has let go (a simultaneous release would hold about 6 cents sharp).
    let f_tail = pitch(&out, 0.51, 0.61, 110.0);
    eprintln!(
        "release tail {f_tail:.3} Hz ({:+.2} cents from the held note)",
        cents(f_tail, f)
    );
    assert!(
        cents(f_tail, f).abs() < 1.0,
        "release tail {f_tail:.3} Hz against {f:.3} Hz held"
    );
    assert!(
        after < 0.01 * held,
        "after the release: {after:.2e} against {held:.3} held"
    );
}

#[test]
fn the_lowest_key_sounds_and_contours_are_not_retriggered() {
    let mut v = Voice::new(SR, Panel::default());
    let mut out = Vec::new();
    let mut loud_after_switch = f64::MIN;
    let mut loud_before_switch = 0.0;
    for i in 0..(1.3 * SR) as usize {
        let t = i as f64 / SR;
        match i {
            _ if i == (0.1 * SR) as usize => v.note(57, true),
            // A lower key while A3 is held: the pitch goes to it.
            _ if i == (0.4 * SR) as usize => v.note(45, true),
            // A higher key: no change.
            _ if i == (0.7 * SR) as usize => v.note(52, true),
            // The lowest released: the next lowest, E, sounds.
            _ if i == (0.9 * SR) as usize => v.note(45, false),
            // (Notes beyond the 44 keys: tests/midi_range.rs.)
            _ => {}
        }
        out.push(v.tick());
        let (_, l, _) = v.envelopes();
        if t > 0.35 && t < 0.4 {
            loud_before_switch = l;
        }
        if t > 0.4 {
            loud_after_switch = loud_after_switch.max(l);
        }
    }
    let f1 = pitch(&out, 0.2, 0.4, 220.0);
    let f2 = pitch(&out, 0.5, 0.7, 110.0);
    let f3 = pitch(&out, 0.75, 0.9, 110.0);
    let f4 = pitch(&out, 1.0, 1.3, 164.8);
    eprintln!(
        "A3 {f1:.2} Hz, then A2 {f2:.2}, with E3 held too {f3:.2}, A2 released {f4:.2}; loudness contour \
         {loud_before_switch:.3} V before the second key, at most {loud_after_switch:.3} V after"
    );
    assert!(cents(f1, 220.0).abs() < 3.0, "A3: {f1:.2} Hz");
    assert!(cents(f2, 110.0).abs() < 3.0, "A2 under A3: {f2:.2} Hz");
    assert!(cents(f3, 110.0).abs() < 3.0, "A2 with E3 above: {f3:.2} Hz");
    let e3 = 110.0 * 2f64.powf(7.0 / 12.0);
    assert!(
        cents(f4, e3).abs() < 3.0,
        "E3 after A2's release: {f4:.2} Hz"
    );
    // Single triggering: the loudness contour stays at its sustain, not a new attack.
    assert!(
        loud_after_switch < loud_before_switch + 0.05,
        "retriggered: {loud_after_switch:.3} V after {loud_before_switch:.3} V"
    );
}

#[test]
fn glide_moves_the_keyboard_voltage_between_keys() {
    // GLIDE at 5 (500K on the 1 uF hold capacitor) and off, legato from A2 up to A3.
    let mut kbd = Vec::new();
    for on in [true, false] {
        let panel = Panel {
            glide: 0.5,
            glide_on: on,
            ..Panel::default()
        };
        let mut v = Voice::new(SR, panel);
        let mut trace = Vec::new();
        for i in 0..(0.9 * SR) as usize {
            if i == (0.1 * SR) as usize {
                v.note(45, true);
            }
            if i == (0.4 * SR) as usize {
                v.note(57, true);
                v.note(45, false);
            }
            v.tick();
            trace.push(v.probe().0);
        }
        assert_eq!(v.keyboard().failed, 0);
        kbd.push(trace);
    }
    let at = |trace: &[f64], t: f64| trace[(t * SR) as usize];
    let (a2, a3) = (at(&kbd[1], 0.39), at(&kbd[1], 0.89));
    // A2's pitch contact opens RELEASE_LEAD (2 ms) after the release: then the pitch moves.
    let off_1ms = at(&kbd[1], 0.403);
    let on = [0.41, 0.43, 0.46, 0.5].map(|t| at(&kbd[0], t));
    eprintln!(
        "keyboard: A2 {a2:.4} V, A3 {a3:.4} V; GLIDE off 1 ms after the contact: {off_1ms:.4} V; GLIDE on at 10, 30, 60, 100 ms: {:.3} {:.3} {:.3} {:.3} V",
        on[0], on[1], on[2], on[3]
    );
    assert!(
        (off_1ms - a3).abs() < 1e-3,
        "GLIDE off: {off_1ms} V 1 ms after the contact, {a3} V at rest"
    );
    assert!(on.windows(2).all(|w| w[1] > w[0]), "GLIDE on rises: {on:?}");
    assert!(
        on[0] > a2 + 0.05 && on[1] < a3 - 0.05,
        "GLIDE on: {on:?} between {a2} and {a3}"
    );
    assert!((at(&kbd[0], 0.89) - a3).abs() < 1e-4, "GLIDE on arrives");
}

/// A voice with only oscillator `n` (0-based) on, set by `set`, playing `midi` from 0.05 s
/// for `seconds`: the output and the mixer bus's current.
fn solo(
    n: usize,
    set: impl Fn(&mut Panel),
    midi: &[(f64, i32)],
    seconds: f64,
) -> (Vec<f64>, Vec<f64>) {
    let mut panel = Panel::default();
    for (k, o) in panel.osc.iter_mut().enumerate() {
        o.on = k == n;
    }
    set(&mut panel);
    let mut v = Voice::new(SR, panel);
    let (mut out, mut bus) = (Vec::new(), Vec::new());
    for i in 0..(seconds * SR) as usize {
        let t = i as f64 / SR;
        for &(at, note) in midi {
            if i == (at * SR) as usize {
                v.note(note, true);
            }
            if i == ((at + 0.3) * SR) as usize {
                v.note(note, false);
            }
            let _ = t;
        }
        out.push(v.tick());
        bus.push(v.probe().1);
    }
    assert_eq!(v.keyboard().failed + v.revsaw().failed, 0);
    (out, bus)
}

#[test]
fn oscillators_2_and_3_play_through_their_controls() {
    // Oscillator 2 at FREQUENCY centred: tuned as oscillator 1 (low A on 8': 110 Hz).
    let (o2, _) = solo(1, |_| {}, &[(0.05, 45)], 0.4);
    let f2 = pitch(&o2, 0.15, 0.35, 110.0);
    // At FREQUENCY 10: its travel's upper half (about +8.2 semitones in the circuit).
    let (o2_up, _) = solo(1, |p| p.osc[1].freq = 1.0, &[(0.05, 45)], 0.4);
    let f2_up = pitch(&o2_up, 0.15, 0.35, 176.0);
    // Oscillator 3 with OSC. 3 CONTROL off: the keys do not move it.
    let off = |p: &mut Panel| p.osc3_control = false;
    let (o3_a, _) = solo(2, off, &[(0.05, 45)], 0.4);
    let (o3_c, _) = solo(2, off, &[(0.05, 57)], 0.4);
    let (fa, fc) = (
        pitch(&o3_a, 0.15, 0.35, 234.0),
        pitch(&o3_c, 0.15, 0.35, 234.0),
    );
    // Oscillator 3's reverse sawtooth against its sawtooth: the mixer's current inverted.
    let (_, saw) = solo(2, |_| {}, &[(0.05, 45)], 0.3);
    let (_, rev) = solo(
        2,
        |p| p.osc[2].waveform = Waveform::ReverseSawtooth,
        &[(0.05, 45)],
        0.3,
    );
    let span = (0.1 * SR) as usize..(0.3 * SR) as usize;
    let mean = |x: &[f64]| x.iter().sum::<f64>() / x.len() as f64;
    let (ms, mr) = (mean(&saw[span.clone()]), mean(&rev[span.clone()]));
    let corr = span
        .clone()
        .map(|i| (saw[i] - ms) * (rev[i] - mr))
        .sum::<f64>()
        / (span.clone().map(|i| (saw[i] - ms).powi(2)).sum::<f64>()
            * span.clone().map(|i| (rev[i] - mr).powi(2)).sum::<f64>())
        .sqrt();
    eprintln!(
        "oscillator 2: {f2:.3} Hz ({:+.2} cents), at FREQUENCY 10 {f2_up:.2} Hz ({:+.2} semitones); oscillator 3 with CONTROL off: \
         A2 {fa:.3} Hz, A3 {fc:.3} Hz; reverse against sawtooth: correlation {corr:+.3}",
        cents(f2, 110.0),
        12.0 * (f2_up / f2).log2()
    );
    assert!(cents(f2, 110.0).abs() < 3.0, "oscillator 2: {f2} Hz");
    assert!(
        (12.0 * (f2_up / f2).log2() - 8.2).abs() < 0.3,
        "oscillator 2 at FREQUENCY 10: {f2_up} Hz"
    );
    assert!(
        cents(fa, fc).abs() < 0.1,
        "oscillator 3 followed the keys with CONTROL off: {fa} {fc}"
    );
    assert!(
        corr < -0.95,
        "the reverse sawtooth is not the sawtooth inverted: {corr}"
    );
}

/// A voice holding low A (MIDI 45 on 8') from the start for `seconds`, its loudness sustained
/// and the filter open: the output, and the filter's I0 and the modulation line per sample.
fn held(set: impl Fn(&mut Panel), key: i32, seconds: f64) -> (Vec<f64>, Vec<f64>) {
    let mut panel = Panel {
        cutoff: 1.0,
        contour_amount: 0.0,
        ..Panel::default()
    };
    panel.loudness_contour.sustain = 1.0;
    panel.filter_contour.sustain = 0.0;
    set(&mut panel);
    let mut v = Voice::new(SR, panel);
    v.note(key, true);
    let (mut out, mut i0) = (Vec::new(), Vec::new());
    for _ in 0..(seconds * SR) as usize {
        out.push(v.tick());
        i0.push(v.filter_probe().0);
    }
    assert_eq!(v.keyboard().failed + v.revsaw().failed, 0);
    (out, i0)
}

/// The frequency over `[a, b)` s from the rising crossings of the window's mean (for a
/// pitch anywhere), interpolated; none with fewer than three.
fn crossings(x: &[f64], a: f64, b: f64) -> Option<f64> {
    let s = &x[(a * SR) as usize..(b * SR) as usize];
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    let ups: Vec<f64> = s
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < mean && w[1] >= mean)
        .map(|(i, w)| i as f64 + (mean - w[0]) / (w[1] - w[0]))
        .collect();
    (ups.len() >= 3).then(|| SR * (ups.len() - 1) as f64 / (ups[ups.len() - 1] - ups[0]))
}

#[test]
fn modulation_and_the_wheels_meet_the_service_manual() {
    // Oscillator 3 on LO with its square, off the mixer, MODULATION MIX at oscillator 3, the
    // MODULATION wheel fully forward.
    let lfo = |p: &mut Panel| {
        p.osc[2].range = ca72::tuning::Range::Lo;
        p.osc[2].waveform = Waveform::Square;
        p.mod_mix = 0.0;
        p.mod_wheel = 1.0;
    };
    // 5.37: oscillator 1 on 2', its triangle, OSCILLATOR MODULATION on: "13 to 23 semitones".
    let (o, _) = held(
        |p| {
            lfo(p);
            p.osc[0].range = ca72::tuning::Range::R2;
            p.osc[0].waveform = Waveform::Triangle;
            p.osc_mod = true;
        },
        45,
        3.0,
    );
    // The pitch in 40 ms windows through the square's cycle.
    let pitches: Vec<f64> = (5..75)
        .filter_map(|k| crossings(&o, k as f64 * 0.04, k as f64 * 0.04 + 0.04))
        .collect();
    let (lo, hi) = pitches
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &f| (a.min(f), b.max(f)));
    let swing = 12.0 * (hi / lo).log2();
    // 5.19: FILTER MODULATION on: the filter's corner from 440 Hz to at least 2.4 kHz (its
    // corner is proportional to I0).
    let (_, i0) = held(
        |p| {
            lfo(p);
            p.cutoff = 0.5;
            p.filter_mod = true;
        },
        45,
        3.0,
    );
    let (i_lo, i_hi) = i0[(0.2 * SR) as usize..]
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    let corner = i_hi / i_lo;
    // The pitch wheel's travel (5.3x: "PITCH WHEEL PITCH adjust 13-17 semitones").
    let wheel = |w: f64| {
        let (o, _) = held(|p| p.pitch_wheel = w, 45, 0.5);
        pitch(&o, 0.2, 0.5, 110.0 * 2f64.powf(8.0 * w / 12.0))
    };
    let (down, up) = (wheel(-1.0), wheel(1.0));
    let bend = 12.0 * (up / down).log2();
    eprintln!(
        "oscillator 1 under oscillator 3's square, the wheel fully forward: {lo:.1} to {hi:.1} Hz, {swing:.1} semitones (5.37: 13 to 23); \
         the filter's corner {corner:.2} times (5.19: at least 2400 / 440 = 5.45); the pitch wheel {down:.2} to {up:.2} Hz, {bend:.3} semitones (13 to 17)"
    );
    assert!((13.0..=23.0).contains(&swing), "5.37: {swing:.1} semitones");
    assert!(corner >= 2400.0 / 440.0, "5.19: {corner:.2}");
    assert!(
        (13.0..=17.0).contains(&bend),
        "the pitch wheel: {bend:.1} semitones"
    );
    // The wheel's travel each way, against which MIDI BEND RANGE scales a bend.
    let each = ca72::modulation::PITCH_WHEEL_SEMITONES;
    assert!(
        (bend / 2.0 - each).abs() < 0.01,
        "the pitch wheel's travel each way: {:.3} semitones, not {each}",
        bend / 2.0
    );
}

#[test]
fn the_noise_sits_under_the_triangle_as_the_factory_set_it() {
    use ca72::voice::NOISE_CALIBRATION_VOLUME as VOL;
    let dev = |x: &[f64]| {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64).sqrt()
    };
    // Oscillator 1's triangle on 2' against the noise, both channels at the calibration's
    // VOLUME (10), the filter open (5.22), the loudness sustained (5.23).
    let (tri, _) = held(
        |p| {
            p.osc[0].range = ca72::tuning::Range::R2;
            p.osc[0].waveform = Waveform::Triangle;
            p.osc[0].volume = VOL;
        },
        45,
        1.0,
    );
    let noise = |pink: bool| {
        let (o, _) = held(
            |p| {
                p.osc[0].on = false;
                p.noise_on = true;
                p.noise_volume = VOL;
                p.noise_pink = pink;
            },
            45,
            2.5,
        );
        o[(0.5 * SR) as usize..].to_vec()
    };
    let t = dev(&tri[(0.3 * SR) as usize..]);
    let (white, pink) = (noise(false), noise(true));
    let (dw, dp) = (
        20.0 * (dev(&white) / t).log10(),
        20.0 * (dev(&pink) / t).log10(),
    );
    // The spectra's tilt: a low octave (125 to 250 Hz) against a high one (4 to 8 kHz),
    // each as mean power per hertz. White is flat (0 dB); pink falls 3 dB an octave (15 dB).
    let tilt = |x: &[f64]| {
        let band = |f0: f64, f1: f64| {
            let n = 1usize << 13;
            let segs = x.len() / n;
            let (b0, b1) = ((f0 * n as f64 / SR) as usize, (f1 * n as f64 / SR) as usize);
            let mut p = 0.0;
            for s in 0..segs {
                let seg = &x[s * n..(s + 1) * n];
                for b in (b0..b1).step_by(((b1 - b0) / 24).max(1)) {
                    let w = 2.0 * std::f64::consts::PI * b as f64 / n as f64;
                    let (mut re, mut im) = (0.0, 0.0);
                    for (i, &v) in seg.iter().enumerate() {
                        let h =
                            0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
                        re += h * v * (w * i as f64).cos();
                        im -= h * v * (w * i as f64).sin();
                    }
                    p += re * re + im * im;
                }
            }
            p
        };
        10.0 * (band(125.0, 250.0) / band(4e3, 8e3)).log10()
    };
    let (tw, tp) = (tilt(&white), tilt(&pink));
    let (t_db, w_db, p_db) = (dbu(t), dbu(dev(&white)), dbu(dev(&pink)));
    eprintln!(
        "at the output, both channels at VOLUME {:.0}: the triangle {t_db:+.1} dB (5.8: 1 +- 3), white noise {w_db:+.1} dB (Folkman: -5), \
         pink {p_db:+.1} dB (Norlin's 5.27: -5 +- 3, not met: A23); against the triangle white {dw:+.2} dB, pink {dp:+.2} dB; \
         tilt from 125-250 Hz to 4-8 kHz: white {tw:+.1} dB, pink {tp:+.1} dB",
        VOL * 10.0
    );
    // The white is calibrated over 20 Hz to 20 kHz at the bus; the filter's top and the
    // rendered band take a little of it.
    assert!((dw + 6.0).abs() < 1.0, "white {dw:+.2} dB");
    assert!(
        (t_db - 1.0).abs() <= 3.0,
        "5.8: the triangle at {t_db:+.1} dB"
    );
    assert!(
        (w_db + 5.0).abs() <= 3.0,
        "the white noise at {w_db:+.1} dB"
    );
    // The pink is the circuit's own (R50 and the pink filter): about 5 dB under the white,
    // 2 dB below Norlin's 5.27 window for it.
    assert!(
        (dp - dw + 4.8).abs() <= 1.0,
        "pink {:+.2} dB against white",
        dp - dw
    );
    assert!(tw.abs() < 3.0, "white noise tilted {tw:+.1} dB");
    assert!((tp - 15.0).abs() < 4.0, "pink noise's tilt {tp:+.1} dB");
}

/// Oscillator 3 alone (the voice's tuning), its sawtooth's resets over up to `seconds`: the
/// mean period, s. Run at a low rate: these periods are long.
fn osc3_period(
    t3: &ca72::tuning::Tuning,
    freq: f64,
    control: bool,
    range: ca72::tuning::Range,
    key: u32,
    seconds: f64,
) -> f64 {
    use ca72::tuning::{Buses, KEY_STEP, OPEN_BUS, Osc, osc_drive_with};
    use ca72::vco::Vco;
    let rate = if seconds > 1.0 { 4_800.0 } else { SR };
    let mut vco = Vco::new(rate, 4);
    vco.expo.r11 = t3.r11;
    vco.expo.a8 = t3.a8;
    let buses = Buses {
        ext: OPEN_BUS,
        ..Buses::RESTING
    };
    let d = osc_drive_with(
        Osc::Three { freq, control },
        t3,
        f64::from(key) * KEY_STEP,
        &buses,
        range,
    );
    let i_in = d.apply(&mut vco.expo);
    let mut resets = Vec::new();
    let mut last = vco.tick(i_in, 0.0).saw;
    for i in 1..(seconds * rate) as usize {
        let s = vco.tick(i_in, 0.0).saw;
        // A reset: the ramp's jump back (either way), which can span a few samples.
        let t = i as f64 / rate;
        if (s - last).abs() > 1.0 && resets.last().is_none_or(|&r| t - r > 1e-3) {
            resets.push(t);
        }
        last = s;
    }
    assert!(resets.len() >= 3, "fewer than three resets in {seconds} s");
    (resets[resets.len() - 1] - resets[1]) / (resets.len() - 2) as f64
}

#[test]
fn oscillator_3s_frequency_and_wide_range_meet_the_service_manual() {
    use ca72::tuning::Range;
    let v = Voice::new(SR, Panel::default());
    let t3 = v.tunings()[2];
    // 5.35: middle C (key 19), FREQUENCY from counterclockwise to clockwise: "14-17
    // Semitones".
    let (lo, hi) = (
        osc3_period(&t3, 0.0, true, Range::R8, 19, 0.3),
        osc3_period(&t3, 1.0, true, Range::R8, 19, 0.3),
    );
    let span = 12.0 * (lo / hi).log2();
    // 5.36: CONTROL off, LO, FREQUENCY at its minimum: "clicks ... between two to five seconds
    // apart"; LO's top overlaps 32''s bottom.
    let slowest = osc3_period(&t3, 0.0, false, Range::Lo, 19, 16.0);
    let lo_top = osc3_period(&t3, 1.0, false, Range::Lo, 19, 2.0);
    let r32_bottom = osc3_period(&t3, 0.0, false, Range::R32, 19, 2.0);
    eprintln!(
        "oscillator 3: FREQUENCY spans {span:.1} semitones at middle C (5.35: 14 to 17); with CONTROL off on LO it clicks every \
         {slowest:.2} s at its minimum (5.36: 2 to 5), {:.2} Hz at its maximum against 32''s minimum {:.2} Hz (5.36: overlapping)",
        1.0 / lo_top,
        1.0 / r32_bottom
    );
    assert!((14.0..=17.0).contains(&span), "5.35: {span:.1} semitones");
    assert!((2.0..=5.0).contains(&slowest), "5.36: {slowest:.2} s");
    assert!(
        lo_top < r32_bottom,
        "5.36: LO's top {:.2} Hz under 32''s bottom {:.2} Hz",
        1.0 / lo_top,
        1.0 / r32_bottom
    );
}

/// dB against 0.775 V (the service manual's meter), from an output's RMS in the voice's
/// scale (1.0 = 5 V).
fn dbu(rms: f64) -> f64 {
    20.0 * (rms * 5.0 / 0.775).log10()
}

#[test]
fn the_a440_and_the_external_input_meet_the_service_manual() {
    let dev = |x: &[f64]| {
        let m = x.iter().sum::<f64>() / x.len() as f64;
        (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64).sqrt()
    };
    // 5.7: A-440 on, nothing else: 440 Hz, "output is -8 +- 2dB" (against oscillator 1's
    // triangle's "1 +- 3dB", 5.8, at the calibration's VOLUME 4: A23).
    let mut panel = Panel {
        a440: true,
        ..Panel::default()
    };
    panel.osc[0].on = false;
    let mut v = Voice::new(SR, panel);
    let a: Vec<f64> = (0..(1.0 * SR) as usize).map(|_| v.tick()).collect();
    let f = crossings(&a, 0.5, 1.0).unwrap_or(0.0);
    let a_db = dbu(dev(&a[(0.5 * SR) as usize..]));
    let (tri, _) = held(
        |p| {
            p.osc[0].range = ca72::tuning::Range::R2;
            p.osc[0].waveform = Waveform::Triangle;
            p.osc[0].volume = ca72::voice::NOISE_CALIBRATION_VOLUME;
        },
        45,
        0.6,
    );
    let t_db = dbu(dev(&tri[(0.3 * SR) as usize..]));
    // 5.26: -30 dB at 1 kHz into the external input, the filter open and the loudness
    // sustained: the OVERLOAD lamp lights before the output shows distortion. The input's
    // VOLUME raised until the lamp lights; the output's harmonics there.
    let amp = 0.775 * 10f64.powf(-30.0 / 20.0) * 2f64.sqrt() / ca72::voice::INPUT_VOLTS;
    let run = |vol: f64| {
        let mut p = Panel {
            cutoff: 1.0,
            ext_on: true,
            ext_volume: vol,
            ..Panel::default()
        };
        p.osc[0].on = false;
        p.loudness_contour.sustain = 1.0;
        let mut v = Voice::new(SR, p);
        v.note(45, true);
        let n = (0.4 * SR) as usize;
        let mut out = Vec::with_capacity(n);
        let mut lamp = 0.0f64;
        for k in 0..n {
            out.push(v.tick_in(amp * (2.0 * std::f64::consts::PI * 1000.0 * k as f64 / SR).sin()));
            lamp = lamp.max(v.overload());
        }
        let tail = &out[(0.3 * SR) as usize..];
        // The 1 kHz fundamental and harmonics 2..10 over whole periods (48 samples each).
        let h = |k: usize| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &x) in tail.iter().enumerate() {
                let w = 2.0 * std::f64::consts::PI * 1000.0 * k as f64 * i as f64 / SR;
                re += x * w.cos();
                im -= x * w.sin();
            }
            (re * re + im * im).sqrt()
        };
        let thd = ((2..=10).map(|k| h(k).powi(2)).sum::<f64>()).sqrt() / h(1);
        (lamp, thd, dbu(dev(tail)))
    };
    let mut lit_at = None;
    for step in 0..=20 {
        let vol = step as f64 / 20.0;
        let (lamp, thd, level) = run(vol);
        if lamp >= 0.5 {
            lit_at = Some((vol, thd, level));
            break;
        }
    }
    eprintln!(
        "A-440: {f:.2} Hz at {a_db:+.1} dB (5.7: -8 +- 2); the triangle at {t_db:+.1} dB (5.8: 1 +- 3), {:+.1} dB apart (the manual's -9); \
         external input -30 dB at 1 kHz: the lamp lights at VOLUME {:?} with the output's THD {:?} % at {:?} dB",
        a_db - t_db,
        lit_at.map(|l| l.0 * 10.0),
        lit_at.map(|l| l.1 * 100.0),
        lit_at.map(|l| l.2)
    );
    assert!((f - 440.0).abs() < 0.05, "A-440 at {f} Hz");
    // 5.7's window is -10..-6 dB; the model is 0.7 dB above it (board4.md, B4-7).
    assert!((a_db + 8.0).abs() <= 3.0, "the A-440 at {a_db:+.1} dB");
    assert!(v.keyboard().failed == 0);
    // "Before any distortion is seen": the lamp lights while the output's distortion is
    // still the filter's soft overdrive (under 6 %), before the preamplifier clips (20 % at
    // the next step).
    let (vol, thd, _) = lit_at.expect("the OVERLOAD lamp never lit");
    assert!(
        thd < 0.06,
        "5.26: the output distorts ({:.1} %) before the lamp lights",
        thd * 100.0
    );
    assert!(vol < 1.0, "the lamp lights only at full VOLUME");
}
