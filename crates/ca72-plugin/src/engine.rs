//! The plug-in's audio engine, apart from any plug-in format: the circuit model's voice in its
//! real-time (Potato) quality, played by MIDI notes, pitch bend and the modulation wheel, with
//! the side chain at EXTERNAL INPUT. Once the voices are built nothing here allocates
//! (`tests/realtime.rs`).
//!
//! POLY off, the one instrument takes every key through its keyboard circuit (lowest-note
//! priority, single triggering), as before. POLY on, one voice per note, up to VOICES (2 to
//! 10) of the ten built: each the whole instrument with its own keyboard circuit, which its
//! note reaches as one key (`PolyKey`; decisions.md, "POLY"). ENTROPY makes each voice its
//! own parts, SPREAD puts the voices across the stereo field (`character.rs`); both 0 by
//! default, the circuit as drawn.

use ca72::modulation::PITCH_WHEEL_SEMITONES;
use ca72::resample::{Decimator, Interpolator};
use ca72::voice::{Jacks, Panel, Quality, RETRIGGER_GAP, Voice, audio_taper};

use crate::character::Character;
use crate::pool::{Ask, Crew, MAX, Pool, Shared};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How far ENTROPY goes at 100 %, in the character's figures (`character.rs`, the DAW's):
/// twice them, as in the DAW's instrument (the owner, 2026-10-02, found them too subtle at
/// once).
pub const ENTROPY_DEPTH: f64 = 2.0;

/// The oscillators' own mismatch and drift with LOCK off, in the character's figures
/// (decisions.md R9; the DAW's): a tuning spread of 3 cents and 2.25 of drift, so oscillators
/// at one pitch never lock in phase (locked, three in unison sounded 3.3 dB louder, not
/// better). ENTROPY adds to it.
pub const ENTROPY_FLOOR: f64 = 0.75;

/// How much of the ideal gain of locking (`10 log10(coherent / incoherent)`) shows at the
/// output, the mixer's overdrive and the filter taking the rest (fitted in the DAW).
pub const LOCK_SHARE: f64 = 0.68;

/// The output's gain with LOCK on: down by the panel's switched-on oscillators that share a
/// pitch (RANGE, FREQUENCY and, for oscillator 3, OSC. 3 CONTROL alike) and their volumes, so
/// locked oscillators sound as loud as drifting ones. With no two at one pitch, 1.
pub fn lock_trim(p: &Panel) -> f64 {
    let keys = |n: usize| (p.osc[n].range, p.osc[n].freq, n < 2 || p.osc3_control);
    let on = |n: usize| p.osc[n].on && p.osc[n].volume > 0.0;
    let incoherent: f64 = (0..3)
        .filter(|&n| on(n))
        .map(|n| p.osc[n].volume.powi(2))
        .sum();
    let mut coherent = 0.0;
    for n in (0..3).filter(|&n| on(n)) {
        if (0..n).any(|m| on(m) && keys(m) == keys(n)) {
            continue;
        }
        let a: f64 = (0..3)
            .filter(|&m| on(m) && keys(m) == keys(n))
            .map(|m| p.osc[m].volume)
            .sum();
        coherent += a * a;
    }
    if incoherent > 0.0 && coherent > incoherent {
        ca72::ulp::pow(coherent / incoherent, -0.5 * LOCK_SHARE)
    } else {
        1.0
    }
}

/// The voices POLY may play: the fewest, by default, the most (all of them built).
pub const POLY_VOICES: (usize, usize, usize) = (2, 4, 10);

/// ENTROPY's tolerances on the panel: EMPHASIS, the filter contour's ATTACK and DECAY, the
/// loudness contour's, GLIDE.
const ENTROPY_KNOBS: usize = 6;

/// What the parameters set: the panel (its wheels where the parameters leave them), MAIN
/// OUTPUT's VOLUME (0..1 of its travel) and switch, how far a MIDI keyboard's full pitch
/// bend moves the PITCH wheel, in semitones, POWER (off: bypassed, silent), and POLY,
/// VOICES, ENTROPY and SPREAD (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub panel: Panel,
    pub volume: f64,
    pub main_output: bool,
    pub bend_range: f64,
    pub power: bool,
    pub poly: bool,
    pub voices: usize,
    pub entropy: f64,
    pub spread: f64,
    /// FEEDBACK (0..1): the voices' output patched back into their EXTERNAL INPUT, a sample
    /// late, by this much of its level (the phones' VOLUME knob; decisions.md R8).
    pub feedback: f64,
    /// LOCK: the oscillators identical, the circuit as drawn, its level matched
    /// ([`lock_trim`]); off, each oscillator keeps its own floor of mismatch
    /// ([`ENTROPY_FLOOR`]; decisions.md R9).
    pub lock: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            panel: Panel::default(),
            volume: 1.0,
            main_output: true,
            bend_range: 2.0,
            power: true,
            poly: false,
            voices: POLY_VOICES.1,
            entropy: 0.0,
            spread: 0.0,
            feedback: 0.0,
            lock: false,
        }
    }
}

/// MIDI input, as the engine takes it (any channel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Note {
        key: u8,
        on: bool,
    },
    /// The pitch bend, -1..1, 0 at rest.
    PitchBend(f32),
    /// The modulation wheel (control change 1), 0..1.
    Modulation(f32),
    /// Reset All Controllers (control change 121): the bend and modulation wheel at rest.
    ResetControllers,
    /// All Notes Off (control change 123): every key released (the contours then run out as
    /// the DECAY switch has them).
    AllNotesOff,
    /// All Sound Off (control change 120): every key released and the sound gone at once, the
    /// output faded out over [`HUSH_FADE`] and every voice put to rest (decisions.md R18).
    AllSoundOff,
}

/// The control change of a keyboard's modulation wheel.
pub const MODULATION_WHEEL: u8 = 1;
pub const ALL_SOUND_OFF: u8 = 120;
pub const RESET_CONTROLLERS: u8 = 121;
pub const ALL_NOTES_OFF: u8 = 123;

/// How fast the reported OVERLOAD lamp falls from a peak, s: the lamp's own thermal lag is
/// not modelled, and a display polling now and then should still see a flash.
const LAMP_FALL: f32 = 0.05;

/// The lowest rate the voice runs at, Hz: below it, at 2, 4 or 8 times the host's. The top
/// key's fundamental on 2' (4.2 kHz) stays under its Nyquist frequency, and the model's
/// factory calibration fails at much lower rates (it gives NaN at 1.2 kHz).
pub const MIN_VOICE_RATE: f64 = 8_000.0;

/// How long POWER takes to fade the output out or in, s (switched at once, it clicks).
const POWER_FADE: f64 = 0.01;

/// How long All Sound Off takes to fade the output out before the voices are put to rest, s
/// (at once, it clicks; decisions.md R18). It fades back in as POWER does.
pub const HUSH_FADE: f64 = 0.005;

/// The longest All Sound Off keeps the output silent waiting for clean voices to put in the
/// sounding ones' places ([`Spares`]), s; they are normally ready long before.
const HUSH_WAIT: f64 = 0.1;

/// A POLY voice falls silent, and is free for another note, once its key is up and its
/// output has stayed under this for a whole block.
const SILENT: f64 = 1e-6;

/// The voices at a multiple of the host's rate: the side chain interpolated up, the left and
/// right outputs decimated down.
#[derive(Debug)]
struct Oversampled {
    factor: usize,
    up: Interpolator,
    down: [Decimator; 2],
}

/// One POLY voice's key (decisions.md, "POLY"): its keyboard circuit gets its note as a key
/// down while the voice's gate is high. A new note on the voice while its gate was high
/// (the gate dropped for a sample between them) lifts the key and presses the new one once
/// the trigger contact has been open [`RETRIGGER_GAP`]: the circuit re-arms its trigger only
/// then, so the contours retrigger; the new note's pitch and attack come that much later.
#[derive(Debug, Clone, Copy)]
struct PolyKey {
    down: Option<i32>,
    /// Samples the trigger contact has been open, and how many it must be.
    open: u32,
    gap: u32,
}

impl PolyKey {
    fn new(rate: f64) -> PolyKey {
        let gap = (RETRIGGER_GAP * rate).ceil() as u32;
        PolyKey {
            down: None,
            open: gap,
            gap,
        }
    }

