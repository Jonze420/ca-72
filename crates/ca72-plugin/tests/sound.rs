//! The engine plays as the instrument does: the default parameters are the default panel, a
//! key sounds at its pitch, MIDI's full bend moves it by MIDI BEND RANGE, the side chain
//! reaches the output through EXTERNAL INPUT, POWER fades the output out and in, and a key
//! held through a change of sample rate plays on.

#![allow(clippy::unwrap_used)]

use ca72::voice::Panel;
use ca72_analysis::pitch::pitch;
use ca72_plugin::engine::{Controls, Engine, Event, output_gain};
use ca72_plugin::params::Ca72Params;

const RATE: f64 = 48_000.0;

fn ready(c: &Controls) -> Engine {
    let mut e = Engine::new();
    e.set(c);
    e.prepare(RATE, 1);
    e
}

/// `secs` of output, `ext` at the side chain.
fn run(e: &mut Engine, secs: f64, ext: impl Fn(f64) -> f32) -> Vec<f64> {
    (0..(secs * RATE) as usize)
        .map(|i| f64::from(e.tick(ext(i as f64 / RATE))))
        .collect()
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|y| y * y).sum::<f64>() / x.len() as f64).sqrt()
}

fn cents(hz: f64, want: f64) -> f64 {
    1200.0 * (hz / want).log2()
}

/// The pitch of a key held for a second, its attack left out.
fn pitch_of(e: &mut Engine, key: u8) -> f64 {
    e.event(Event::Note { key, on: true });
    let y = run(e, 1.0, |_| 0.0);
    pitch(&y[(0.3 * RATE) as usize..], RATE as u32)
        .median_hz
        .unwrap()
}

#[test]
fn the_default_parameters_are_the_default_panel() {
    let c = Ca72Params::default().controls();
    assert_eq!(c.panel, Panel::default());
    assert_eq!((c.volume, c.main_output, c.bend_range), (1.0, true, 2.0));
    assert!(c.power, "POWER on: not bypassed");
}

#[test]
fn power_off_fades_to_silence_and_on_plays_again() {
    let mut c = Controls::default();
    let mut e = ready(&c);
    e.event(Event::Note { key: 57, on: true });
    let playing = run(&mut e, 0.5, |_| 0.0);
    assert!(rms(&playing[(0.2 * RATE) as usize..]) > 0.01);

    c.power = false;
    e.set(&c);
    let fading = run(&mut e, 0.05, |_| 0.0);
    let gone = (0.011 * RATE) as usize;
    assert!(
        fading[..gone].iter().any(|y| *y != 0.0),
        "a fade, not a click"
    );
    assert!(fading[gone..].iter().all(|y| *y == 0.0));

    c.power = true;
    e.set(&c);
    let again = run(&mut e, 0.2, |_| 0.0);
    assert!(
        rms(&again[(0.05 * RATE) as usize..]) > 0.01,
        "the held key plays on"
    );
}

#[test]
fn main_output_volume_follows_its_taper() {
    assert_eq!(output_gain(1.0, true), 1.0);
    assert_eq!(output_gain(0.0, true), 0.0);
    assert_eq!(output_gain(1.0, false), 0.0);
    let half = output_gain(0.5, true);
    assert!(half > 0.05 && half < 0.2, "{half}");
}

#[test]
fn a_key_sounds_at_its_pitch() {
    let mut e = ready(&Controls::default());
    // The A below middle C, on 8': 220 Hz.
    let hz = pitch_of(&mut e, 57);
    assert!(cents(hz, 220.0).abs() < 10.0, "{hz} Hz");
}

#[test]
fn below_its_lowest_rate_the_voice_runs_at_a_multiple_of_the_hosts() {
    // (Where the model alone gives NaN: clap-validator's lowest rate.)
    let rate = 1234.5678;
    let mut e = Engine::new();
    e.set(&Controls::default());
    e.prepare(rate, 1);
    e.event(Event::Note { key: 57, on: true });
    let y: Vec<f64> = (0..(2.0 * rate) as usize)
        .map(|_| f64::from(e.tick(0.0)))
        .collect();
    assert!(y.iter().all(|y| y.is_finite()));
    assert!(rms(&y[(0.3 * rate) as usize..]) > 0.01);
}

