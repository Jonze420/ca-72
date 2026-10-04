//! The worst-case benchmark (quality modes): the voice as a DAW runs it, in blocks of
//! the stream's size, with every control moving at once, and the worst block's time
//! against the block's duration (`ca72-lab worst`). What counts live is the worst block, not
//! the average.
//!
//! The load: the panel set anew every control step (32 samples, as the DAW the model was
//! developed in sets its devices), every knob sweeping at its own rate, every switch and
//! selector changing on its own period, both wheels moving; the EXTERNAL INPUT on throughout
//! (its preamplifier is the dearest part) with a tone at the jack; notes as sixteenths at 120 BPM over the whole
//! keyboard, detached and legato in turn, GLIDE on half the time; every rear jack plugged
//! (the oscillators' control input at audio rate, the filter's and the loudness input's
//! swept, S-TRIG pulsed). Deterministic: the same load every run.

use ca72::threaded::Threaded;
use ca72::tuning::Range;
use ca72::voice::{Jacks, Panel, Quality, Voice, Waveform};
use std::time::{Duration, Instant};

/// The control grid, samples (that DAW's).
pub const STEP: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub rate: f64,
    pub block: usize,
    pub seconds: f64,
    /// Run the voice in turn on the calling thread instead of on its threads.
    pub serial: bool,
    /// The quality mode the voice runs in.
    pub quality: Quality,
    /// Each block started at its time, as an audio callback is (one block's duration after
    /// the last's start, or at once when that has passed), not as soon as the last ends.
    pub paced: bool,
}

#[derive(Debug, Clone)]
pub struct Report {
    /// Each block's time, ms.
    pub blocks: Vec<f64>,
    /// A block's duration, ms.
    pub budget: f64,
    /// Samples the threaded voice output silent (a worker stopped).
    pub failed: u64,
    /// With the `profile` feature: each block's time part by part (ns, in
    /// `ca72::prof::NAMES`' order), and the keyboard's substeps and Newton
    /// iterations in it.
    pub parts: Vec<[u64; ca72::prof::PARTS]>,
    pub kbd: Vec<(u64, u64)>,
    /// The same for the preamplifier.
    pub pre: Vec<(u64, u64)>,
}

impl Report {
    pub fn percentile(&self, p: f64) -> f64 {
        let mut s = self.blocks.clone();
        s.sort_by(f64::total_cmp);
        s[((s.len() - 1) as f64 * p).round() as usize]
    }

    pub fn worst(&self) -> f64 {
        self.blocks.iter().copied().fold(0.0, f64::max)
    }

    pub fn over(&self) -> usize {
        self.blocks.iter().filter(|&&t| t > self.budget).count()
    }

    pub fn summary(&self) -> String {
        let mean = self.blocks.iter().sum::<f64>() / self.blocks.len() as f64;
        format!(
            "{} blocks of {:.3} ms: mean {:.3} ms, p50 {:.3}, p99 {:.3}, p99.9 {:.3}, worst {:.3} ({:.0} % of the block); over the block: {}; silent samples {}",
            self.blocks.len(),
            self.budget,
            mean,
            self.percentile(0.5),
            self.percentile(0.99),
            self.percentile(0.999),
            self.worst(),
            100.0 * self.worst() / self.budget,
            self.over(),
            self.failed
        )
    }
}

/// A knob's sweep: 0..1 at `hz` from `phase`.
fn sweep(t: f64, hz: f64, phase: f64) -> f64 {
    0.5 + 0.5 * (2.0 * std::f64::consts::PI * hz * t + phase).sin()
}

/// A switch on a period of `s` seconds (on for the first half).
fn toggle(t: f64, s: f64, offset: f64) -> bool {
    ((t + offset) / s).rem_euclid(1.0) < 0.5
}

/// A selector stepping through `n` positions every `s` seconds.
fn pick(t: f64, s: f64, n: usize, offset: usize) -> usize {
    ((t / s) as usize + offset) % n
}

