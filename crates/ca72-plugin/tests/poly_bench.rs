//! POLY in real time (decisions.md, "POLY in real time"): the engine as a host calls it, on
//! one thread promoted as an audio thread, `CA72_BLOCK`-frame blocks (256) at 48 kHz, each
//! at its time and never back to back (a late block is followed by the next period's
//! start), for `CA72_SECONDS` (60) after 2 s: `CA72_VOICES` voices (10) playing a changing
//! chord of as many notes every half second (each held seven eighths of it, so the next
//! chord takes voices still releasing), with the modulation moving (oscillator 3 on the
//! filter through the MODULATION wheel, GLIDE on, ENTROPY at 50 %, SPREAD at 70 %, the
//! cutoff swept). Prints each block's time, the late blocks and the watchdog's demotions.
//!
//! `CA72_PRESET` plays a factory preset (its panel as it is, nothing swept) instead.
//! `CA72_WORKERS` (0) shares the voices with that many workers, made audio threads as the
//! plug-in makes them (at most the process's budget of them, R18), each block given the
//! plug-in's deadline (decisions.md R11).
//!
//! By hand, on a quiet machine: `cargo test --release -p ca72-plugin --test poly_bench --
//! --ignored --nocapture`.

#![allow(clippy::unwrap_used)]

mod common;

use ca72_plugin::engine::{Controls, DEADLINE, Engine, Event};
use common::env;
use std::time::{Duration, Instant};