#[test]
fn midi_bend_moves_the_pitch_by_its_range() {
    for (range, semitones) in [(2.0, 2.0), (5.0, 5.0)] {
        let c = Controls {
            bend_range: range,
            ..Controls::default()
        };
        let mut e = ready(&c);
        e.event(Event::PitchBend(1.0));
        let up = pitch_of(&mut e, 57);
        let want = 220.0 * 2f64.powf(semitones / 12.0);
        assert!(
            cents(up, want).abs() < 15.0,
            "range {range}: {up} Hz, wanted {want}"
        );
    }
}

#[test]
fn the_side_chain_reaches_the_output_through_external_input() {
    let tau = 2.0 * std::f64::consts::PI;
    let tone = |t: f64| (0.5 * (tau * 440.0 * t).sin()) as f32;
    let mut on = Controls::default();
    on.panel.osc[0].on = false;
    on.panel.ext_on = true;
    let mut off = on;
    off.panel.ext_on = false;
    let level = |c: &Controls| {
        let mut e = ready(c);
        e.event(Event::Note { key: 57, on: true });
        rms(&run(&mut e, 0.5, tone)[(0.2 * RATE) as usize..])
    };
    let (with, without) = (level(&on), level(&off));
    assert!(with > 0.01, "the side chain at {with}");
    assert!(without < with * 1e-2, "{without} with EXTERNAL INPUT off");
}

#[test]
fn silent_until_prepared_and_a_held_key_plays_on_at_a_new_rate() {
    let mut e = Engine::new();
    e.set(&Controls::default());
    e.event(Event::Note { key: 57, on: true });
    assert!(run(&mut e, 0.01, |_| 0.0).iter().all(|y| *y == 0.0));
    e.prepare(44_100.0, 1);
    e.prepare(RATE, 1);
    let y = run(&mut e, 0.5, |_| 0.0);
    assert!(rms(&y[(0.2 * RATE) as usize..]) > 0.01);
}

#[test]
fn reset_controllers_returns_the_wheels() {
    let mut e = Engine::new();
    e.event(Event::PitchBend(1.0));
    e.event(Event::Modulation(0.5));
    let (bend, modulation) = e.wheels();
    assert!((bend - 2.0 / ca72::modulation::PITCH_WHEEL_SEMITONES).abs() < 1e-9);
    assert_eq!(modulation, 0.5);
    e.event(Event::ResetControllers);
    assert_eq!(e.wheels(), (0.0, 0.0));
}

/// POLY off with ENTROPY and SPREAD at 0 and LOCK on (the oscillators without their floor,
/// R9; one oscillator, so no trim) is the one voice as the plug-in played it before POLY
/// existed: the engine's output, sample for sample, a voice's own (its noise seeded with the
/// instance's seed, in Potato) times MAIN OUTPUT's gain.
#[test]
fn poly_off_is_the_one_voice_as_before() {
    use ca72::voice::{Quality, Voice};
    let c = Controls {
        lock: true,
        ..Controls::default()
    };
    let mut e = ready(&c);
    let mut v = Voice::prototype(RATE);
    v.set_seed(1);
    v.panel = Panel {
        quality: Quality::Potato,
        ..Panel::default()
    };
    let gain = output_gain(c.volume, c.main_output);
    let keys = [
        (0, 45, true),
        (9_000, 57, true),
        (20_000, 45, false),
        (30_000, 57, false),
    ];
    let mut k = 0;
    for i in 0..40_000 {
        while k < keys.len() && keys[k].0 == i {
            let (_, key, on) = keys[k];
            e.event(Event::Note { key, on });
            v.note(i32::from(key), on);
            k += 1;
        }
        let got = e.tick(0.0);
        let want = (v.tick_in(0.0) * gain * 1.0) as f32;
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "sample {i}: {got} against {want}"
        );
    }
}

/// POLY: a chord plays on its own voices (louder than one; the one voice plays its lowest
/// key), and with SPREAD at 100 % they part left and right; one voice stays in the centre.
#[test]
fn poly_plays_a_chord_and_spread_parts_it() {
    let chord = |c: &Controls| {
        let mut e = ready(c);
        for key in [45, 52, 57] {
            e.event(Event::Note { key, on: true });
        }
        let (mut l, mut r) = (Vec::new(), Vec::new());
        for _ in 0..(0.6 * RATE) as usize {
            let (a, b) = e.tick_stereo(0.0);
            l.push(f64::from(a));
            r.push(f64::from(b));
        }
        (l, r)
    };
    let (ml, mr) = chord(&Controls::default());
    let poly = Controls {
        poly: true,
        voices: 3,
        ..Controls::default()
    };
    let (pl, pr) = chord(&poly);
    let tail = |x: &[f64]| rms(&x[(0.3 * RATE) as usize..]);
    assert_eq!(ml, mr, "one voice is in the centre");
    assert_eq!(pl, pr, "POLY without SPREAD is in the centre");
    assert!(
        tail(&pl) > 1.4 * tail(&ml),
        "{} against {}",
        tail(&pl),
        tail(&ml)
    );
    let (sl, sr) = chord(&Controls {
        spread: 1.0,
        ..poly
    });
    let apart: f64 = sl.iter().zip(&sr).map(|(a, b)| (a - b).abs()).sum();
    assert!(apart > 1.0, "SPREAD left them together");
    let (one_l, one_r) = chord(&Controls {
        spread: 1.0,
        ..Controls::default()
    });
    assert_eq!(one_l, one_r, "SPREAD moved the one voice");
    assert_eq!(one_l, ml, "SPREAD changed the one voice");
}