    /// The voice's gate and key for the next sample, played on `voice`'s keys.
    fn step(&mut self, gate: bool, key: i32, voice: &mut Voice) {
        if gate {
            match self.down {
                None if self.open >= self.gap => {
                    voice.note(key, true);
                    self.down = Some(key);
                }
                None => {}
                Some(old) if old != key => {
                    voice.note(key, true);
                    voice.note(old, false);
                    self.down = Some(key);
                }
                Some(_) => {}
            }
        } else if let Some(old) = self.down.take() {
            voice.note(old, false);
            self.open = 0;
        }
        if self.down.is_none() {
            self.open = self.open.saturating_add(1);
        }
    }

    /// Its key lifted at once (POLY switched, the voices rebuilt).
    fn lift(&mut self, voice: &mut Voice) {
        if let Some(old) = self.down.take() {
            voice.note(old, false);
        }
    }

    /// As on a voice at rest: no key down, the trigger contact open long enough.
    fn rest(&mut self) {
        self.down = None;
        self.open = self.gap;
    }
}

/// A POLY voice's place: its note, gate and age, whether it still sounds, kept by the
/// engine (events change them without touching the voice); and the voice as it plays,
/// shared with the workers ([`crate::pool`]).
#[derive(Debug)]
struct Slot {
    play: Shared,
    note: u8,
    gate: bool,
    /// The gate the voice saw at its last sample, and a one-sample drop pending (a new note
    /// on a gate that was high).
    shown: bool,
    drop: bool,
    age: u64,
    /// Whether it is played (from its note until it falls silent), and its output's peak
    /// over the block so far.
    active: bool,
    peak: f64,
    /// What waits for the voice while a worker late from an earlier run holds it: its key
    /// lifted, and the one instrument's keys pressed or released (POLY off), in order.
    lift: bool,
    notes: [(i32, bool); 32],
    pending: usize,
    /// To be put to rest once All Sound Off has faded the output out.
    rest: bool,
}

impl Slot {
    /// The run's gate, note and drop to its voice. A drop it was given for a run it did not
    /// play (a worker late, the run closed before it began) stays with it for its next.
    fn hand(&self, p: &mut Playing) {
        (p.gate, p.note) = (self.gate, self.note);
        p.drop |= self.drop;
    }

    /// After a run its voice played by the deadline (`ok`) or not: the drop is the voice's
    /// now; the gate it shows is the run's, or, not known to have played it, either that or the
    /// one before, so that a new note on it retriggers whichever it saw (decisions.md R18).
    fn ran(&mut self, ok: bool) {
        self.drop = false;
        self.shown = if ok {
            self.gate
        } else {
            self.shown || self.gate
        };
    }
}

/// A voice as it plays: the voice, its character and its key, and a run's gate, note and
/// drop in, its samples out.
#[derive(Debug)]
pub struct Playing {
    voice: Voice,
    character: Character,
    key: PolyKey,
    gate: bool,
    note: u8,
    drop: bool,
    /// The last run's samples (left, right) and its output's peak.
    out: [[f64; CHUNK]; 2],
    peak: f64,
    /// The engine's panel's generation it plays with ([`Engine::apply`]).
    panel_gen: u64,
    /// Its output went non-finite: silent until the engine puts a clean voice in its place
    /// ([`Engine::restore`]; decisions.md R18).
    broken: bool,
}

impl Playing {
    /// A run of `n` samples with `ext` at EXTERNAL INPUT: its key moved on a sample at a time
    /// (the drop at the first), its character's offsets and SPREAD's place; the samples in
    /// `out`, the peak in `peak`. A sample that is not finite marks the voice broken, and it
    /// is silent from there.
    pub fn play(&mut self, n: usize, ext: &[f64; CHUNK], rate: f64, m: &Mix) {
        let mut peak = 0.0f64;
        for (k, &x) in ext.iter().enumerate().take(n) {
            let gate = if self.drop {
                self.drop = false;
                false
            } else {
                self.gate
            };
            if self.broken {
                (self.out[0][k], self.out[1][k]) = (0.0, 0.0);
                continue;
            }
            self.key.step(gate, i32::from(self.note), &mut self.voice);
            let j = jacks(&mut self.character, x, rate, m);
            let mut y = self.voice.tick_jacks(&j);
            if !y.is_finite() {
                self.broken = true;
                y = 0.0;
            }
            peak = peak.max(y.abs());
            let (gl, gr) = self.character.gains(m.spread);
            self.out[0][k] = y * f64::from(gl);
            self.out[1][k] = y * f64::from(gr);
        }
        self.peak = peak;
    }
}

/// A clean voice for each voice's place, made off the audio thread, to take a voice's place
/// on it without allocating (decisions.md R18): a voice whose output went non-finite, and
/// every sounding voice at All Sound Off. The voice taken out is dropped off the audio thread
/// too, when the spares are next mended ([`Spares::mend`]). The audio thread only tries their
/// locks.
#[derive(Debug, Default)]
pub struct Spares {
    places: [Mutex<Spare>; POLY_VOICES.2],
}

/// A place's spare voice: clean, or the voice it replaced.
#[derive(Debug, Default)]
struct Spare {
    voice: Option<Voice>,
    clean: bool,
}

impl Spares {
    /// Each place whose voice was taken given a clean one again, at its rate, and the one
    /// taken out dropped. Not on the audio thread: the plug-in has its helper thread do it when
    /// the engine asks ([`Engine::take_mending`]; decisions.md R23).
    pub fn mend(&self) {
        self.mend_until(|| false);
    }

    /// As [`Spares::mend`], stopping before the next voice once `stop` (the plug-in dropped:
    /// decisions.md R23), so that waiting for it takes at most one voice's making.
    pub fn mend_until(&self, stop: impl Fn() -> bool) {
        for place in &self.places {
            if stop() {
                return;
            }
            let used = place
                .lock()
                .ok()
                .filter(|s| !s.clean)
                .and_then(|s| s.voice.as_ref().map(Voice::rate));
            if let Some(rate) = used {
                fill(place, rate, |s| {
                    !s.clean && s.voice.as_ref().map(Voice::rate) == Some(rate)
                });
            }
        }
    }

    /// Every place given a clean voice at `rate` Hz (the voices' own) unless it has one.
    fn make(&self, rate: f64) {
        for place in &self.places {
            fill(place, rate, |s| {
                !(s.clean && s.voice.as_ref().map(Voice::rate) == Some(rate))
            });
        }
    }
}

/// A clean voice at `rate` Hz put in `place` while it is `wanted` there: made before the lock
/// is taken, and the voice it replaces dropped after.
fn fill(place: &Mutex<Spare>, rate: f64, wanted: impl Fn(&Spare) -> bool) {
    if !place.lock().is_ok_and(|s| wanted(&s)) {
        return;
    }
    let mut voice = Some(Voice::prototype(rate));
    if let Ok(mut s) = place.lock()
        && wanted(&s)
    {
        std::mem::swap(&mut s.voice, &mut voice);
        s.clean = true;
    }
    drop(voice);
}

/// The voices, the MIDI input's state and the controls they play with.
#[derive(Debug)]
pub struct Engine {
    slots: Vec<Slot>,
    over: Option<Oversampled>,
    rate: f64,
    seed: u64,
    controls: Controls,
    gain: f64,
    /// Keys held (MIDI numbers), played on by voices built for a new rate (POLY off).
    held: [bool; 128],
    /// POLY's notes taken so far (each voice's age).
    counter: u64,
    bend: f32,
    modulation: f32,
    lamp: f32,
    /// The output's gain under POWER (0 off, 1 on), and its step a sample while it fades.
    fade: f64,
    fade_step: f64,
    /// All Sound Off under way (the output fading out, or out and the voices being put to
    /// rest), its fade's step a sample and the samples it has waited, faded out.
    hush: bool,
    hush_step: f64,
    hush_waited: usize,
    /// The clean voices put in voices' places, and whether some are to be made again.
    spares: Arc<Spares>,
    mend: bool,
    /// The panel's generation (each [`Engine::apply`] a new one), and the voices'
    /// preamplifier inside FEEDBACK's loop.
    panel_gen: u64,
    in_loop: ca72::preamp::InLoop,
    /// The workers sharing POLY's voices (none: every voice on the caller's thread), how many
    /// were asked for and the block period they were made audio threads for; and the
    /// present block's deadline for them.
    pool: Option<Pool>,
    workers: (usize, Option<Duration>),
    deadline: Option<Instant>,
    /// Where POLY's workers are started and stopped off the audio thread, the plan they are
    /// started for there (0: none), this thread's ask for them and whether the helper
    /// thread is to serve it (decisions.md R21, R23).
    crew: Arc<Crew>,
    plan: u64,
    ask: Ask,
    serve: bool,
    /// How often the workers were started (for the tests).
    #[cfg(test)]
    pool_starts: u64,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

/// How a voice's place reaches its voice: `f` on it at once, unless a worker late from an
/// earlier run holds it.
fn with_voice<T>(s: &Slot, f: impl FnOnce(&mut Playing) -> T) -> Option<T> {
    s.play.try_lock().ok().map(|mut p| f(&mut p))
}

impl Engine {
    /// An engine without voices (silent until [`Engine::prepare`]).
    pub fn new() -> Engine {
        let controls = Controls::default();
        Engine {
            slots: Vec::new(),
            over: None,
            rate: 0.0,
            seed: 0,
            gain: output_gain(controls.volume, controls.main_output),
            controls,
            held: [false; 128],
            counter: 0,
            bend: 0.0,
            modulation: 0.0,
            lamp: 0.0,
            fade: 1.0,
            fade_step: 1.0,
            hush: false,
            hush_step: 1.0,
            hush_waited: 0,
            spares: Arc::new(Spares::default()),
            mend: false,
            panel_gen: 0,
            in_loop: ca72::preamp::InLoop::default(),
            pool: None,
            workers: (0, None),
            deadline: None,
            crew: Arc::new(Crew::new()),
            plan: 0,
            ask: Ask::Not,
            serve: false,
            #[cfg(test)]
            pool_starts: 0,
        }
    }