/// The panel at `t` seconds.
pub fn panel_at(t: f64) -> Panel {
    const RANGES: [Range; 6] = [
        Range::Lo,
        Range::R32,
        Range::R16,
        Range::R8,
        Range::R4,
        Range::R2,
    ];
    const WAVES: [Waveform; 6] = [
        Waveform::Triangle,
        Waveform::SharkTooth,
        Waveform::Sawtooth,
        Waveform::Square,
        Waveform::WideRectangle,
        Waveform::NarrowRectangle,
    ];
    let mut p = Panel::default();
    for (n, o) in p.osc.iter_mut().enumerate() {
        // The low ranges move through their positions too, but mostly the audible ones.
        o.range = RANGES[2 + pick(t, 1.3 + 0.4 * n as f64, 4, n)];
        o.waveform = if n == 2 && pick(t, 2.9, 2, 0) == 1 {
            Waveform::ReverseSawtooth
        } else {
            WAVES[pick(t, 1.1 + 0.3 * n as f64, 6, 2 * n)]
        };
        o.on = toggle(t, 2.3 + 0.7 * n as f64, 0.3 * n as f64) || n == 0;
        o.volume = sweep(t, 0.31 + 0.07 * n as f64, n as f64);
        o.freq = sweep(t, 0.23 + 0.05 * n as f64, 1.0 + n as f64);
    }
    p.osc3_control = toggle(t, 3.7, 0.5);
    p.cutoff = sweep(t, 0.47, 0.2);
    p.emphasis = sweep(t, 0.19, 1.4);
    p.contour_amount = sweep(t, 0.37, 2.1);
    p.keyboard_control_1 = toggle(t, 2.1, 0.0);
    p.keyboard_control_2 = toggle(t, 3.3, 1.0);
    p.filter_contour.attack = sweep(t, 0.11, 0.3);
    p.filter_contour.decay = sweep(t, 0.13, 0.9);
    p.filter_contour.sustain = sweep(t, 0.17, 2.7);
    p.loudness_contour.attack = sweep(t, 0.07, 1.9);
    p.loudness_contour.decay = sweep(t, 0.09, 0.4);
    p.loudness_contour.sustain = 0.4 + 0.6 * sweep(t, 0.21, 0.8);
    p.glide = sweep(t, 0.29, 2.2);
    p.glide_on = toggle(t, 4.1, 0.0);
    p.decay = toggle(t, 2.7, 0.9);
    p.noise_on = toggle(t, 1.9, 0.2);
    p.noise_volume = sweep(t, 0.53, 0.6);
    p.noise_pink = toggle(t, 3.1, 1.2);
    p.mod_mix = sweep(t, 0.41, 1.1);
    p.osc_mod = toggle(t, 2.5, 0.7);
    p.filter_mod = toggle(t, 1.7, 0.1);
    p.pitch_wheel = 2.0 * sweep(t, 0.61, 0.0) - 1.0;
    p.mod_wheel = sweep(t, 0.43, 3.0);
    p.ext_on = true;
    p.ext_volume = sweep(t, 0.27, 0.5);
    p.a440 = toggle(t, 5.3, 2.0);
    p.tune = 2.0 * sweep(t, 0.03, 0.0) - 1.0;
    p
}

/// The notes: (sample, MIDI note, on), sixteenths at 120 BPM over the keyboard, on the
/// control grid.
pub fn notes(rate: f64, seconds: f64) -> Vec<(usize, i32, bool)> {
    let sixteenth = rate * 60.0 / 120.0 / 4.0;
    let mut seed = 0x2545_F491u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut ev = Vec::new();
    let mut k = 0;
    while (k as f64 + 1.0) * sixteenth < seconds * rate {
        let key = 41 + (next() % 44) as i32;
        let on = ((k as f64 * sixteenth) as usize / STEP) * STEP;
        // Legato every other bar: the note lasts past the next one's start.
        let legato = (k / 16) % 2 == 1;
        let len = if legato { 1.5 } else { 0.6 } * sixteenth;
        let off = (((k as f64 * sixteenth + len) as usize / STEP) * STEP).max(on + STEP);
        ev.push((on, key, true));
        ev.push((off, key, false));
        k += 1;
    }
    ev.sort_by_key(|&(at, _, on)| (at, on));
    ev
}