/// Every MIDI note plays, in both modes, where the keyboard's scale continues.
#[test]
fn every_midi_note_plays_in_both_modes() {
    for poly in [false, true] {
        let mut c = Controls {
            poly,
            ..Controls::default()
        };
        // (The filter open; MIDI 95, 1976 Hz, the highest this pitch detector reads.)
        c.panel.cutoff = 1.0;
        for key in [20u8, 95] {
            let mut e = ready(&c);
            let f = pitch_of(&mut e, key);
            let want = 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0);
            assert!(
                cents(f, want).abs() < 6.0,
                "POLY {poly}: MIDI {key} at {f} Hz"
            );
        }
    }
}

/// LOCK (decisions.md R9): with it off each oscillator keeps a floor of mismatch, so three in
/// unison drift apart and LOCK sounds different; its trim holds the level to the drifting one
/// (within 0.7 dB, untrimmed 3.3 dB louder), and with one oscillator there is nothing to lock
/// (within 0.3 dB). POLY, a chord with no octave, 7.5 s (drifting oscillators beat slowly).
#[test]
fn lock_is_the_circuit_at_the_drifting_level() {
    for (oscillators, most) in [(3usize, 0.7f64), (1, 0.3)] {
        let level = |lock: bool| {
            let mut c = Controls {
                poly: true,
                voices: 4,
                lock,
                ..Controls::default()
            };
            c.panel.osc[1].on = oscillators == 3;
            c.panel.osc[2].on = oscillators == 3;
            let mut e = ready(&c);
            for k in [45u8, 52, 56, 61] {
                e.event(Event::Note { key: k, on: true });
            }
            let y = run(&mut e, 8.0, |_| 0.0);
            let from = y.len() / 16;
            (y[from..].to_vec(), 20.0 * rms(&y[from..]).log10())
        };
        let ((drift, a), (locked, b)) = (level(false), level(true));
        eprintln!(
            "{oscillators} oscillators: LOCK {:+.2} dB against drifting",
            b - a
        );
        assert!(drift != locked, "LOCK changed nothing");
        assert!(
            (b - a).abs() < most,
            "{oscillators} oscillators: LOCK {:+.2} dB",
            b - a
        );
    }
}

/// `secs` of output played as the plug-in plays its host's blocks (`Engine::render`, 256
/// samples at a time), `ext` at the side chain: left and right together.
fn rendered(e: &mut Engine, secs: f64, ext: impl Fn(f64) -> f32) -> Vec<f64> {
    let mut y = Vec::new();
    let (mut l, mut r, mut x) = ([0.0f32; 256], [0.0f32; 256], [0.0f32; 256]);
    while y.len() < (secs * RATE) as usize {
        for (i, s) in x.iter_mut().enumerate() {
            *s = ext((y.len() + i) as f64 / RATE);
        }
        e.render(&x, &mut l, &mut r);
        e.end_block(256);
        y.extend(l.iter().zip(&r).map(|(a, b)| 0.5 * f64::from(a + b)));
    }
    y
}

fn peak(x: &[f64]) -> f64 {
    x.iter().fold(0.0, |m, y| m.max(y.abs()))
}

/// Two keys held 0.3 s with a long DECAY, then `event`: 0.5 s of output after it, and 0.3 s
/// after a key pressed again. A sample at a time, or with `blocks` as the plug-in plays.
fn after_event(c: &Controls, blocks: bool, event: Event) -> (Vec<f64>, Vec<f64>) {
    let play = |e: &mut Engine, secs: f64| {
        if blocks {
            rendered(e, secs, |_| 0.0)
        } else {
            run(e, secs, |_| 0.0)
        }
    };
    let mut e = ready(c);
    for key in [57, 64] {
        e.event(Event::Note { key, on: true });
    }
    assert!(peak(&play(&mut e, 0.3)) > 0.05, "silent before");
    e.event(event);
    let after = play(&mut e, 0.5);
    e.event(Event::Note { key: 57, on: true });
    (after, play(&mut e, 0.3))
}

