//! The preamplifier's models inside FEEDBACK's loop (decisions.md R8, R11), against the
//! circuit's (High Fidelity's solve, what R8 approved): each FEEDBACK preset's panel at
//! FEEDBACK 3, 6 and 10 and EXTERNAL INPUT VOLUME 0, 5 and 10, a bass line of one note at a
//! time (a new note every half second, held seven eighths of it), 48 kHz, POLY off, rendered
//! with several seeds (ENTROPY's floor: each a slightly different instrument). Inside the loop
//! the sound is chaotic, so the renders are compared by their statistics: per case, the
//! circuit's mean and spread over the seeds and the model's mean of the level, the power
//! above 1 kHz (brightness) and under 150 Hz, and the OVERLOAD lamp; and the largest change of
//! a third-octave band's level (the power over the seeds; the bands within 40 dB of the
//! strongest).
//!
//! By hand: `cargo test --release -p ca72-plugin --test feedback_models -- --ignored
//! --nocapture` (`CA72_MODEL`: delayed, the default, or plain; `CA72_SEEDS`, 4;
//! `CA72_SECONDS`, 4; `CA72_ONLY`, a part of a preset's name).

#![allow(clippy::unwrap_used)]

mod common;

use ca72::preamp::InLoop;
use ca72_analysis::fft::{hann, power};
use ca72_plugin::engine::{Engine, Event};
use ca72_plugin::library::factory;
use common::{controls_of, env};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 256;

/// A render (mono) and the lamp after each block.
fn play(
    preset: &str,
    knob: f64,
    volume: f64,
    model: InLoop,
    seconds: f64,
    seed: u64,
) -> (Vec<f64>, Vec<f32>) {
    let s = factory().iter().find(|s| s.name == preset).unwrap();
    let mut c = controls_of(s);
    c.poly = false;
    c.feedback = ca72::voice::feedback_law(knob / 10.0);
    c.panel.ext_volume = volume / 10.0;
    let mut e = Engine::new();
    e.set(&c);
    e.prepare(RATE, seed);
    e.set_in_loop(model);
    let beat = (0.5 * RATE) as usize;
    let roots = [33u8, 29, 31, 36, 28, 33, 38, 31];
    let blocks = (seconds * RATE / BLOCK as f64) as usize;
    let mut y = Vec::with_capacity(blocks * BLOCK);
    let mut lamps = Vec::with_capacity(blocks);
    let (mut l, mut r) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
    let mut held = None;
    for b in 0..blocks {
        for k in 0..BLOCK {
            let n = b * BLOCK + k;
            if n % beat == beat * 7 / 8
                && let Some(key) = held.take()
            {
                e.event(Event::Note { key, on: false });
            }
            if n.is_multiple_of(beat) {
                let key = roots[(n / beat) % roots.len()];
                e.event(Event::Note { key, on: true });
                held = Some(key);
            }
            e.render(&[], &mut l[k..k + 1], &mut r[k..k + 1]);
        }
        lamps.push(e.end_block(BLOCK));
        y.extend(l.iter().zip(&r).map(|(a, b)| 0.5 * f64::from(a + b)));
    }
    (y, lamps)
}