    /// The voices for `rate` Hz, their noise seeded from `seed` (the first voice's is `seed`
    /// itself, as with one voice), playing the keys held, and their spares ([`Spares`]). Not
    /// on the audio thread: voices at a rate not yet built take up to a second (the
    /// oscillators' factory tuning, the tables); later ones are copies. At the rate they were
    /// built for, the voices are kept, their noise and (for a new seed) their characters
    /// drawn from `seed` again, as a session's saved seed loaded into a running instance has
    /// them (decisions.md R18). New voices stop the workers, here ([`Engine::start_workers`]
    /// plans them again).
    pub fn prepare(&mut self, rate: f64, seed: u64) {
        let reseeded = seed != self.seed;
        self.seed = seed;
        if rate == self.rate && !self.slots.is_empty() {
            for (k, s) in self.slots.iter_mut().enumerate() {
                if let Ok(mut p) = s.play.lock() {
                    p.voice.set_seed(voice_seed(seed, k));
                    if reseeded {
                        p.character = Character::new(voice_seed(seed, k), k, 3, ENTROPY_KNOBS);
                    }
                }
            }
            if reseeded {
                self.apply();
            }
            self.spares.make(self.voice_rate());
            return;
        }
        self.stop_workers();
        self.rate = rate;
        self.fade_step = (1.0 / (POWER_FADE * rate)).min(1.0);
        self.hush_step = (1.0 / (HUSH_FADE * rate)).min(1.0);
        self.hush = false;
        let factor = [1, 2, 4, 8]
            .into_iter()
            .find(|f| rate * *f as f64 >= MIN_VOICE_RATE)
            .unwrap_or(8);
        self.over = (factor > 1).then(|| Oversampled {
            factor,
            up: Interpolator::new(factor),
            down: [Decimator::new(factor), Decimator::new(factor)],
        });
        let voice_rate = rate * factor as f64;
        self.slots = (0..POLY_VOICES.2)
            .map(|k| {
                // (Each its own copy from the prototype, its key lists' room reserved: a key
                // press must not allocate on the audio thread.)
                let mut voice = Voice::prototype(voice_rate);
                voice.set_seed(voice_seed(seed, k));
                Slot {
                    play: Arc::new(Mutex::new(Playing {
                        voice,
                        character: Character::new(voice_seed(seed, k), k, 3, ENTROPY_KNOBS),
                        key: PolyKey::new(voice_rate),
                        gate: false,
                        note: 0,
                        drop: false,
                        out: [[0.0; CHUNK]; 2],
                        peak: 0.0,
                        panel_gen: u64::MAX,
                        broken: false,
                    })),
                    note: 0,
                    gate: false,
                    shown: false,
                    drop: false,
                    age: 0,
                    active: false,
                    peak: 0.0,
                    lift: false,
                    notes: [(0, false); 32],
                    pending: 0,
                    rest: false,
                }
            })
            .collect();
        // (Each voice's lock taken once here, off the audio thread: on macOS the standard
        // library may make a mutex's lock on its first use; history.md, "The worker locks".
        // The spares' are taken as they are made.)
        for s in &self.slots {
            drop(s.play.lock());
        }
        self.spares.make(voice_rate);
        self.apply();
        if !self.controls.poly
            && let Ok(mut p) = self.slots[0].play.lock()
        {
            for (key, _) in self.held.iter().enumerate().filter(|(_, h)| **h) {
                p.voice.note(key as i32, true);
            }
        }
    }

    /// `workers` threads of the plug-in's own to share POLY's voices with the caller's
    /// thread, made audio threads for blocks of `period` (None: left ordinary, for
    /// measurements that run as fast as they go; decisions.md R11); 0, none. Held only while
    /// POLY is on (R21): started here if it is, else when it is switched on, off the audio
    /// thread ([`Crew::serve`]); let go when it is switched off. Not on the audio thread.
    /// Workers already running for these voices, as many and for the same period, are kept
    /// while POLY stays on (a host's state loaded into a running instance prepares it again:
    /// decisions.md R18). Returns how many run (none with POLY off, or if the system would not
    /// start them).
    pub fn start_workers(&mut self, workers: usize, period: Option<Duration>) -> usize {
        if self.pool.is_some() && self.workers == (workers, period) && self.controls.poly {
            return self.workers();
        }
        self.stop_workers();
        self.workers = (workers, period);
        if workers == 0 || self.slots.is_empty() {
            return 0;
        }
        let voices: Vec<Shared> = self.slots.iter().map(|s| s.play.clone()).collect();
        self.plan = self.crew.plan(&voices, workers, period);
        if self.controls.poly {
            self.pool = self.crew.start(self.plan);
            self.ask = Ask::Answered;
            #[cfg(test)]
            {
                self.pool_starts += 1;
            }
        }
        self.workers()
    }

    /// POLY's workers stopped, here, their share of the process's budget given back, and none
    /// started again until [`Engine::start_workers`] (a plug-in deactivated: decisions.md
    /// R21). Not on the audio thread.
    pub fn stop_workers(&mut self) {
        self.pool = None;
        self.crew.clear();
        (self.plan, self.ask) = (0, Ask::Not);
    }

    /// At a block's start, on the audio thread: POLY's workers as POLY asks (decisions.md
    /// R21), without waiting. POLY switched on, a pool asked of the helper thread
    /// ([`Engine::take_serving`]) and taken once it is ready, every voice played here until
    /// then; switched off, the pool let go, to be stopped there.
    fn follow_poly(&mut self) {
        let on = self.controls.poly;
        if self
            .crew
            .follow(on, self.plan, &mut self.pool, &mut self.ask)
        {
            self.serve = true;
        }
    }

    /// Where POLY's workers are started and stopped off the audio thread ([`Crew::serve`]).
    pub fn crew(&self) -> Arc<Crew> {
        self.crew.clone()
    }

    /// Whether POLY's workers are to be started or stopped ([`Crew::serve`], off the audio
    /// thread) since this was last asked.
    pub fn take_serving(&mut self) -> bool {
        std::mem::take(&mut self.serve)
    }

    /// The clean voices put in voices' places ([`Spares`]): for mending off the audio thread.
    pub fn spares(&self) -> Arc<Spares> {
        self.spares.clone()
    }

    /// Whether the spares are to be mended ([`Spares::mend`], off the audio thread) since this
    /// was last asked.
    pub fn take_mending(&mut self) -> bool {
        std::mem::take(&mut self.mend)
    }

    /// The workers sharing POLY's voices.
    pub fn workers(&self) -> usize {
        self.pool.as_ref().map_or(0, Pool::workers)
    }

    /// When the present block's voices must be done by: a voice a worker has not finished
    /// then is silent for the run, not waited for (None: waited for). The plug-in sets it at
    /// each block's start, when POLY's workers also follow POLY ([`Engine::follow_poly`]).
    pub fn set_deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
        self.follow_poly();
        if let Some(p) = &self.pool {
            p.block();
        }
    }