/// All Sound Off (CC 120) silences at once, a long DECAY's tail and all: the output fades out
/// over `HUSH_FADE` and stays silent, the voices put to rest. All Notes Off (CC 123) only
/// releases the keys, and the tail rings on. A key after either plays. POLY off and on, a
/// sample at a time and a block at a time (decisions.md R18).
#[test]
fn all_sound_off_silences_at_once_and_all_notes_off_lets_the_tail_ring() {
    use ca72_plugin::engine::HUSH_FADE;
    let mut c = Controls::default();
    c.panel.decay = true;
    c.panel.filter_contour.decay = 1.0;
    c.panel.loudness_contour.decay = 1.0;
    let gone = (HUSH_FADE * RATE) as usize + 2;
    for (poly, blocks) in [(false, false), (false, true), (true, false), (true, true)] {
        c.poly = poly;
        let case = format!("POLY {poly}, blocks {blocks}");
        let (after, again) = after_event(&c, blocks, Event::AllSoundOff);
        assert!(after[..gone].iter().any(|y| *y != 0.0), "{case}: a click");
        let rest = peak(&after[gone..]);
        assert!(rest < 1e-6, "{case}: {rest} after All Sound Off");
        assert!(
            rms(&again[(0.1 * RATE) as usize..]) > 0.01,
            "{case}: silent after"
        );
        let (after, again) = after_event(&c, blocks, Event::AllNotesOff);
        let tail = peak(&after[(0.4 * RATE) as usize..]);
        assert!(tail > 1e-2, "{case}: {tail} after All Notes Off");
        assert!(
            rms(&again[(0.1 * RATE) as usize..]) > 0.01,
            "{case}: silent after"
        );
    }
}

/// A side chain that is not finite (NaN and the infinities, as a host's bus may give) is
/// silence at EXTERNAL INPUT: with EXTERNAL INPUT and FEEDBACK on, the output stays finite and
/// the voice sounds on (R18; before, one such sample made the output NaN for good). POLY off
/// and on, a sample at a time and a block at a time.
#[test]
fn a_side_chain_that_is_not_finite_leaves_the_output_finite_and_sounding() {
    let tau = 2.0 * std::f64::consts::PI;
    let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    let side = |t: f64| {
        if (0.1..0.12).contains(&t) {
            bad[(t * RATE) as usize % 3]
        } else {
            (0.2 * (tau * 110.0 * t).sin()) as f32
        }
    };
    let mut c = Controls {
        feedback: 0.3,
        ..Controls::default()
    };
    c.panel.ext_on = true;
    c.panel.ext_volume = 0.7;
    for poly in [false, true] {
        c.poly = poly;
        for blocks in [false, true] {
            let mut e = ready(&c);
            e.event(Event::Note { key: 57, on: true });
            let y = if blocks {
                rendered(&mut e, 0.5, side)
            } else {
                run(&mut e, 0.5, side)
            };
            let case = format!("POLY {poly}, blocks {blocks}");
            assert!(y.iter().all(|y| y.is_finite()), "{case}");
            assert!(rms(&y[(0.3 * RATE) as usize..]) > 0.01, "{case}: silent");
        }
    }
}

/// A session's seed loaded into a running instance at the rate its voices were built for
/// (the engine prepared again) plays as an instance opened with it: ENTROPY's characters
/// drawn from it too, not only the noise (R18).
#[test]
fn a_seed_loaded_into_a_running_instance_plays_as_one_opened_with_it() {
    let c = Controls {
        poly: true,
        voices: 4,
        entropy: 1.0,
        ..Controls::default()
    };
    let chord = |e: &mut Engine| {
        for key in [45, 52, 57, 64] {
            e.event(Event::Note { key, on: true });
        }
        rendered(e, 0.3, |_| 0.0)
    };
    let mut running = ready(&c);
    running.prepare(RATE, 2);
    let mut opened = Engine::new();
    opened.set(&c);
    opened.prepare(RATE, 2);
    let (a, b) = (chord(&mut running), chord(&mut opened));
    assert!(peak(&a) > 0.05);
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
}