/// Runs the benchmark; `start` is called first on the calling thread and on each of the
/// voice's workers (to take the audio threads' priority).
pub fn run(o: Options, start: fn()) -> Report {
    // (The calibration first: long work, not at the audio threads' priority.)
    let voice = Voice::prototype(o.rate);
    start();
    let mut serial = o.serial.then(|| voice.clone());
    let mut threaded = if o.serial {
        None
    } else {
        Some(Threaded::new(voice, start).expect("the voice's workers start"))
    };
    let ev = notes(o.rate, o.seconds);
    let n = (o.seconds * o.rate) as usize;
    let jacks = |i: usize| jacks_at(i as f64 / o.rate);
    let mut blocks = Vec::with_capacity(n / o.block + 1);
    let (mut parts, mut kbd, mut kbd_seen) = (Vec::new(), Vec::new(), (0u64, 0u64));
    let (mut pre, mut pre_seen) = (Vec::new(), (0u64, 0u64));
    ca72::prof::take();
    let mut e = 0;
    let mut at = 0;
    let mut sink = 0.0;
    let period = Duration::from_secs_f64(o.block as f64 / o.rate);
    let mut due = Instant::now();
    while at < n {
        let end = (at + o.block).min(n);
        if o.paced {
            // Sleeps to within a millisecond of the start, then waits out the rest.
            let now = Instant::now();
            if due > now + Duration::from_millis(1) {
                std::thread::sleep(due - now - Duration::from_millis(1));
            }
            while Instant::now() < due {
                std::hint::spin_loop();
            }
            due += period;
        }
        let t0 = Instant::now();
        let mut s = at;
        while s < end {
            let step_end = ((s / STEP + 1) * STEP).min(end);
            let p = Panel {
                quality: o.quality,
                ..panel_at(s as f64 / o.rate)
            };
            while e < ev.len() && ev[e].0 <= s {
                let (_, key, on) = ev[e];
                if let Some(v) = serial.as_mut() {
                    v.note(key, on);
                }
                if let Some(v) = threaded.as_mut() {
                    v.note(key, on);
                }
                e += 1;
            }
            if let Some(v) = serial.as_mut() {
                v.panel = p;
                for i in s..step_end {
                    sink += v.tick_jacks(&jacks(i));
                }
            }
            if let Some(v) = threaded.as_mut() {
                v.panel = p;
                v.process(step_end - s, |i| jacks(s + i), |_, y| sink += y);
            }
            s = step_end;
        }
        blocks.push(t0.elapsed().as_secs_f64() * 1e3);
        if cfg!(feature = "profile") {
            parts.push(ca72::prof::take());
        }
        // (In turn, each block's solves and Newton iterations: a load that does not depend
        // on the machine.)
        if let Some(v) = serial.as_ref() {
            let c = v.keyboard().circuit();
            kbd.push((c.solves - kbd_seen.0, c.total_iterations - kbd_seen.1));
            kbd_seen = (c.solves, c.total_iterations);
            let c = v.preamp().circuits().0;
            pre.push((c.solves - pre_seen.0, c.total_iterations - pre_seen.1));
            pre_seen = (c.solves, c.total_iterations);
        }
        at = end;
    }
    std::hint::black_box(sink);
    Report {
        blocks,
        budget: o.block as f64 / o.rate * 1e3,
        failed: threaded.map_or(0, |t| t.failed),
        parts,
        kbd,
        pre,
    }
}

/// The same load rendered in turn on the calling thread, `seconds` long: the samples, for
/// comparing optimisations with a recording (`ca72-lab perf`, scenario `worst`).
pub fn render(rate: f64, seconds: f64) -> Vec<f64> {
    render_voice(&mut Voice::prototype(rate), rate, seconds)
}

/// [`render`] with a voice of the caller's (whose counters it can read after).
pub fn render_voice(v: &mut Voice, rate: f64, seconds: f64) -> Vec<f64> {
    render_loop(v, rate, seconds, Quality::NoCompromises, false).0
}

/// One sample's probes of the paths a quality mode is measured on (`ca72-lab hifi`): the
/// keyboard's voltage, V, each oscillator's timing current, A, and the filter's and the
/// loudness contour, V.
pub type Probes = [f64; 6];

/// A voice's probes after a sample ([`Probes`]).
pub fn probes(v: &Voice) -> Probes {
    let (k, i) = v.probe_pitch();
    let (f, l) = v.probe_contours();
    [k, i[0], i[1], i[2], f, l]
}