    /// The controls for the samples that follow. POLY switched lifts every key first (the
    /// notes then sounding stop; the next notes play in the new way).
    pub fn set(&mut self, c: &Controls) {
        if *c != self.controls {
            if c.poly != self.controls.poly {
                self.lift_all();
            }
            if c.voices < self.controls.voices {
                // The voices past the new count let go (they run on until silent).
                for s in self.slots.iter_mut().skip(c.voices.max(1)) {
                    s.gate = false;
                }
            }
            self.controls = *c;
            self.gain = output_gain(c.volume, c.main_output)
                * if c.lock { lock_trim(&c.panel) } else { 1.0 };
            self.apply();
        }
    }

    /// The one instrument's key pressed or released (POLY off), on the first voice (once it
    /// is free, if a worker late from an earlier run holds it).
    fn one_note(&mut self, key: i32, on: bool) {
        let s = &mut self.slots[0];
        if with_voice(s, |p| p.voice.note(key, on)).is_none() && s.pending < s.notes.len() {
            s.notes[s.pending] = (key, on);
            s.pending += 1;
        }
    }

    pub fn event(&mut self, e: Event) {
        match e {
            Event::Note { key, on } => {
                let Some(h) = self.held.get_mut(usize::from(key)) else {
                    return;
                };
                *h = on;
                if self.slots.is_empty() {
                    return;
                }
                if !self.controls.poly {
                    self.one_note(i32::from(key), on);
                } else if on {
                    self.poly_on(key);
                } else {
                    for s in &mut self.slots {
                        if s.gate && s.note == key {
                            s.gate = false;
                        }
                    }
                }
            }
            Event::PitchBend(b) => {
                self.bend = b.clamp(-1.0, 1.0);
                self.apply();
            }
            Event::Modulation(m) => {
                self.modulation = m.clamp(0.0, 1.0);
                self.apply();
            }
            Event::ResetControllers => {
                (self.bend, self.modulation) = (0.0, 0.0);
                self.apply();
            }
            Event::AllNotesOff => self.release_all(),
            Event::AllSoundOff => self.all_sound_off(),
        }
    }

    /// All Sound Off (decisions.md R18): every key released and the output faded out over
    /// [`HUSH_FADE`]; then each voice that sounded is put to rest ([`Engine::settle`]) and the
    /// output fades back in. A voice taken for a note meanwhile plays on.
    fn all_sound_off(&mut self) {
        self.release_all();
        let poly = self.controls.poly;
        for (k, s) in self.slots.iter_mut().enumerate() {
            s.rest = if poly { s.active } else { k == 0 };
        }
        self.hush = !self.slots.is_empty();
        self.hush_waited = 0;
    }

    /// Once All Sound Off has faded the output out, the voices it marked put to rest, a clean
    /// voice in each one's place ([`Engine::restore`]), and the output let back in; a voice
    /// whose clean voice is not yet made is waited for, silent, up to [`HUSH_WAIT`]. At the
    /// start of each run (one sample, or the samples between events).
    fn settle(&mut self) {
        if !self.hush || self.fade > 0.0 {
            return;
        }
        let poly = self.controls.poly;
        let mut waiting = false;
        for k in 0..self.slots.len() {
            if !self.slots[k].rest {
                continue;
            }
            if poly && self.slots[k].gate {
                self.slots[k].rest = false;
                continue;
            }
            let play = self.slots[k].play.clone();
            let rested = play.try_lock().is_ok_and(|mut p| self.restore(k, &mut p));
            let s = &mut self.slots[k];
            if rested {
                s.rest = false;
                if poly {
                    (s.active, s.peak) = (false, 0.0);
                }
            } else {
                waiting = true;
            }
        }
        if !waiting || self.hush_waited as f64 >= HUSH_WAIT * self.rate {
            self.hush = false;
            for s in &mut self.slots {
                s.rest = false;
            }
        }
    }

    /// Voice `k` put back to rest without allocating (decisions.md R18): a clean voice in its
    /// place from its spare ([`Spares`]), its noise seeded, the engine's panel, its key up
    /// (pressed again if its gate is high; POLY off, the keys held pressed again). False if
    /// the spare has not been made again since it was last used. Either way the spares are
    /// then to be mended ([`Engine::take_mending`]).
    fn restore(&mut self, k: usize, p: &mut Playing) -> bool {
        self.mend = true;
        let rate = p.voice.rate();
        let Ok(mut spare) = self.spares.places[k].try_lock() else {
            return false;
        };
        if !spare.clean {
            return false;
        }
        let Some(clean) = spare.voice.as_mut().filter(|v| v.rate() == rate) else {
            return false;
        };
        std::mem::swap(&mut p.voice, clean);
        spare.clean = false;
        drop(spare);
        p.voice.set_seed(voice_seed(self.seed, k));
        p.key.rest();
        (p.broken, p.drop) = (false, false);
        let s = &mut self.slots[k];
        (s.lift, s.pending, s.shown) = (false, 0, false);
        if !self.controls.poly && k == 0 {
            for (key, _) in self.held.iter().enumerate().filter(|(_, h)| **h) {
                p.voice.note(key as i32, true);
            }
        }
        self.voice_panel(p, &self.panel());
        true
    }

    /// A POLY note: a free voice by its number, else the oldest let go, else the oldest held
    /// (its key lifted and pressed again: `PolyKey`).
    fn poly_on(&mut self, key: u8) {
        let n = self.voices();
        let pick = self.slots[..n]
            .iter()
            .position(|s| !s.active)
            .or_else(|| {
                (0..n)
                    .filter(|&k| !self.slots[k].gate)
                    .min_by_key(|&k| self.slots[k].age)
            })
            .or_else(|| (0..n).min_by_key(|&k| self.slots[k].age))
            .unwrap_or(0);
        self.counter += 1;
        let s = &mut self.slots[pick];
        s.drop = s.active && s.shown;
        (s.note, s.gate, s.age, s.active, s.peak) = (key, true, self.counter, true, 0.0);
    }

    /// The voices POLY plays.
    fn voices(&self) -> usize {
        self.controls
            .voices
            .clamp(POLY_VOICES.0, POLY_VOICES.2)
            .min(self.slots.len())
    }

    /// Every held key released (the contours then run out as the DECAY switch has them).
    pub fn release_all(&mut self) {
        for key in 0..self.held.len() {
            if std::mem::take(&mut self.held[key]) && !self.controls.poly && !self.slots.is_empty()
            {
                self.one_note(key as i32, false);
            }
        }
        for s in &mut self.slots {
            s.gate = false;
        }
    }

    /// Every voice's key lifted at once and the held keys forgotten (POLY switched).
    fn lift_all(&mut self) {
        let poly = self.controls.poly;
        for k in 0..self.slots.len() {
            let s = &mut self.slots[k];
            if with_voice(s, |p| p.key.lift(&mut p.voice)).is_none() {
                s.lift = true;
            }
            s.gate = false;
            if k == 0 && !poly {
                for key in 0..self.held.len() {
                    if self.held[key] {
                        self.one_note(key as i32, false);
                    }
                }
            }
        }
        self.held = [false; 128];
    }

    /// One output sample (the left and right channels together, as on a mono output) with
    /// `ext` at EXTERNAL INPUT (1.0 the same volts as the output's 1.0).
    pub fn tick(&mut self, ext: f32) -> f32 {
        let (l, r) = self.tick_stereo(ext);
        0.5 * (l + r)
    }

    /// One output sample, left and right, with `ext` at EXTERNAL INPUT (a sample that is not
    /// finite taken as 0: decisions.md R18); silence before [`Engine::prepare`]. With POWER
    /// off, once faded out, the voices rest (the MIDI input still reaches them).
    pub fn tick_stereo(&mut self, ext: f32) -> (f32, f32) {
        self.settle();
        let on = self.controls.power;
        if (!on && self.fade == 0.0) || self.slots.is_empty() {
            return (0.0, 0.0);
        }
        let ext = if ext.is_finite() { ext } else { 0.0 };
        let rate = self.voice_rate();
        let (l, r) = match self.over.take() {
            None => self.sample(f64::from(ext), rate),
            Some(mut o) => {
                let mut ins = [0.0; 8];
                o.up.push(f64::from(ext), &mut ins[..o.factor]);
                let (mut l, mut r) = (0.0, 0.0);
                for x in &ins[..o.factor] {
                    let (a, b) = self.sample(*x, rate);
                    if let Some(d) = o.down[0].push(a) {
                        l = d;
                    }
                    if let Some(d) = o.down[1].push(b) {
                        r = d;
                    }
                }
                self.over = Some(o);
                (l, r)
            }
        };
        self.faded(l, r)
    }

