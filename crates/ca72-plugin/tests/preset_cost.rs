//! What each factory preset costs with POLY's ten voices (decisions.md R10, "POLY in real
//! time"): the engine as a host calls it, 256-frame blocks at 48 kHz, ten-note chords
//! changing every half second (each held seven eighths of it), run as fast as it goes on one
//! thread (not paced, not promoted). Prints, per preset, the share of one core that real
//! time takes, and how many voices that leaves room for in 60 % of a block's period (the
//! strip's figure).
//!
//! By hand: `cargo test --release -p ca72-plugin --test preset_cost -- --ignored
//! --nocapture` (`CA72_SECONDS`, 4; `CA72_VOICES`, 10; `CA72_ONLY`, a part of a preset's
//! name; `CA72_NO_FEEDBACK`, set: FEEDBACK at 0).

#![allow(clippy::unwrap_used)]

mod common;

use ca72_plugin::engine::Engine;
use ca72_plugin::library::factory;
use common::{Chords, controls_of, env, play_block};
use std::time::Instant;

#[test]
#[ignore = "a measurement, run by hand"]
fn each_presets_cost_with_ten_voices() {
    let voices: usize = env("CA72_VOICES", 10);
    let seconds: f64 = env("CA72_SECONDS", 4.0);
    let (rate, block) = (48_000.0, 256usize);
    let blocks = (seconds * rate / block as f64) as usize;
    let mut rows = Vec::new();
    let only = std::env::var("CA72_ONLY").ok();
    let no_feedback = std::env::var_os("CA72_NO_FEEDBACK").is_some();
    for s in factory() {
        if only.as_ref().is_some_and(|o| !s.name.contains(o.as_str())) {
            continue;
        }
        let mut c = controls_of(s);
        c.poly = true;
        c.voices = voices;
        if no_feedback {
            c.feedback = 0.0;
        }
        let mut e = Engine::new();
        e.set(&c);
        e.prepare(rate, 1);
        let workers = common::workers(&mut e);
        let mut chords = Chords::new(voices, rate);
        let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
        let mut events = Vec::with_capacity(64);
        let mut worst = 0.0f64;
        let start = Instant::now();
        for i in 0..blocks {
            let t = Instant::now();
            play_block(&mut e, &mut chords, i * block, &mut l, &mut r, &mut events);
            std::hint::black_box((&l, &r));
            worst = worst.max(t.elapsed().as_secs_f64());
        }
        let took = start.elapsed().as_secs_f64();
        let share = took / (blocks as f64 * block as f64 / rate);
        let period = block as f64 / rate;
        rows.push((s.name.clone(), share, worst / period, c.feedback > 0.0));
        let _ = workers;
    }
    eprintln!(
        "CA-72, {voices} POLY voices, 256-frame blocks at 48 kHz, {}:",
        match env("CA72_WORKERS", 0usize) {
            0 => "one thread".to_owned(),
            n => format!("the caller's thread and {n} workers"),
        }
    );
    eprintln!(
        "{:<18} {:>8} {:>13} {:>10}",
        "preset", "of a core", "worst block", "voices fit"
    );
    for (name, share, worst, fed) in &rows {
        let fits = (0.6 / (share / voices as f64)).floor().min(10.0);
        eprintln!(
            "{name:<18} {:>7.1} % {:>11.0} % {fits:>10}{}",
            share * 100.0,
            worst * 100.0,
            if *fed { "  FEEDBACK" } else { "" }
        );
    }
}