/// [`render`] at a quality mode, the pitch and contour paths held at `hold` if given
/// ([`Voice::hold_control_quality`]), with each sample's [`probes`] if asked.
pub fn render_at(
    rate: f64,
    seconds: f64,
    quality: Quality,
    hold: Option<Quality>,
    with_probes: bool,
) -> (Vec<f64>, Vec<Probes>) {
    let mut v = Voice::prototype(rate);
    v.hold_control_quality(hold);
    render_loop(&mut v, rate, seconds, quality, with_probes)
}

/// [`render_at`] with a voice of the caller's (whose counters it can read after).
pub fn render_loop(
    v: &mut Voice,
    rate: f64,
    seconds: f64,
    quality: Quality,
    with_probes: bool,
) -> (Vec<f64>, Vec<Probes>) {
    let ev = notes(rate, seconds);
    let n = (seconds * rate) as usize;
    let mut out = Vec::with_capacity(n);
    let mut probed = Vec::with_capacity(if with_probes { n } else { 0 });
    let mut e = 0;
    let mut s = 0;
    while s < n {
        let step_end = ((s / STEP + 1) * STEP).min(n);
        v.panel = Panel {
            quality,
            ..panel_at(s as f64 / rate)
        };
        while e < ev.len() && ev[e].0 <= s {
            v.note(ev[e].1, ev[e].2);
            e += 1;
        }
        for i in s..step_end {
            out.push(v.tick_jacks(&jacks_at(i as f64 / rate)));
            if with_probes {
                probed.push(probes(v));
            }
        }
        s = step_end;
    }
    (out, probed)
}

/// [`render`] with the voice's probes each sample: the output, the keyboard's voltage, the
/// mixer bus's current, the filter's output, the filter's control node and the
/// preamplifier's output (for finding where two renders first part).
pub fn render_probed(rate: f64, seconds: f64) -> Vec<[f64; 6]> {
    let mut v = Voice::prototype(rate);
    let ev = notes(rate, seconds);
    let n = (seconds * rate) as usize;
    let mut out = Vec::with_capacity(n);
    let mut e = 0;
    let mut s = 0;
    while s < n {
        let step_end = ((s / STEP + 1) * STEP).min(n);
        v.panel = panel_at(s as f64 / rate);
        while e < ev.len() && ev[e].0 <= s {
            v.note(ev[e].1, ev[e].2);
            e += 1;
        }
        for i in s..step_end {
            let y = v.tick_jacks(&jacks_at(i as f64 / rate));
            let (k, bus, f) = v.probe();
            let (env_f, env_l, _) = v.envelopes();
            out.push([y, k, bus, f, env_f, env_l]);
        }
        s = step_end;
    }
    out
}

/// The jacks at `t` seconds (see [`run`]).
fn jacks_at(t: f64) -> Jacks {
    let tau = 2.0 * std::f64::consts::PI;
    Jacks {
        ext: 0.3 * (tau * 220.0 * t).sin(),
        // Audio-rate FM on the oscillators, a slow sweep on the filter, a tremolo on the
        // loudness, an S-TRIG pulse of 5 ms every 350 ms.
        osc: Some(0.25 * (tau * 110.0 * t).sin()),
        filter: Some(3.0 * (tau * 0.7 * t).sin()),
        loudness: Some(4.0 + 4.0 * (tau * 5.0 * t).sin()),
        s_trig: t.rem_euclid(0.35) < 0.005,
        ..Jacks::default()
    }
}

/// One voice's share of a block from sample `at` to `end` (see [`crowd`]): its panel and
/// notes where it is in the worst case's load, `offset` samples on; `e` is its next event.
fn crowd_voice(
    v: &mut Voice,
    offset: usize,
    e: &mut usize,
    (at, end): (usize, usize),
    ev: &[(usize, i32, bool)],
    o: &Crowd,
) {
    let mut s = at;
    while s < end {
        let step_end = ((s / STEP + 1) * STEP).min(end);
        let when = s + offset;
        v.panel = Panel {
            quality: o.quality,
            ..panel_at(when as f64 / o.rate)
        };
        while *e < ev.len() && ev[*e].0 <= when {
            v.note(ev[*e].1, ev[*e].2);
            *e += 1;
        }
        for i in s..step_end {
            std::hint::black_box(v.tick_jacks(&jacks_at((i + offset) as f64 / o.rate)));
        }
        s = step_end;
    }
}