    /// `left.len()` output samples, left and right, with `ext[i]` at EXTERNAL INPUT (an empty
    /// `ext`: nothing there; a sample that is not finite, 0) and no event among them: the same
    /// samples, to the bit, as as many [`Engine::tick_stereo`]. With POLY the voices play
    /// [`CHUNK`] samples at a time, one after another (a voice's state, some 30 KB, then stays
    /// in the processor's nearest cache), or shared with the workers, and are summed in their
    /// order (decisions.md R11).
    pub fn render(&mut self, ext: &[f32], left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        let at = |i: usize| ext.get(i).copied().filter(|x| x.is_finite()).unwrap_or(0.0);
        if !self.controls.poly || self.over.is_some() || self.slots.is_empty() {
            for i in 0..len {
                (left[i], right[i]) = self.tick_stereo(at(i));
            }
            return;
        }
        let rate = self.voice_rate();
        let mix = self.mix();
        let mut start = 0;
        while start < len {
            self.settle();
            let mut n = (len - start).min(CHUNK);
            // The samples the voices play: all of them, unless POWER is off, when they rest
            // from the sample the fade reaches 0. All Sound Off's run ends at that sample, its
            // voices put to rest from the next, as a sample at a time has them.
            let mut run = n;
            if !self.controls.power || self.hush {
                let (mut fade, mut out) = (self.fade, 0);
                while out < n && fade != 0.0 {
                    fade = self.fade_on(fade);
                    out += 1;
                }
                if self.hush && self.fade > 0.0 {
                    n = out;
                }
                run = if self.controls.power { n } else { out };
            }
            let mut ins = [0.0f64; CHUNK];
            for (k, x) in ins.iter_mut().enumerate().take(run) {
                *x = f64::from(at(start + k));
            }
            let (mut l, mut r) = ([0.0f64; CHUNK], [0.0f64; CHUNK]);
            if run > 0 {
                self.run(run, &ins, rate, &mix, true, &mut l, &mut r);
            }
            for k in 0..n {
                (left[start + k], right[start + k]) = if k < run {
                    self.faded(l[k], r[k])
                } else {
                    (0.0, 0.0)
                };
            }
            start += n;
        }
    }

    /// POLY's sounding voices over `n` samples, summed into `l` and `r` in their order: each
    /// given its gate, note and drop and brought up to the engine's panel, then played here,
    /// or shared with the workers (with `share`, and a run long enough to be worth it). A
    /// voice a worker late from an earlier run still holds sits the run out.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        n: usize,
        ext: &[f64; CHUNK],
        rate: f64,
        mix: &Mix,
        share: bool,
        l: &mut [f64; CHUNK],
        r: &mut [f64; CHUNK],
    ) {
        let mut list = [0usize; MAX];
        let mut count = 0;
        for k in 0..self.slots.len() {
            if !self.slots[k].active || count == MAX {
                continue;
            }
            let play = self.slots[k].play.clone();
            let Ok(mut p) = play.try_lock() else {
                self.slots[k].peak = self.slots[k].peak.max(SILENT);
                continue;
            };
            self.bring_up(k, &mut p);
            self.slots[k].hand(&mut p);
            drop(p);
            list[count] = k;
            count += 1;
        }
        let deadline = self.deadline;
        let played = match &mut self.pool {
            Some(pool) if share && count > 1 && n >= SHARED_RUN => {
                pool.play(&list[..count], n, ext, rate, mix, deadline)
            }
            _ => {
                let mut played = [false; MAX];
                for (ok, &k) in played.iter_mut().zip(&list[..count]) {
                    if let Ok(mut p) = self.slots[k].play.try_lock() {
                        p.play(n, ext, rate, mix);
                        *ok = true;
                    }
                }
                played
            }
        };
        for (&ok, &k) in played.iter().zip(&list[..count]) {
            let s = &mut self.slots[k];
            s.ran(ok);
            // (A voice not heard this run is not known to have fallen silent: it is not freed
            // for it at the block's end. decisions.md R18.)
            let p = if ok { s.play.try_lock().ok() } else { None };
            let Some(p) = p else {
                s.peak = s.peak.max(SILENT);
                continue;
            };
            for i in 0..n {
                l[i] += p.out[0][i];
                r[i] += p.out[1][i];
            }
            s.peak = s.peak.max(p.peak);
        }
    }

    /// A voice brought up to what the engine asked of it meanwhile: a clean voice in its place
    /// if it broke ([`Engine::restore`]), its key lifted, the one instrument's keys, and the
    /// panel.
    fn bring_up(&mut self, k: usize, p: &mut Playing) {
        if p.broken {
            self.restore(k, p);
        }
        let s = &mut self.slots[k];
        if std::mem::take(&mut s.lift) {
            p.key.lift(&mut p.voice);
        }
        for &(key, on) in &s.notes[..s.pending] {
            p.voice.note(key, on);
        }
        s.pending = 0;
        if p.panel_gen != self.panel_gen {
            self.voice_panel(p, &self.panel());
        }
    }

    /// A sample of the voices' sum, left and right, through MAIN OUTPUT's gain and POWER's
    /// or All Sound Off's fade (moved a sample on). Never a sample that is not finite
    /// (decisions.md R18): 0 instead, and the resamplers, which may hold one, start again.
    fn faded(&mut self, l: f64, r: f64) -> (f32, f32) {
        self.fade = self.fade_on(self.fade);
        if self.hush && self.fade == 0.0 {
            self.hush_waited += 1;
        }
        // (As the one voice was: the gain, then the fade.)
        let (l, r) = (
            (l * self.gain * self.fade) as f32,
            (r * self.gain * self.fade) as f32,
        );
        if l.is_finite() && r.is_finite() {
            return (l, r);
        }
        if let Some(o) = &mut self.over {
            o.up.reset();
            o.down.iter_mut().for_each(Decimator::reset);
        }
        (0.0, 0.0)
    }

    /// The output's gain under POWER and All Sound Off a sample after `fade`: rising while
    /// POWER is on, else falling (All Sound Off's faster).
    fn fade_on(&self, fade: f64) -> f64 {
        if self.controls.power && !self.hush {
            (fade + self.fade_step).min(1.0)
        } else if self.hush {
            (fade - self.hush_step).max(0.0)
        } else {
            (fade - self.fade_step).max(0.0)
        }
    }

    /// How ENTROPY and SPREAD stand for the voices' samples.
    fn mix(&self) -> Mix {
        let entropy = self.controls.entropy * ENTROPY_DEPTH;
        // The oscillators': their own floor and ENTROPY's, unless LOCK; the cutoff's, ENTROPY's.
        let oscillators = if self.controls.lock {
            0.0
        } else {
            ENTROPY_FLOOR + entropy
        };
        Mix {
            entropy,
            oscillators,
            at: oscillators.max(entropy),
            spread: self.controls.spread,
        }
    }

    /// One sample of the voices at their rate (`rate` Hz): left and right.
    fn sample(&mut self, ext: f64, rate: f64) -> (f64, f64) {
        let mix = self.mix();
        if !self.controls.poly {
            let play = self.slots[0].play.clone();
            let Ok(mut p) = play.try_lock() else {
                return (0.0, 0.0);
            };
            self.bring_up(0, &mut p);
            if p.broken {
                return (0.0, 0.0);
            }
            let j = jacks(&mut p.character, ext, rate, &mix);
            let y = p.voice.tick_jacks(&j);
            if !y.is_finite() {
                p.broken = true;
                return (0.0, 0.0);
            }
            let (gl, gr) = p.character.gains(mix.spread);
            return (y * f64::from(gl), y * f64::from(gr));
        }
        let mut ins = [0.0f64; CHUNK];
        ins[0] = ext;
        let (mut l, mut r) = ([0.0f64; CHUNK], [0.0f64; CHUNK]);
        self.run(1, &ins, rate, &mix, false, &mut l, &mut r);
        (l[0], r[0])
    }

    fn voice_rate(&self) -> f64 {
        self.rate * self.over.as_ref().map_or(1, |o| o.factor) as f64
    }

    /// After a block of `len` samples: the OVERLOAD lamp (0 off, 1 fully lit), its peak held
    /// and falling with a time constant of [`LAMP_FALL`]; POLY's voices let go and fallen
    /// silent over the block freed.
    pub fn end_block(&mut self, len: usize) -> f32 {
        let mut peak = 0.0f64;
        for s in &mut self.slots {
            if let Some(p) = with_voice(s, |p| p.voice.take_overload_peak()) {
                peak = peak.max(p);
            }
            if s.active && !s.gate && s.peak < SILENT {
                s.active = false;
            }
            s.peak = 0.0;
        }
        let fall = (-(len as f32) / (self.rate as f32 * LAMP_FALL)).exp();
        let fall = if fall.is_finite() { fall } else { 0.0 };
        self.lamp = (self.lamp * fall).max(peak.clamp(0.0, 1.0) as f32);
        self.lamp
    }

    pub fn lamp(&self) -> f32 {
        self.lamp
    }

    /// The voices sounding now (POLY on), or 1 (off).
    pub fn sounding(&self) -> usize {
        if self.controls.poly {
            self.slots.iter().filter(|s| s.active).count()
        } else {
            1
        }
    }

    /// The wheels where the parameters and the MIDI input leave them: PITCH -1..1 and
    /// MODULATION 0..1.
    pub fn wheels(&self) -> (f64, f64) {
        let p = self.panel();
        (p.pitch_wheel, p.mod_wheel)
    }

    /// Where a MIDI keyboard's wheels move the panel's, as the panel draws them: its pitch
    /// bend's share of the PITCH wheel's travel (0..1, 0.5 at rest) and its modulation wheel
    /// (0..1).
    pub fn midi_wheels(&self) -> (f32, f32) {
        let share = (self.controls.bend_range / PITCH_WHEEL_SEMITONES).clamp(0.0, 1.0);
        let share = if share.is_finite() { share as f32 } else { 0.0 };
        (0.5 + self.bend * share / 2.0, self.modulation)
    }

    /// The panel the voices play: the controls', in Potato, the MIDI input's bend moving
    /// the PITCH wheel by its share of the wheel's travel and its modulation wheel holding
    /// MODULATION up to its own position.
    fn panel(&self) -> Panel {
        let c = &self.controls;
        let share = (c.bend_range / PITCH_WHEEL_SEMITONES).clamp(0.0, 1.0);
        let share = if share.is_finite() { share } else { 0.0 };
        let mut p = c.panel;
        p.quality = Quality::Potato;
        p.pitch_wheel = (p.pitch_wheel + f64::from(self.bend) * share).clamp(-1.0, 1.0);
        p.mod_wheel = p.mod_wheel.max(f64::from(self.modulation));
        p
    }

    /// How every voice's preamplifier is solved inside FEEDBACK's loop (for comparing the
    /// models: decisions.md R11).
    pub fn set_in_loop(&mut self, m: ca72::preamp::InLoop) {
        self.in_loop = m;
        self.apply();
    }

    /// The panel to every voice (each with its tolerances under ENTROPY): a new generation,
    /// which a voice a worker still holds takes before it next plays.
    fn apply(&mut self) {
        self.panel_gen = self.panel_gen.wrapping_add(1);
        let p = self.panel();
        for k in 0..self.slots.len() {
            let play = self.slots[k].play.clone();
            if let Ok(mut v) = play.try_lock() {
                self.voice_panel(&mut v, &p);
            }
        }
    }

    /// A voice's panel `p` with its tolerances, FEEDBACK and its loop's preamplifier.
    fn voice_panel(&self, v: &mut Playing, p: &Panel) {
        let entropy = self.controls.entropy * ENTROPY_DEPTH;
        v.voice.feedback = self.controls.feedback.clamp(0.0, 1.0);
        v.voice.in_loop = self.in_loop;
        let pv = if entropy > 0.0 {
            entropy_panel(&v.character, *p, entropy)
        } else {
            *p
        };
        if v.voice.panel != pv {
            v.voice.panel = pv;
        }
        v.panel_gen = self.panel_gen;
    }
}