/// Third-octave bands' power, 20 Hz to 20 kHz.
fn bands(y: &[f64]) -> Vec<f64> {
    const N: usize = 8192;
    let w = hann(N);
    let mut acc = vec![0.0; N / 2 + 1];
    let mut start = 0;
    while start + N <= y.len() {
        for (a, p) in acc.iter_mut().zip(power(&y[start..start + N], &w)) {
            *a += p;
        }
        start += N / 2;
    }
    let bin = RATE / N as f64;
    (0..31)
        .map(|k| {
            let c = 1000.0 * 2f64.powf((k as f64 - 17.0) / 3.0);
            let (lo, hi) = (c / 2f64.powf(1.0 / 6.0), c * 2f64.powf(1.0 / 6.0));
            acc.iter()
                .enumerate()
                .filter(|(k, _)| (lo..hi).contains(&(*k as f64 * bin)))
                .map(|(_, v)| v)
                .sum()
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Stats {
    level: f64,
    bright: f64,
    low: f64,
    lamp: f64,
}

fn stats(y: &[f64], lamps: &[f32]) -> Stats {
    let y = &y[y.len() / 8..];
    let all = y.iter().map(|x| x * x).sum::<f64>().max(1e-30);
    let hp = |hz: f64| {
        let a = (-std::f64::consts::TAU * hz / RATE).exp();
        let mut lp = 0.0;
        y.iter()
            .map(|&x| {
                lp = a * lp + (1.0 - a) * x;
                (x - lp) * (x - lp)
            })
            .sum::<f64>()
    };
    Stats {
        level: 10.0 * (all / y.len() as f64).log10(),
        bright: hp(1000.0) / all,
        low: 1.0 - hp(150.0) / all,
        lamp: lamps.iter().map(|&l| f64::from(l)).sum::<f64>() / lamps.len() as f64,
    }
}

/// Each seed's statistics, and the bands' power summed over the seeds.
fn seeds(
    preset: &str,
    knob: f64,
    volume: f64,
    model: InLoop,
    seconds: f64,
    n: u64,
) -> (Vec<Stats>, Vec<f64>, f64) {
    let t = std::time::Instant::now();
    let mut all = Vec::new();
    let mut sum = vec![0.0; 31];
    for seed in 1..=n {
        let (y, l) = play(preset, knob, volume, model, seconds, seed);
        all.push(stats(&y, &l));
        for (a, b) in sum.iter_mut().zip(bands(&y)) {
            *a += b;
        }
    }
    (all, sum, t.elapsed().as_secs_f64())
}

/// The largest change of a third-octave band's level (dB) between two sets of bands, among
/// those within 40 dB of the strongest; and where.
fn worst_band(a: &[f64], b: &[f64]) -> (f64, f64) {
    let top = a.iter().fold(0.0f64, |m, x| m.max(*x));
    let (k, d) = a
        .iter()
        .zip(b)
        .enumerate()
        .filter(|(_, (x, _))| **x > top * 1e-4)
        .map(|(k, (x, y))| (k, 10.0 * (y / x).log10()))
        .max_by(|p, q| p.1.abs().total_cmp(&q.1.abs()))
        .unwrap_or((0, 0.0));
    (d, 1000.0 * 2f64.powf((k as f64 - 17.0) / 3.0))
}

/// A statistic's mean and standard deviation over the seeds.
fn ms(v: &[Stats], f: impl Fn(&Stats) -> f64) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().map(&f).sum::<f64>() / n;
    let sd = (v.iter().map(|s| (f(s) - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    (m, sd)
}

#[test]
#[ignore = "a comparison, run by hand"]
fn the_preamplifiers_models_in_the_loop() {
    let model = match std::env::var("CA72_MODEL").as_deref() {
        Ok("delayed") | Err(_) => InLoop::Delayed,
        Ok("plain") => InLoop::Plain,
        Ok(other) => panic!("no model {other}"),
    };
    let seconds: f64 = env("CA72_SECONDS", 4.0);
    let n: u64 = env("CA72_SEEDS", 4);
    let only = std::env::var("CA72_ONLY").ok();
    eprintln!(
        "the circuit against {model:?}, {n} seeds (ENTROPY's floor) of {seconds} s each: the mean (and the circuit's spread, one standard deviation) of the level (dB), the power above 1 kHz and under 150 Hz (%), the lamp's mean; the largest change of a third-octave band (power over the seeds) between the circuit's first half of the seeds and its second, and between the circuit and the model"
    );
    for preset in ["Pulse Strut", "Undertow Growl"] {
        if only.as_ref().is_some_and(|o| !preset.contains(o.as_str())) {
            continue;
        }
        for knob in [3.0, 6.0, 10.0] {
            for volume in [0.0, 5.0, 10.0] {
                let (a, ba, ta) = seeds(preset, knob, volume, InLoop::Circuit, seconds, n);
                let (b, bb, tb) = seeds(preset, knob, volume, model, seconds, n);
                let dist = worst_band(&ba, &bb);
                let f = |v: &[Stats], g: &dyn Fn(&Stats) -> f64| ms(v, g);
                let (la, lsd) = f(&a, &|s| s.level);
                let (lb, _) = f(&b, &|s| s.level);
                let (ra, rsd) = f(&a, &|s| 100.0 * s.bright);
                let (rb, _) = f(&b, &|s| 100.0 * s.bright);
                let (wa, wsd) = f(&a, &|s| 100.0 * s.low);
                let (wb, _) = f(&b, &|s| 100.0 * s.low);
                let (pa, psd) = f(&a, &|s| s.lamp);
                let (pb, _) = f(&b, &|s| s.lamp);
                eprintln!(
                    "{preset:<15} FB {knob:>2} VOL {volume:>2}: level {la:>6.2} ({lsd:.2}) {lb:>6.2} dB  bright {ra:>5.1} ({rsd:>4.1}) {rb:>5.1} %  low {wa:>5.1} ({wsd:>4.1}) {wb:>5.1} %  lamp {pa:.2} ({psd:.2}) {pb:.2}  bands {:>+5.1} dB @ {:.0} Hz  ({:.1}x faster)",
                    dist.0,
                    dist.1,
                    ta / tb,
                );
            }
        }
    }
}