/// [`crowd`] as the engine's worker pool runs a stage's tracks: each block every voice is
/// a task; `threads - 1` workers and the calling thread claim tasks from one counter until
/// none is left, and the block ends when the last finishes (and every worker has left its
/// claims, so none claims into the next block).
pub fn crowd_pool(o: Crowd, start: fn()) -> Report {
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    const APART: f64 = 0.0371;
    let prototype = Voice::prototype(o.rate);
    let n = (o.seconds * o.rate) as usize;
    let span = o.seconds + APART * o.voices as f64;
    let ev = Arc::new(notes(o.rate, span));
    let voices: Arc<Vec<Mutex<(Voice, usize, usize)>>> = Arc::new(
        (0..o.voices)
            .map(|k| Mutex::new((prototype.clone(), (k as f64 * APART * o.rate) as usize, 0)))
            .collect(),
    );
    let workers = o.threads.max(1) - 1;
    // The block asked for (its first sample, plus one), the next task, the tasks done, and
    // the workers that have left their claims for it.
    let asked = Arc::new(AtomicU64::new(0));
    let next = Arc::new(AtomicUsize::new(usize::MAX));
    let done = Arc::new(AtomicUsize::new(0));
    let out = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    // Claims tasks for the block from `at` until none is left.
    let claim = {
        let (voices, next, done, ev) = (voices.clone(), next.clone(), done.clone(), ev.clone());
        move |at: usize| {
            let end = (at + o.block).min(n);
            loop {
                let i = next.fetch_add(1, Ordering::AcqRel);
                if i >= voices.len() {
                    break;
                }
                let mut guard = voices[i].lock().unwrap_or_else(|p| p.into_inner());
                let (v, offset, e) = &mut *guard;
                crowd_voice(v, *offset, e, (at, end), &ev, &o);
                drop(guard);
                done.fetch_add(1, Ordering::AcqRel);
            }
        }
    };
    let mut handles = Vec::new();
    for _ in 0..workers {
        let (asked, out, stop, claim) = (asked.clone(), out.clone(), stop.clone(), claim.clone());
        handles.push(std::thread::spawn(move || {
            start();
            let mut seen = 0;
            loop {
                let idle = Instant::now();
                let a = loop {
                    let a = asked.load(Ordering::Acquire);
                    if a != seen || stop.load(Ordering::Acquire) {
                        break a;
                    }
                    if idle.elapsed() < Duration::from_micros(100) {
                        std::hint::spin_loop();
                    } else {
                        std::thread::sleep(Duration::from_micros(20));
                    }
                };
                if stop.load(Ordering::Acquire) {
                    return;
                }
                seen = a;
                claim((a - 1) as usize);
                out.fetch_add(1, Ordering::AcqRel);
            }
        }));
    }
    start();
    let mut blocks = Vec::with_capacity(n / o.block + 1);
    let period = Duration::from_secs_f64(o.block as f64 / o.rate);
    let mut due = Instant::now();
    let mut at = 0;
    while at < n {
        if o.paced {
            let now = Instant::now();
            if due > now + Duration::from_millis(1) {
                std::thread::sleep(due - now - Duration::from_millis(1));
            }
            while Instant::now() < due {
                std::hint::spin_loop();
            }
            due += period;
        }
        let t0 = Instant::now();
        done.store(0, Ordering::Release);
        out.store(0, Ordering::Release);
        next.store(0, Ordering::Release);
        asked.store(at as u64 + 1, Ordering::Release);
        claim(at);
        let mut wait = ca72::threaded::Backoff::new();
        while done.load(Ordering::Acquire) < o.voices || out.load(Ordering::Acquire) < workers {
            wait.snooze();
        }
        blocks.push(t0.elapsed().as_secs_f64() * 1e3);
        at += o.block;
    }
    stop.store(true, Ordering::Release);
    for h in handles {
        let _ = h.join();
    }
    Report {
        blocks,
        budget: o.block as f64 / o.rate * 1e3,
        failed: 0,
        parts: Vec::new(),
        kbd: Vec::new(),
        pre: Vec::new(),
    }
}

/// The benchmark's own promotion hook: nothing (the binary passes the real one).
pub fn no_start() {}