/// How many workers share POLY's voices on this machine (decisions.md R11): a third of its
/// processors less two, at most four (four on the Mac's 14, two on 8), none below 5.
pub fn default_workers() -> usize {
    let n = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    (n.saturating_sub(2) / 3).min(4)
}

/// When a block's voices must be done by, as a share of its period from its start: a voice a
/// worker has not finished then is silent for its run (decisions.md R11). Normally they are
/// done long before.
pub const DEADLINE: f64 = 0.75;

/// The shortest run worth sharing with the workers, samples (waking them takes some
/// microseconds).
const SHARED_RUN: usize = 8;

/// Samples [`Engine::render`] plays a voice for at a time.
pub const CHUNK: usize = 128;

/// ENTROPY and SPREAD as the voices' samples take them: ENTROPY's depth, the oscillators'
/// (their floor with it, unless LOCK), the depth the character's offsets are drawn at, and
/// SPREAD.
#[derive(Debug, Clone, Copy)]
pub struct Mix {
    entropy: f64,
    oscillators: f64,
    at: f64,
    spread: f64,
}

impl Mix {
    pub fn to_array(self) -> [f64; 4] {
        [self.entropy, self.oscillators, self.at, self.spread]
    }

    pub fn from_array([entropy, oscillators, at, spread]: [f64; 4]) -> Mix {
        Mix {
            entropy,
            oscillators,
            at,
            spread,
        }
    }
}

/// A voice's jacks for its next sample: `ext` at EXTERNAL INPUT, and its character's offsets
/// (a sample on).
fn jacks(c: &mut Character, ext: f64, rate: f64, m: &Mix) -> Jacks {
    let mut j = Jacks {
        ext,
        ..Jacks::default()
    };
    if m.at > 0.0 {
        let (cents, cutoff) = c.offsets(m.at, rate);
        j.detune = cents.map(|c| c * m.oscillators / m.at);
        j.cutoff = cutoff * m.entropy / m.at;
    }
    j
}

/// The share of a block's period the voices may take: the rest is the host's, its other
/// plug-ins' and the machine's own jitter (the DAW's figure).
pub const HEADROOM: f64 = 0.6;

/// How many POLY voices this machine plays in real time at `rate` Hz in blocks of `block`
/// frames with `workers` workers (decisions.md, "POLY in real time"; R11): the engine itself,
/// its workers made audio threads, played paced at the block's period on an audio thread of
/// its own for a moment, as a host plays it: chords of as many notes as voices tried, a new
/// one every few blocks (each taking voices still sounding, as legato), gliding, oscillators
/// 2 and 3 on, oscillator 3 on the filter through the MODULATION wheel, ENTROPY at half. The
/// count is the most voices whose blocks where the keys change take, at their median, no
/// more than [`HEADROOM`] of the period. With `feedback`, FEEDBACK on (at half, into the
/// external input at half VOLUME). About a fifth of a second a count tried, ten first, then
/// the estimate from it downward; once a process for each rate, block, FEEDBACK and number
/// of workers. Not on the audio thread.
pub fn realtime_voices(rate: f64, block: usize, feedback: bool, workers: usize) -> u32 {
    /// A measurement: the rate's bits, the block, FEEDBACK, the workers, the voices.
    type Measured = (u64, usize, bool, usize, u32);
    static MEASURED: std::sync::OnceLock<Mutex<Vec<Measured>>> = std::sync::OnceLock::new();
    let cache = MEASURED.get_or_init(|| Mutex::new(Vec::new()));
    let key = (rate.to_bits(), block, feedback, workers);
    if let Some(n) = cache
        .lock()
        .ok()
        .and_then(|c| c.iter().find(|e| (e.0, e.1, e.2, e.3) == key).map(|e| e.4))
    {
        return n;
    }
    let n = std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("ca72-measure".to_owned())
            .spawn_scoped(s, || measure_voices(rate, block, feedback, workers))
            .ok()
            .and_then(|h| h.join().ok())
            .unwrap_or(0)
    });
    if let Ok(mut c) = cache.lock() {
        c.push((key.0, key.1, key.2, key.3, n));
    }
    n
}