#[test]
#[ignore = "a measurement, run by hand"]
fn poly_chords_in_real_time() {
    let voices: usize = env("CA72_VOICES", 10);
    let seconds: f64 = env("CA72_SECONDS", 60.0);
    let block: usize = env("CA72_BLOCK", 256);
    let rate = 48_000.0;
    let mut c = Controls {
        poly: true,
        voices,
        entropy: 0.5,
        spread: 0.7,
        ..Controls::default()
    };
    c.panel.osc[1].on = true;
    c.panel.osc[1].freq = 0.52;
    c.panel.osc[2].on = true;
    c.panel.osc[2].range = ca72::tuning::Range::Lo;
    c.panel.filter_mod = true;
    c.panel.mod_wheel = 0.4;
    c.panel.glide_on = true;
    c.panel.glide = 0.15;
    c.panel.emphasis = 0.5;
    let preset = std::env::var("CA72_PRESET").ok();
    if let Some(name) = &preset {
        let s = ca72_plugin::library::factory()
            .iter()
            .find(|s| s.name == *name)
            .expect("a factory preset");
        c = common::controls_of(s);
        c.poly = true;
        c.voices = voices;
    }
    let mut e = Engine::new();
    e.set(&c);
    e.prepare(rate, 1);
    let period = Duration::from_secs_f64(block as f64 / rate);
    let workers = e.start_workers(
        env("CA72_WORKERS", 0),
        (env("CA72_WORKERS_RT", 1) == 1).then_some(period),
    );
    let unpaced = env("CA72_UNPACED", 0) == 1;
    let rt = if unpaced {
        Err("unpaced".to_owned())
    } else {
        ca72_rt::promote(period, period / 2)
    };
    ca72_rt::watch();
    let roots = [45u8, 41, 43, 48, 40, 45, 50, 43];
    let shape = [0u8, 7, 12, 16, 19, 24, 28, 31, 34, 36];
    let beat = (0.5 * rate) as usize;
    let warm = (2.0 * rate) as usize / block;
    let blocks = (seconds * rate / block as f64) as usize;
    let mut took = Vec::with_capacity(blocks);
    let mut peak = 0.0f32;
    // A host whose audio thread works under an interval of its own, as a CoreAudio device's
    // IO thread does (`CA72_HOST_WG=1`).
    let host_wg = (env("CA72_HOST_WG", 0) == 1)
        .then(|| ca72_rt::WorkInterval::new("poly_bench host"))
        .flatten();
    let _host_token = host_wg.as_ref().and_then(ca72_rt::WorkInterval::join);
    let start = Instant::now();
    let mut next = 0u32;
    let mut held: Vec<u8> = Vec::with_capacity(16);
    let mut events = Vec::with_capacity(64);
    let (mut lb, mut rb) = (vec![0.0f32; block], vec![0.0f32; block]);
    for i in 0..warm + blocks {
        let due = start + period * next;
        if let Some(wait) = due.checked_duration_since(Instant::now())
            && !unpaced
        {
            std::thread::sleep(wait);
        }
        let t = Instant::now();
        if let Some(w) = &host_wg {
            w.start(period);
        }
        e.set_deadline(Some(t + period.mul_f64(DEADLINE)));
        let at = i * block;
        // The block's notes at their samples, then the block played between them as the
        // plug-in plays its host's (`Engine::render`).
        events.clear();
        for s in 0..block {
            let n = at + s;
            if n % beat == beat * 7 / 8 {
                for k in held.drain(..) {
                    events.push((s, Event::Note { key: k, on: false }));
                }
            }
            if n.is_multiple_of(beat) {
                let b = n / beat;
                let r = roots[b % roots.len()] + (b / roots.len() % 3) as u8;
                for s2 in shape.iter().take(voices) {
                    events.push((
                        s,
                        Event::Note {
                            key: r + s2,
                            on: true,
                        },
                    ));
                    held.push(r + s2);
                }
            }
        }
        let (mut k, mut ev) = (0, 0);
        while k < block {
            while ev < events.len() && events[ev].0 <= k {
                e.event(events[ev].1);
                ev += 1;
            }
            if k == 0 && preset.is_none() {
                c.panel.cutoff = 0.5 + 0.2 * (at as f64 * 2.0 * std::f64::consts::PI / rate).sin();
                e.set(&c);
            }
            let end = events.get(ev).map_or(block, |x| x.0).min(block);
            e.render(&[], &mut lb[k..end], &mut rb[k..end]);
            k = end;
        }
        for (l, r) in lb.iter().zip(&rb) {
            peak = peak.max(l.abs()).max(r.abs());
        }
        e.end_block(block);
        if let Some(w) = &host_wg {
            w.finish();
        }
        if i >= warm {
            took.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let elapsed = start.elapsed().as_secs_f64() / period.as_secs_f64();
        next = (next + 1).max(elapsed.ceil() as u32);
    }
    let p_ms = period.as_secs_f64() * 1e3;
    let late = took.iter().filter(|&&t| t > p_ms).count();
    let mut sorted = took.clone();
    sorted.sort_by(f64::total_cmp);
    let q = |x: f64| sorted[((sorted.len() - 1) as f64 * x) as usize];
    let mean = took.iter().sum::<f64>() / took.len() as f64;
    eprintln!(
        "CA-72{}: {voices} POLY voices at {block}-frame blocks ({p_ms:.3} ms), {seconds} s paced on one thread and {workers} workers: mean {mean:.3} ms, p50 {:.3}, p99 {:.3}, p99.9 {:.3}, worst {:.3}; {late} of {} late; {:.1} % of the thread; peak {peak:.3}; real-time: {rt:?}, demoted by the watchdog {} times",
        preset
            .as_deref()
            .map_or(String::new(), |n| format!(" ({n})")),
        q(0.5),
        q(0.99),
        q(0.999),
        sorted[sorted.len() - 1],
        took.len(),
        mean / p_ms * 100.0,
        ca72_rt::demotions(),
    );
    assert!(peak > 0.01, "the chords were not heard");
}

/// What the strip showed (`realtime_voices`), printed for decisions.md beside the paced runs
/// above: as many with FEEDBACK as without or fewer, and at a larger block as many or more.
/// Those compare separately timed runs, which a loaded machine can upset.
#[test]
#[ignore = "a measurement, run by hand"]
fn what_this_machine_plays_in_real_time() {
    use ca72_plugin::engine::{default_workers, realtime_voices};
    let w = default_workers();
    let small = realtime_voices(48_000.0, 256, false, w);
    let large = realtime_voices(48_000.0, 1024, false, w);
    let fed = realtime_voices(48_000.0, 256, true, w);
    let alone = realtime_voices(48_000.0, 256, false, 0);
    eprintln!(
        "POLY voices in real time here at 48 kHz on the host's thread alone: {alone} at 256 frames"
    );
    eprintln!("POLY voices in real time here at 48 kHz with FEEDBACK on: {fed} at 256 frames");
    assert!(fed <= small);
    eprintln!(
        "POLY voices in real time here at 48 kHz with {w} workers: {small} at 256 frames, {large} at 1024"
    );
    assert!(small <= 10 && large <= 10);
    assert!(large >= small);
    assert_eq!(realtime_voices(48_000.0, 256, false, w), small);
}

/// `realtime_voices` counts within 0 to 10 voices, and gives the same answer again: measured
/// once a process for a rate and block (how many, a loaded machine decides).
#[test]
fn realtime_voices_counts_once_within_the_range() {
    use ca72_plugin::engine::{default_workers, realtime_voices};
    let w = default_workers();
    let n = realtime_voices(48_000.0, 512, false, w);
    assert!(n <= 10, "{n}");
    assert_eq!(realtime_voices(48_000.0, 512, false, w), n);
}