/// A block's duration at `rate`, for reports.
pub fn block_duration(block: usize, rate: f64) -> Duration {
    Duration::from_secs_f64(block as f64 / rate)
}

/// The crowd benchmark (Potato): `voices` voices, each under the worst case's load
/// (each a fraction of a second apart in it, so their knobs, notes and jacks do not move
/// together), spread over `threads` threads as a DAW's engine spreads its devices over its
/// workers: each thread runs its voices one after another in turn, each block, and the
/// block takes as long as the slowest thread.
#[derive(Debug, Clone, Copy)]
pub struct Crowd {
    pub voices: usize,
    pub threads: usize,
    pub rate: f64,
    pub block: usize,
    pub seconds: f64,
    pub quality: Quality,
    /// Each block started at its time, as an audio callback is.
    pub paced: bool,
    /// As that DAW's engine runs its tracks on its worker pool: each block every
    /// voice is a task, which `threads - 1` workers and the calling thread claim from one
    /// counter until none is left ([`crowd_pool`]), not each thread its own voices.
    pub pool: bool,
}

/// Runs the crowd benchmark ([`Crowd`]); `start` is called first on each of its threads
/// (to take the audio threads' priority).
pub fn crowd(o: Crowd, start: fn()) -> Report {
    if o.pool {
        return crowd_pool(o, start);
    }
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    const APART: f64 = 0.0371;
    let prototype = Voice::prototype(o.rate);
    let n = (o.seconds * o.rate) as usize;
    let span = o.seconds + APART * o.voices as f64;
    let ev = Arc::new(notes(o.rate, span));
    // The block asked for (its first sample, plus one: 0 none yet), and how many threads
    // have finished it.
    let asked = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handles = Vec::new();
    for t in 0..o.threads {
        let mine: Vec<usize> = (t..o.voices).step_by(o.threads).collect();
        let mut voices: Vec<(Voice, usize, usize)> = mine
            .iter()
            .map(|&k| (prototype.clone(), (k as f64 * APART * o.rate) as usize, 0))
            .collect();
        let (ev, asked, done, stop) = (ev.clone(), asked.clone(), done.clone(), stop.clone());
        handles.push(std::thread::spawn(move || {
            start();
            let mut seen = 0;
            loop {
                // Spinning a moment for the next block, then sleeping in short naps (a
                // thread at real-time priority that spun on would starve the system).
                let idle = Instant::now();
                let a = loop {
                    let a = asked.load(Ordering::Acquire);
                    if a != seen || stop.load(Ordering::Acquire) {
                        break a;
                    }
                    if idle.elapsed() < Duration::from_micros(100) {
                        std::hint::spin_loop();
                    } else {
                        std::thread::sleep(Duration::from_micros(20));
                    }
                };
                if stop.load(Ordering::Acquire) {
                    return;
                }
                seen = a;
                let at = (a - 1) as usize;
                let end = (at + o.block).min(n);
                for (v, offset, e) in voices.iter_mut() {
                    crowd_voice(v, *offset, e, (at, end), &ev, &o);
                }
                done.fetch_add(1, Ordering::AcqRel);
            }
        }));
    }
    start();
    let mut blocks = Vec::with_capacity(n / o.block + 1);
    let period = Duration::from_secs_f64(o.block as f64 / o.rate);
    let mut due = Instant::now();
    let mut at = 0;
    while at < n {
        if o.paced {
            let now = Instant::now();
            if due > now + Duration::from_millis(1) {
                std::thread::sleep(due - now - Duration::from_millis(1));
            }
            while Instant::now() < due {
                std::hint::spin_loop();
            }
            due += period;
        }
        let t0 = Instant::now();
        done.store(0, Ordering::Release);
        asked.store(at as u64 + 1, Ordering::Release);
        let mut wait = ca72::threaded::Backoff::new();
        while done.load(Ordering::Acquire) < o.threads {
            wait.snooze();
        }
        blocks.push(t0.elapsed().as_secs_f64() * 1e3);
        at += o.block;
    }
    stop.store(true, Ordering::Release);
    for h in handles {
        let _ = h.join();
    }
    Report {
        blocks,
        budget: o.block as f64 / o.rate * 1e3,
        failed: 0,
        parts: Vec::new(),
        kbd: Vec::new(),
        pre: Vec::new(),
    }
}