/// [`realtime_voices`]' measurement, on a thread of its own that it makes an audio thread.
fn measure_voices(rate: f64, block: usize, feedback: bool, workers: usize) -> u32 {
    let block = block.max(1);
    let period = Duration::from_secs_f64(block as f64 / rate.max(1.0));
    let mut c = Controls {
        poly: true,
        voices: POLY_VOICES.2,
        entropy: 0.5,
        ..Controls::default()
    };
    c.panel.osc[1].on = true;
    c.panel.osc[2].on = true;
    c.panel.filter_mod = true;
    c.panel.mod_wheel = 0.4;
    c.panel.glide = 0.15;
    c.panel.glide_on = true;
    c.panel.emphasis = 0.5;
    if feedback {
        c.panel.ext_on = true;
        c.panel.ext_volume = 0.5;
        c.feedback = 0.5;
    }
    // (Paced and bounded: a fifth of a second a count.)
    let _ = ca72_rt::promote(period, period / 2);
    let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
    let mut held: Vec<u8> = Vec::with_capacity(POLY_VOICES.2);
    let mut chord = 0usize;
    // The median time of a count's blocks where the keys change, on an engine of its own
    // (no voices of the last count still sounding).
    let mut trial = |voices: usize| -> Duration {
        c.voices = voices;
        let mut e = Engine::new();
        e.set(&c);
        e.prepare(rate, 1);
        e.start_workers(workers, Some(period));
        held.clear();
        let blocks = ((0.2 / period.as_secs_f64()) as usize).clamp(12, 64);
        let every = (blocks / 6).max(2);
        let mut changes = Vec::with_capacity(8);
        let start = Instant::now();
        for i in 0..blocks + every {
            if let Some(wait) = (start + period * i as u32).checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            let t = Instant::now();
            e.set_deadline(Some(t + period.mul_f64(DEADLINE)));
            let change = i % every == 0;
            if change {
                for key in held.drain(..) {
                    e.event(Event::Note { key, on: false });
                }
                const ROOTS: [u8; 6] = [45, 41, 43, 48, 40, 46];
                const SHAPE: [u8; 10] = [0, 7, 12, 16, 19, 24, 28, 31, 34, 36];
                let root = ROOTS[chord % ROOTS.len()];
                chord += 1;
                for d in SHAPE.iter().take(voices) {
                    e.event(Event::Note {
                        key: root + d,
                        on: true,
                    });
                    held.push(root + d);
                }
            }
            e.render(&[], &mut l, &mut r);
            e.end_block(block);
            // (The first chord's blocks warm the voices and the caches up.)
            if change && i >= every {
                changes.push(t.elapsed());
            }
        }
        std::hint::black_box((&l, &r));
        changes.sort();
        changes
            .get(changes.len() / 2)
            .copied()
            .unwrap_or(Duration::MAX)
    };
    let room = period.mul_f64(HEADROOM);
    let most = POLY_VOICES.2;
    let t = trial(most);
    if t <= room {
        return most as u32;
    }
    let mut voices =
        ((most as f64 * room.as_secs_f64() / t.as_secs_f64()).floor() as usize).min(most - 1);
    while voices > 0 {
        if trial(voices) <= room {
            break;
        }
        voices -= 1;
    }
    voices as u32
}

/// Voice `k`'s noise seed from the instance's: the first voice's the instance's own.
fn voice_seed(seed: u64, k: usize) -> u64 {
    if k == 0 {
        seed
    } else {
        seed ^ ((k as u64 + 1) << 48)
    }
}

/// The panel with voice `c`'s tolerances, by `amount` (0..1) of ENTROPY.
fn entropy_panel(c: &Character, mut p: Panel, amount: f64) -> Panel {
    let k = |i: usize, x: f64| c.knob(i, x, amount);
    p.emphasis = k(0, p.emphasis);
    p.filter_contour.attack = k(1, p.filter_contour.attack);
    p.filter_contour.decay = k(2, p.filter_contour.decay);
    p.loudness_contour.attack = k(3, p.loudness_contour.attack);
    p.loudness_contour.decay = k(4, p.loudness_contour.decay);
    p.glide = k(5, p.glide);
    p
}

