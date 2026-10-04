//! The voice on its threads (`ca72::threaded`) gives the same samples as the voice
//! run in turn (`Voice::tick_jacks`), to the bit: notes (legato and detached), GLIDE,
//! panel changes between blocks, the external input, the A-440 and the rear jacks (plugged,
//! driven and unplugged again), the quality mode switched, in blocks of every length from
//! a sample to more than a pass. A worker that stops leaves silence, counted, and never a caller waiting.

use ca72::threaded::{CHUNK, Threaded};
use ca72::voice::{Jacks, Quality, Voice, Waveform};

fn noop() {}

#[test]
fn threads_give_the_same_samples_as_one() {
    let rate = 48_000.0;
    let mut serial = Voice::prototype(rate);
    serial.set_seed(7);
    let mut threaded = Threaded::new(serial.clone(), noop).expect("the workers start");
    // Blocks of these lengths in turn, and at each block's start maybe a key or a control.
    let lengths = [1usize, 5, 32, 7, CHUNK + 44, 32, 3, 2 * CHUNK + 1, 32, 11];
    let tau = 2.0 * core::f64::consts::PI;
    // The rear jacks plugged from 0.5 s to 1.3 s: audio-rate CV on the oscillators, the
    // filter's and loudness inputs swept, S-TRIG pulsed.
    let jacks = |i: usize| {
        let t = i as f64 / rate;
        let plugged = (0.5..1.3).contains(&t);
        Jacks {
            ext: 0.3 * libm::sin(tau * 220.0 * t),
            osc: plugged.then(|| 0.2 * libm::sin(tau * 90.0 * t)),
            filter: plugged.then(|| 2.0 * libm::sin(tau * 3.0 * t)),
            loudness: plugged.then(|| 5.0 + 4.0 * libm::sin(tau * 7.0 * t)),
            s_trig: plugged && t.rem_euclid(0.2) < 0.01,
            // The plug-in's ENTROPY from 0.9 s: each oscillator and the cutoff moving.
            detune: if t > 0.9 {
                [
                    4.0 * libm::sin(tau * 0.5 * t),
                    -3.0,
                    2.0 * libm::cos(tau * 0.7 * t),
                ]
            } else {
                [0.0; 3]
            },
            cutoff: if t > 0.9 {
                0.04 * libm::sin(tau * 0.3 * t)
            } else {
                0.0
            },
        }
    };
    let (mut at, mut block, mut compared) = (0usize, 0usize, 0usize);
    while at < (1.6 * rate) as usize {
        let t = at as f64 / rate;
        // The same key or control on both.
        let mut both = |f: &dyn Fn(&mut ca72::voice::Panel)| {
            f(&mut serial.panel);
            f(&mut threaded.panel);
        };
        match block {
            3 => both(&|p| p.glide_on = true),
            6 => both(&|p| {
                p.osc[2].on = true;
                p.osc[2].waveform = Waveform::ReverseSawtooth;
                p.filter_mod = true;
                p.emphasis = 0.8;
            }),
            9 => both(&|p| {
                p.ext_on = true;
                p.ext_volume = 0.7;
            }),
            14 => both(&|p| p.a440 = true),
            // The quality mode switched as it plays (the numerics change on every thread).
            11 => both(&|p| p.quality = Quality::HighFidelity),
            13 => both(&|p| p.quality = Quality::Potato),
            17 => both(&|p| p.quality = Quality::NoCompromises),
            _ => {}
        }
        // (Keys beyond the 44 too: the plug-in's extension of the string.)
        let key = |b: usize| [45, 57, 52, 45, 64, 50, 30, 100, 57][b % 9];
        if block.is_multiple_of(4) {
            serial.note(key(block / 4), true);
            threaded.note(key(block / 4), true);
        }
        if block % 4 == 2 && t > 0.3 {
            // Legato at times: the next key before this one's release.
            serial.note(key(block / 4), false);
            threaded.note(key(block / 4), false);
        }
        // FEEDBACK (decisions.md R8) from 1.0 s to 1.4 s, the external input on: the output
        // into it, a sample late.
        let fb = if (1.0..1.4).contains(&t) { 0.6 } else { 0.0 };
        serial.feedback = fb;
        threaded.feedback = fb;
        let n = lengths[block % lengths.len()];
        let want: Vec<f64> = (0..n).map(|i| serial.tick_jacks(&jacks(at + i))).collect();
        let mut got = vec![f64::NAN; n];
        let passes = threaded.passes();
        threaded.process(n, |i| jacks(at + i), |i, y| got[i] = y);
        // In Potato the parts run in turn on this thread: no pass goes to the workers.
        assert_eq!(
            threaded.passes() == passes,
            threaded.panel.quality == Quality::Potato || fb > 0.0,
            "block {block}"
        );
        for (i, (w, g)) in want.iter().zip(&got).enumerate() {
            assert_eq!(
                w.to_bits(),
                g.to_bits(),
                "sample {} (block {block}): {w} alone, {g} on threads",
                at + i
            );
        }
        compared += n;
        at += n;
        block += 1;
    }
    assert_eq!(threaded.failed, 0);
    let peak = serial.probe().2.abs();
    println!(
        "{compared} samples in {block} blocks, the same to the bit (the filter's last output {peak:.3} V)"
    );
}

/// A worker that stops (here its start panics) makes the voice silent and counted, and the
/// caller does not wait for it: the contours' thread (which feeds the bias's: that one
/// stops waiting too) and the bias's.
#[test]
fn a_stopped_worker_leaves_silence_not_a_wait() {
    fn stop(name: &str) {
        if std::thread::current().name() == Some(name) {
            panic!("{name} stopped for the test");
        }
    }
    fn stop_contours() {
        stop("ca72-contours");
    }
    fn stop_bias() {
        stop("ca72-vca-bias");
    }
    for start in [stop_contours as fn(), stop_bias] {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut t =
                Threaded::new(Voice::prototype(48_000.0), start).expect("the workers start");
            t.note(60, true);
            let mut out = vec![1.0; 2 * CHUNK];
            t.process(2 * CHUNK, |_| Jacks::default(), |i, y| out[i] = y);
            let _ = tx.send((t.failed, out));
        });
        let (failed, out) = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the caller returned");
        assert!(failed > 0);
        assert!(out.iter().all(|&y| y == 0.0));
    }
}