/// MAIN OUTPUT VOLUME: R20 (5K, audio taper) from the output into the jack's 10K load, and
/// its switch; `volume` 0..1 of the knob's travel.
pub fn output_gain(volume: f64, on: bool) -> f64 {
    if !on {
        return 0.0;
    }
    let t = audio_taper(volume);
    let (top, bottom) = (5e3 * (1.0 - t), 5e3 * t);
    let load = bottom * 10e3 / (bottom + 10e3);
    if load <= 0.0 {
        0.0
    } else {
        load / (top + load)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::Budget;

    fn four_voices(workers: usize) -> Engine {
        let mut e = Engine::new();
        e.set(&Controls {
            poly: true,
            voices: 4,
            ..Controls::default()
        });
        e.prepare(48_000.0, 1);
        assert_eq!(e.start_workers(workers, None), workers);
        for key in [45, 52, 57, 64] {
            e.event(Event::Note { key, on: true });
        }
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        e.render(&[], &mut l, &mut r);
        e.end_block(256);
        e
    }

    /// A voice that a worker late from an earlier run still holds sits the next runs out:
    /// the block is not held up for it (it is still held when the block is done), the other
    /// voices play, and it plays again once let go. (Bounded by what happens, not by time: on
    /// a loaded machine any thread of the test may be held up for milliseconds.)
    #[test]
    fn a_voice_still_held_sits_out_and_nothing_waits_for_it() {
        let mut e = four_voices(2);
        let held = e.slots[1].play.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let (done, until) = std::sync::mpsc::channel::<()>();
        let late = std::thread::spawn(move || {
            let g = held.lock();
            tx.send(()).unwrap();
            let _ = until.recv_timeout(Duration::from_secs(30));
            drop(g);
        });
        rx.recv().unwrap();
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        // (No deadline: the voices the workers take are waited for, however late they are.)
        e.set_deadline(None);
        e.render(&[], &mut l, &mut r);
        assert!(!late.is_finished(), "the block waited for the held voice");
        assert!(l.iter().any(|x| x.abs() > 0.0), "the other voices silent");
        done.send(()).unwrap();
        late.join().unwrap();
        e.render(&[], &mut l, &mut r);
        assert!(e.slots[1].play.try_lock().is_ok());
    }

    /// A worker that has taken a voice and does not finish it by the deadline: the audio
    /// thread waits no longer than the deadline (it returns while the voice is still held, far
    /// past it), and that voice is silent for the run. The deadline is generous so that a
    /// loaded machine holding a worker up does not silence the others too.
    #[test]
    fn a_worker_late_past_the_deadline_is_not_waited_for() {
        let e = four_voices(1);
        let voices: Vec<Shared> = e.slots.iter().map(|s| s.play.clone()).collect();
        let mut pool = Pool::start(1, &voices, None).unwrap();
        let held = voices[0].clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let (done, until) = std::sync::mpsc::channel::<()>();
        let stall = std::thread::spawn(move || {
            let g = held.lock();
            tx.send(()).unwrap();
            let _ = until.recv_timeout(Duration::from_secs(30));
            drop(g);
        });
        rx.recv().unwrap();
        let mix = e.mix();
        let played = pool.play(
            &[0, 1, 2, 3],
            64,
            &[0.0; CHUNK],
            48_000.0,
            &mix,
            Some(Instant::now() + Duration::from_secs(1)),
        );
        assert!(!stall.is_finished(), "waited for the held voice");
        assert!(!played[0], "the held voice played");
        assert!(played[1..4].iter().all(|&p| p), "{played:?}");
        done.send(()).unwrap();
        stall.join().unwrap();
    }

    /// A voice let go that sits a run out (a worker late from an earlier run holds it) is not
    /// freed for it at the block's end: its tail is not known to have ended (R18).
    #[test]
    fn a_voice_that_misses_a_run_is_not_freed_for_it() {
        let mut e = four_voices(0);
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        for _ in 0..40 {
            e.render(&[], &mut l, &mut r);
            e.end_block(256);
        }
        for key in [45, 52, 57, 64] {
            e.event(Event::Note { key, on: false });
        }
        let held = e.slots[1].play.clone();
        let g = held.lock().unwrap();
        e.render(&[], &mut l, &mut r);
        e.end_block(256);
        drop(g);
        assert!(e.slots.iter().take(4).all(|s| s.active && !s.gate));
    }

    /// A retrigger's drop handed to a voice for a run it did not play is kept for its next run,
    /// not overwritten there, and played once; the gate it shows, not known to have played the
    /// run, keeps a new note on it retriggering (R18).
    #[test]
    fn a_drop_for_a_run_not_played_is_kept_for_the_next() {
        let mut e = four_voices(0);
        let play = e.slots[0].play.clone();
        let mut p = play.lock().unwrap();
        let s = &mut e.slots[0];
        (s.drop, s.shown) = (true, true);
        s.hand(&mut p);
        s.ran(false);
        assert!(!s.drop && p.drop);
        s.hand(&mut p);
        assert!(p.drop, "the drop lost");
        p.play(1, &[0.0; CHUNK], 48_000.0, &Mix::from_array([0.0; 4]));
        assert!(!p.drop);
        s.gate = false;
        s.ran(false);
        assert!(s.shown, "a gate the voice may not have seen");
        s.ran(true);
        assert!(!s.shown);
    }

    /// A voice whose output goes non-finite (its preamplifier's states poisoned, as one such
    /// sample at EXTERNAL INPUT left them) is put back to rest: the output stays finite and
    /// the held key sounds again; and again once the spares are mended (R18). POLY off and on.
    #[test]
    fn a_voice_gone_non_finite_is_put_back_to_rest() {
        for poly in [false, true] {
            let mut c = Controls {
                poly,
                ..Controls::default()
            };
            c.panel.ext_on = true;
            c.panel.ext_volume = 1.0;
            let mut e = Engine::new();
            e.set(&c);
            e.prepare(48_000.0, 1);
            e.event(Event::Note { key: 57, on: true });
            let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
            for round in 0..3 {
                if let Ok(mut p) = e.slots[0].play.lock() {
                    p.voice.tick_jacks(&ca72::voice::Jacks {
                        ext: f64::NAN,
                        ..ca72::voice::Jacks::default()
                    });
                }
                let mut last = 0.0f32;
                for _ in 0..40 {
                    e.render(&[], &mut l, &mut r);
                    e.end_block(256);
                    assert!(l.iter().chain(&r).all(|x| x.is_finite()), "poly {poly}");
                    last = l.iter().fold(0.0, |m, x| m.max(x.abs()));
                }
                assert!(e.take_mending(), "poly {poly}, round {round}: not restored");
                assert!(last > 1e-2, "poly {poly}, round {round}: silent ({last})");
                e.spares().mend();
            }
        }
    }

    /// Workers running for the voices, as many and for the same period, are kept when the
    /// engine is prepared again at the same rate (a host's state loaded into a running
    /// instance), and started once for voices built at a new rate (R18); a state with POLY off
    /// stops them (R21).
    #[test]
    fn workers_running_for_the_voices_are_kept() {
        let mut e = Engine::new();
        let mut c = Controls {
            poly: true,
            ..Controls::default()
        };
        e.set(&c);
        e.prepare(48_000.0, 1);
        assert_eq!(e.start_workers(2, None), 2);
        e.prepare(48_000.0, 2);
        assert_eq!(e.start_workers(2, None), 2);
        assert_eq!(e.pool_starts, 1, "started again at the same rate");
        e.prepare(44_100.0, 2);
        assert_eq!(e.workers(), 0, "kept with the old voices");
        assert_eq!(e.start_workers(2, None), 2);
        assert_eq!(e.start_workers(1, None), 1);
        assert_eq!(e.pool_starts, 3);
        c.poly = false;
        e.set(&c);
        e.prepare(44_100.0, 2);
        assert_eq!(e.start_workers(1, None), 0, "kept with POLY off");
    }

    /// A mend stops before its next voice once told to (the plug-in dropped: decisions.md
    /// R23): told after one, it makes one clean voice; the next mend makes the rest.
    #[test]
    fn a_mend_stops_before_its_next_voice() {
        let spares = Spares::default();
        spares.make(48_000.0);
        for place in &spares.places[..3] {
            place.lock().unwrap().clean = false;
        }
        let asked = std::cell::Cell::new(0);
        spares.mend_until(|| {
            asked.set(asked.get() + 1);
            asked.get() > 1
        });
        let clean = || {
            spares
                .places
                .iter()
                .filter(|p| p.lock().unwrap().clean)
                .count()
        };
        assert_eq!(clean(), POLY_VOICES.2 - 2);
        spares.mend();
        assert_eq!(clean(), POLY_VOICES.2);
    }

    /// An engine whose workers come out of `budget`, its voices built, POLY as `poly`, and
    /// workers asked of it as the plug-in asks them (for 5 ms blocks).
    fn planned(budget: &'static Budget, poly: bool, workers: usize) -> Engine {
        let mut e = Engine::new();
        e.crew = Arc::new(Crew::with_budget(budget));
        e.set(&Controls {
            poly,
            voices: 4,
            ..Controls::default()
        });
        e.prepare(48_000.0, 1);
        e.start_workers(workers, Some(Duration::from_millis(5)));
        e
    }

    /// POLY switched on or off, and the next block's start; returns whether serving is due
    /// (which the plug-in asks of its helper: decisions.md R23).
    fn switch(e: &mut Engine, poly: bool) -> bool {
        let c = Controls { poly, ..e.controls };
        e.set(&c);
        e.set_deadline(None);
        e.take_serving()
    }

    /// The helper thread's serving, run here (decisions.md R21, R23).
    fn serve(e: &Engine) {
        e.crew().serve();
    }

    /// An instance holds workers only while POLY is on (R21): activated with POLY off, none;
    /// switched on, none at once (its voices play on the caller's thread) and its workers once
    /// the helper thread has started them; switched off, let go at the next block and
    /// back in the budget once the helper thread has stopped them.
    #[test]
    fn an_instance_holds_workers_only_while_poly_is_on() {
        static THREE: Budget = Budget::new(|| 3);
        let mut e = planned(&THREE, false, 2);
        assert_eq!((e.workers(), THREE.held()), (0, 0));
        assert!(!switch(&mut e, false));
        assert!(switch(&mut e, true), "no workers asked for");
        assert_eq!(e.workers(), 0);
        for key in [45, 52, 57, 64] {
            e.event(Event::Note { key, on: true });
        }
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        e.render(&[], &mut l, &mut r);
        assert!(l.iter().any(|x| x.abs() > 0.0), "silent while waiting");
        serve(&e);
        assert_eq!(THREE.held(), 2);
        e.set_deadline(None);
        assert_eq!(e.workers(), 2);
        e.render(&[], &mut l, &mut r);
        e.end_block(256);
        assert!(switch(&mut e, false), "not let go");
        assert_eq!(e.workers(), 0);
        assert_eq!(THREE.held(), 2, "stopped on the audio thread");
        serve(&e);
        assert_eq!(THREE.held(), 0);
    }

    /// The bug R21 fixes: an instance with POLY off, activated first, held its share of the
    /// budget, and one with POLY on, activated later, got none. Now the second takes them all;
    /// the first, switched on, finds none left and plays its voices itself, and asks again
    /// when POLY is next switched on, by which time the second has given them back.
    #[test]
    fn an_instance_with_poly_off_leaves_its_workers_to_one_with_it_on() {
        static FOUR: Budget = Budget::new(|| 4);
        let mut off = planned(&FOUR, false, 4);
        let mut on = planned(&FOUR, true, 4);
        assert_eq!((off.workers(), on.workers()), (0, 4));
        assert!(switch(&mut off, true));
        serve(&off);
        off.set_deadline(None);
        assert_eq!(off.workers(), 0);
        assert!(!switch(&mut off, true), "asked again while POLY stays on");
        assert!(switch(&mut on, false));
        serve(&on);
        assert_eq!(FOUR.held(), 0);
        switch(&mut off, false);
        assert!(switch(&mut off, true));
        serve(&off);
        off.set_deadline(None);
        assert_eq!(off.workers(), 4);
        drop((off, on));
        assert_eq!(FOUR.held(), 0);
    }

    /// Voices built at a new rate while a pool started for the old ones waits to be taken, or
    /// the plug-in deactivated then: that pool is stopped, and the new voices' own pool gets
    /// every worker; nothing is kept from the budget.
    #[test]
    fn a_pool_for_voices_gone_is_stopped() {
        static TWO: Budget = Budget::new(|| 2);
        let mut e = planned(&TWO, false, 2);
        assert!(switch(&mut e, true));
        serve(&e);
        assert_eq!(TWO.held(), 2);
        e.prepare(44_100.0, 1);
        assert_eq!(TWO.held(), 0);
        assert_eq!(e.start_workers(2, Some(Duration::from_millis(5))), 2);
        e.set_deadline(None);
        assert_eq!(e.workers(), 2);
        assert!(switch(&mut e, false));
        assert!(switch(&mut e, true));
        serve(&e);
        e.stop_workers();
        assert_eq!(TWO.held(), 0);
        e.set_deadline(None);
        assert_eq!(e.workers(), 0);
        assert!(!e.take_serving());
    }

    /// A new seed at the rate the voices were built for draws their characters again, as
    /// voices built with it have them (R18).
    #[test]
    fn a_new_seed_draws_the_characters_again() {
        let mut a = Engine::new();
        a.prepare(48_000.0, 1);
        a.prepare(48_000.0, 2);
        let mut b = Engine::new();
        b.prepare(48_000.0, 2);
        for (k, (x, y)) in a.slots.iter().zip(&b.slots).enumerate() {
            let (x, y) = (x.play.lock().unwrap(), y.play.lock().unwrap());
            assert_eq!(
                format!("{:?}", x.character),
                format!("{:?}", y.character),
                "voice {k}"
            );
        }
    }
}
