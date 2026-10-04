//! The voice on six threads, for real time. The keyboard circuit, the contours (with
//! the noise source) and the external input's preamplifier depend only on the keys, the
//! panel and the input, not on the rest of the audio path, so each runs on a thread of its
//! own a few samples ahead of it; so does the VCA's bias, fed the loudness contour by the
//! contours' thread as it goes. The front of the audio path (modulation, oscillators,
//! mixer, the filter's control node) runs on the caller's thread and takes their outputs
//! sample by sample as they come, and the back (filter, the VCA's signal path, A-440) runs
//! on a thread of its own a few samples behind the front, taking the front's outputs and
//! the bias as they come. Every part does what [`Voice::tick_jacks`] has it do, in the same
//! order, so the output is the same to the bit; what changes is a block's worst case, which
//! becomes the slowest part's instead of the parts' sum.
//!
//! Safe Rust only: each worker's part and its inputs sit behind a mutex that the audio
//! thread only ever tries (a pass starts once the last has ended, so it is free) and the
//! worker holds for a pass; the outputs go through atomics, a sample's published by a
//! release store of the count ready, and so do the inputs of a worker fed as the pass goes
//! (the back's). Workers spin a little between passes (the next grid step follows at once)
//! and park after.
//!
//! No wait spins for good ([`Backoff`]): every part runs at the audio threads' priority
//! (`SCHED_FIFO` on Linux), and a thread spinning on a CPU where the thread it waits for is
//! queued keeps that thread off it for as long as no other CPU is free, as with more such
//! threads than CPUs (several voices at once). The two then wait on each other, and
//! everything below their priority on that CPU (the kernel's interrupt threads among it,
//! where real-time throttling is off) starves: on 2026-09-30 three voices at once stalled
//! the machine until it was reset.

use crate::noise::NoiseOut;
use crate::vca::VcaDrive;
use crate::voice::{
    BackPart, BiasPart, Contacts, ControlOut, ControlPart, FrontOut, FrontPart, Jacks,
    KeyboardPart, Keys, Panel, PreampPart, Quality, Voice,
};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::Thread;
use std::time::{Duration, Instant};

/// The most samples one pass hands the workers (a longer block goes in passes).
pub const CHUNK: usize = 256;

/// The control part's outputs a sample: two contours, three noise outputs and EXT.
/// LOUDNESS (passed on to the bias).
const CTL_OUTS: usize = 6;

/// The VCA's bias a sample.
const DRIVE: usize = VcaDrive::LEN;

/// The back's inputs a sample: the front's three outputs and the VCA's bias.
const BACK_INS: usize = 3 + DRIVE;

/// How long an idle worker waits for the next pass before it parks.
const SPIN: Duration = Duration::from_micros(300);

/// A wait for another thread that cannot keep it, or anything else, off the CPU for long:
/// it spins for [`Backoff::SPIN`] (the usual wait is microseconds), then yields to the
/// threads queued on its CPU (at its priority: the other parts), and from
/// [`Backoff::YIELD`] on sleeps between looks, giving the CPU to every thread, the system's
/// too. On Windows it yields instead of sleeping: a sleep there lasts a timer tick (1 to
/// 15.6 ms), far past a block's deadline, and a yield there gives the CPU to any thread
/// waiting for it, of any priority. Allocates nothing.
#[derive(Debug, Default)]
pub struct Backoff {
    looks: u32,
    since: Option<Instant>,
}

impl Backoff {
    /// Spinning only, this long.
    pub const SPIN: Duration = Duration::from_micros(20);
    /// Spinning and yielding, until this long.
    pub const YIELD: Duration = Duration::from_micros(200);
    /// Each sleep after that (not on Windows).
    pub const NAP: Duration = Duration::from_micros(20);

    pub fn new() -> Backoff {
        Backoff::default()
    }

    /// Waits a little: call it each time the awaited condition is found false.
    pub fn snooze(&mut self) {
        self.looks = self.looks.wrapping_add(1);
        // The clock is read only every 64 looks.
        if !self.looks.is_multiple_of(64) {
            std::hint::spin_loop();
            return;
        }
        let waited = self.since.get_or_insert_with(Instant::now).elapsed();
        if waited < Self::SPIN {
            std::hint::spin_loop();
        } else if waited < Self::YIELD || cfg!(windows) {
            std::thread::yield_now();
        } else {
            std::thread::sleep(Self::NAP);
        }
    }

    /// How long it has waited, roughly (from its 64th look).
    pub fn waited(&self) -> Duration {
        self.since.map_or(Duration::ZERO, |t| t.elapsed())
    }
}

/// A part a worker runs: a sample's contacts and `I` other inputs `x` in, its `O` outputs
/// out.
trait Stage<const I: usize, const O: usize>: Send + 'static {
    fn run(&mut self, p: &Panel, c: Contacts, x: &[f64; I], out: &mut [f64; O]);
}

impl Stage<0, 1> for KeyboardPart {
    fn run(&mut self, p: &Panel, c: Contacts, _: &[f64; 0], out: &mut [f64; 1]) {
        out[0] = self.tick(p, c);
    }
}

/// The contours and the noise: the EXT. LOUDNESS jack (NaN: empty) in; the two contours,
/// the three noise outputs and the jack out (the bias's inputs, which this thread feeds it).
impl Stage<1, CTL_OUTS> for ControlPart {
    fn run(&mut self, p: &Panel, c: Contacts, x: &[f64; 1], out: &mut [f64; CTL_OUTS]) {
        let o = self.tick(p, c);
        *out = [
            o.env_f,
            o.env_l,
            o.noise.white,
            o.noise.pink,
            o.noise.red,
            x[0],
        ];
    }
}

/// The VCA's bias: the control part's outputs in (the loudness contour, the EXT. LOUDNESS
/// jack); the bias out.
impl Stage<CTL_OUTS, DRIVE> for BiasPart {
    fn run(&mut self, p: &Panel, _: Contacts, x: &[f64; CTL_OUTS], out: &mut [f64; DRIVE]) {
        let loudness = (!x[5].is_nan()).then_some(x[5]);
        *out = self.tick(x[1], loudness, p.quality).to_array();
    }
}

/// The preamplifier: the EXTERNAL INPUT's sample in; its current into the bus and the
/// OVERLOAD lamp out.
impl Stage<1, 2> for PreampPart {
    fn run(&mut self, p: &Panel, _: Contacts, x: &[f64; 1], out: &mut [f64; 2]) {
        out[0] = self.tick(p, x[0]);
        out[1] = self.lamp().0;
    }
}

/// The back: the front's outputs and the VCA's bias in; the filter's output and the output
/// sample out.
impl Stage<BACK_INS, 2> for BackPart {
    fn run(&mut self, p: &Panel, _: Contacts, x: &[f64; BACK_INS], out: &mut [f64; 2]) {
        let f = FrontOut {
            i_bus: x[0],
            g_bus: x[1],
            i0: x[2],
        };
        let (y, o) = self.tick(p, &f, &drive(&x[3..]));
        out[0] = y;
        out[1] = o;
    }
}

/// The VCA's bias from its values ([`VcaDrive::to_array`]'s).
fn drive(x: &[f64]) -> VcaDrive {
    let mut d = [0.0; VcaDrive::LEN];
    d.copy_from_slice(x);
    VcaDrive::from_array(&d)
}

/// A pass's inputs, with the part that runs on them.
struct Job<P, const I: usize> {
    part: P,
    panel: Panel,
    contacts: [Contacts; CHUNK],
    x: [[f64; I]; CHUNK],
    len: usize,
    /// Whether the audio thread runs at real-time priority, for a worker that follows it.
    realtime: Option<bool>,
}

/// How the workers follow the audio thread's scheduling ([`Threaded::following`]): at real-
/// time priority while the thread that plays the voice has it (a host's audio threads),
/// ordinary otherwise (an offline render, a benchmark run as fast as it goes, or once the
/// system's watchdog has made the audio threads ordinary). Each worker changes its own
/// only when the audio thread's changes.
#[derive(Debug, Clone, Copy)]
pub struct Follow {
    /// Whether the calling thread runs at real-time priority.
    pub realtime: fn() -> bool,
    /// Gives the calling thread real-time priority (true) or ordinary priority.
    pub set: fn(bool),
}

struct Shared<P, const I: usize> {
    job: Mutex<Job<P, I>>,
    /// The pass asked for, and the last one finished.
    asked: AtomicU64,
    done: AtomicU64,
    /// The pass's samples ready.
    ready: AtomicUsize,
    /// Each sample's outputs (f64 bits), the stage's `O` a sample.
    out: Vec<AtomicU64>,
    quit: AtomicBool,
    /// False once the worker has stopped (a panic): the audio thread stops waiting.
    alive: Arc<AtomicBool>,
    /// A worker fed as the pass goes: its inputs a sample (f64 bits, `I` a sample) and how
    /// many it has been given, by the caller or by the worker that feeds it (`fed_by`, that
    /// worker's liveness: while it has stopped nothing more comes).
    streamed: bool,
    inp: Vec<AtomicU64>,
    in_ready: AtomicUsize,
    fed_by: Option<Arc<AtomicBool>>,
    /// The worker this one feeds its outputs, each sample as it is ready.
    feeds: Option<Arc<dyn Feed>>,
}

/// A worker's inputs, given sample by sample as the pass goes.
trait Feed: Send + Sync {
    /// Sample `i`'s inputs (every sample of the pass, in order).
    fn feed(&self, i: usize, x: &[f64]);
}

impl<P: Send, const I: usize> Feed for Shared<P, I> {
    fn feed(&self, i: usize, x: &[f64]) {
        for (slot, v) in self.inp[i * I..(i + 1) * I].iter().zip(x) {
            slot.store(v.to_bits(), Ordering::Relaxed);
        }
        self.in_ready.store(i + 1, Ordering::Release);
    }
}

/// How a worker is fed and whom it feeds ([`Worker::spawn`]).
struct Links {
    /// Fed as the pass goes (by the caller unless `fed_by`), not given its inputs at once.
    streamed: bool,
    /// The worker that feeds it: its liveness.
    fed_by: Option<Arc<AtomicBool>>,
    /// The worker it feeds its outputs to.
    feeds: Option<Arc<dyn Feed>>,
    /// Its own liveness (shared with the worker it feeds).
    alive: Arc<AtomicBool>,
}

impl Links {
    /// Given its inputs at once, feeding nothing.
    fn given() -> Links {
        Links {
            streamed: false,
            fed_by: None,
            feeds: None,
            alive: Arc::new(AtomicBool::new(true)),
        }
    }
}

struct Worker<P, const I: usize, const O: usize> {
    shared: Arc<Shared<P, I>>,
    thread: Thread,
}

impl<P, const I: usize, const O: usize> std::fmt::Debug for Worker<P, I, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("thread", &self.thread.name())
            .finish()
    }
}

impl<P: Stage<I, O>, const I: usize, const O: usize> Worker<P, I, O> {
    fn spawn(
        part: P,
        name: &str,
        links: Links,
        start: fn(),
        follow: Option<Follow>,
    ) -> std::io::Result<Worker<P, I, O>> {
        let shared = Arc::new(Shared {
            job: Mutex::new(Job {
                part,
                panel: Panel::default(),
                contacts: [Contacts::default(); CHUNK],
                x: [[0.0; I]; CHUNK],
                len: 0,
                realtime: None,
            }),
            asked: AtomicU64::new(0),
            done: AtomicU64::new(0),
            ready: AtomicUsize::new(0),
            out: (0..CHUNK * O).map(|_| AtomicU64::new(0)).collect(),
            quit: AtomicBool::new(false),
            alive: links.alive,
            streamed: links.streamed,
            inp: (0..CHUNK * I).map(|_| AtomicU64::new(0)).collect(),
            in_ready: AtomicUsize::new(0),
            fed_by: links.fed_by,
            feeds: links.feeds,
        });
        // On macOS the standard library makes a mutex's pthread lock on its first use (64
        // bytes): take it once here, off the audio thread, so that the first pass's
        // `try_lock` allocates nothing (history.md, "The worker locks"; Linux's futex allocates
        // nothing).
        drop(shared.job.lock());
        let s = shared.clone();
        let handle = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || work::<P, I, O>(&s, start, follow.map(|f| f.set)))?;
        Ok(Worker {
            shared,
            thread: handle.thread().clone(),
        })
    }

    /// Hands the worker a pass of `len` samples: their contacts and, unless it is fed as
    /// the pass goes ([`Worker::feed`]), their other inputs `x`; false if it cannot take one
    /// (it has stopped).
    fn begin(
        &self,
        pass: u64,
        (panel, realtime): (&Panel, Option<bool>),
        len: usize,
        contacts: &[Contacts],
        x: &[[f64; I]],
    ) -> bool {
        let s = &*self.shared;
        let Ok(mut job) = s.job.try_lock() else {
            return false;
        };
        job.panel = *panel;
        job.realtime = realtime;
        job.len = len;
        job.contacts[..len].copy_from_slice(&contacts[..len]);
        if !s.streamed {
            job.x[..len].copy_from_slice(&x[..len]);
        }
        drop(job);
        s.ready.store(0, Ordering::Relaxed);
        s.in_ready.store(0, Ordering::Relaxed);
        s.asked.store(pass, Ordering::Release);
        self.thread.unpark();
        true
    }

    /// Sample `i`'s inputs, for a worker the caller feeds as the pass goes (in order, every
    /// sample of the pass).
    fn feed(&self, i: usize, x: &[f64; I]) {
        self.shared.feed(i, x);
    }

    /// Sample `i`'s outputs, once ready; None if the worker has stopped.
    fn take(&self, i: usize) -> Option<[f64; O]> {
        let s = &*self.shared;
        let mut wait = Backoff::new();
        while s.ready.load(Ordering::Acquire) <= i {
            if !s.alive.load(Ordering::Acquire) {
                return None;
            }
            wait.snooze();
        }
        Some(std::array::from_fn(|k| {
            f64::from_bits(s.out[i * O + k].load(Ordering::Relaxed))
        }))
    }

    /// Waits for the pass to end (the worker has let go of its part); false if it stopped.
    fn finish(&self, pass: u64) -> bool {
        let s = &*self.shared;
        let mut wait = Backoff::new();
        while s.done.load(Ordering::Acquire) != pass {
            if !s.alive.load(Ordering::Acquire) {
                return false;
            }
            wait.snooze();
        }
        true
    }
}

impl<P, const I: usize, const O: usize> Drop for Worker<P, I, O> {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

/// A worker's loop: waits for a pass (spinning a moment, then parked), runs its part over
/// the pass's samples, publishing each, and marks the pass done.
fn work<P: Stage<I, O>, const I: usize, const O: usize>(
    s: &Shared<P, I>,
    start: fn(),
    follow: Option<fn(bool)>,
) {
    struct Alive<'a>(&'a AtomicBool);
    impl Drop for Alive<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    // (A worker's liveness also tells the worker it feeds that nothing more will come.)
    let _alive = Alive(&s.alive);
    // What a thread sets up the first time it parks or looks itself up (its handle, its
    // thread-locals' registration) is done here, before `start` makes it an audio thread.
    let _ = std::thread::current();
    std::thread::park_timeout(Duration::from_nanos(1));
    let _ = Instant::now().elapsed();
    start();
    let mut seen = 0;
    // This thread's scheduling as last set to follow the audio thread's.
    let mut mine = None;
    loop {
        let mut idle = Backoff::new();
        loop {
            if s.quit.load(Ordering::Acquire) {
                return;
            }
            let asked = s.asked.load(Ordering::Acquire);
            if asked != seen {
                seen = asked;
                break;
            }
            if idle.waited() < SPIN {
                idle.snooze();
            } else {
                std::thread::park();
            }
        }
        {
            // The part is let go before the pass is marked done (the audio thread's next
            // `begin` only tries the lock).
            let Ok(mut guard) = s.job.lock() else {
                return;
            };
            let job = &mut *guard;
            if let (Some(set), Some(want)) = (follow, job.realtime)
                && mine != Some(want)
            {
                set(want);
                mine = Some(want);
            }
            let mut out = [0.0; O];
            for i in 0..job.len {
                let x = if s.streamed {
                    // Fed as the pass goes: every sample of it comes, unless the worker that
                    // feeds this one has stopped.
                    let mut wait = Backoff::new();
                    while s.in_ready.load(Ordering::Acquire) <= i {
                        if s.quit.load(Ordering::Acquire)
                            || s.fed_by
                                .as_ref()
                                .is_some_and(|a| !a.load(Ordering::Acquire))
                        {
                            return;
                        }
                        wait.snooze();
                    }
                    std::array::from_fn(|k| {
                        f64::from_bits(s.inp[i * I + k].load(Ordering::Relaxed))
                    })
                } else {
                    job.x[i]
                };
                job.part.run(&job.panel, job.contacts[i], &x, &mut out);
                for (slot, v) in s.out[i * O..(i + 1) * O].iter().zip(out) {
                    slot.store(v.to_bits(), Ordering::Relaxed);
                }
                s.ready.store(i + 1, Ordering::Release);
                if let Some(f) = &s.feeds {
                    f.feed(i, &out);
                }
            }
        }
        s.done.store(seen, Ordering::Release);
    }
}

/// The voice with its keyboard circuit, contours, preamplifier and back on threads of their
/// own.
#[derive(Debug)]
pub struct Threaded {
    pub panel: Panel,
    keys: Keys,
    front: FrontPart,
    kbd: Worker<KeyboardPart, 0, 1>,
    ctl: Worker<ControlPart, 1, CTL_OUTS>,
    bias: Worker<BiasPart, CTL_OUTS, DRIVE>,
    pre: Worker<PreampPart, 1, 2>,
    back: Worker<BackPart, BACK_INS, 2>,
    pass: u64,
    /// The VCA's bias of the last sample every worker gave (the back's input when one has
    /// stopped and the output is silent).
    last_drive: [f64; DRIVE],
    /// The OVERLOAD lamp after the last sample and its brightest since it was last read.
    overload: f64,
    overload_peak: f64,
    /// Samples output silent because a worker had stopped (tests require none).
    pub failed: u64,
    /// How the workers follow the audio thread's scheduling, if they do.
    follow: Option<Follow>,
    /// FEEDBACK, as [`Voice::feedback`]: on, the parts run in turn on the caller's thread
    /// (a one-sample loop from the output to the preamplifier leaves nothing to pipeline).
    pub feedback: f64,
    /// The last output sample, for FEEDBACK.
    fed_back: f64,
}

impl Threaded {
    /// Splits the voice and starts its five workers, each of which calls `start` first (to
    /// take the audio threads' priority, say).
    pub fn new(voice: Voice, start: fn()) -> std::io::Result<Threaded> {
        Threaded::build(voice, start, None)
    }

    /// As [`Threaded::new`], the workers following the scheduling of the thread that plays
    /// the voice ([`Follow`]): each pass carries whether that thread is at real-time
    /// priority.
    pub fn following(voice: Voice, start: fn(), follow: Follow) -> std::io::Result<Threaded> {
        Threaded::build(voice, start, Some(follow))
    }

    fn build(voice: Voice, start: fn(), follow: Option<Follow>) -> std::io::Result<Threaded> {
        let (panel, mut keys, kbd, ctl, bias, audio) = voice.into_parts();
        keys.reserve();
        let last_drive = bias.drive().to_array();
        // The contours' thread feeds the bias's.
        let ctl_links = Links::given();
        let bias: Worker<BiasPart, CTL_OUTS, DRIVE> = Worker::spawn(
            bias,
            "ca72-vca-bias",
            Links {
                streamed: true,
                fed_by: Some(ctl_links.alive.clone()),
                ..Links::given()
            },
            start,
            follow,
        )?;
        let ctl_links = Links {
            feeds: Some(bias.shared.clone()),
            ..ctl_links
        };
        Ok(Threaded {
            panel,
            keys,
            front: audio.front,
            kbd: Worker::spawn(kbd, "ca72-keyboard", Links::given(), start, follow)?,
            ctl: Worker::spawn(ctl, "ca72-contours", ctl_links, start, follow)?,
            bias,
            pre: Worker::spawn(audio.pre, "ca72-preamp", Links::given(), start, follow)?,
            back: Worker::spawn(
                audio.back,
                "ca72-filter",
                Links {
                    streamed: true,
                    ..Links::given()
                },
                start,
                follow,
            )?,
            pass: 0,
            last_drive,
            overload: 0.0,
            overload_peak: 0.0,
            failed: 0,
            follow,
            feedback: 0.0,
            fed_back: 0.0,
        })
    }

    /// A MIDI note (0 to 127) pressed or released ([`Voice::note`]).
    pub fn note(&mut self, midi: i32, on: bool) {
        self.keys.note(midi, on);
    }

    /// The passes handed to the workers so far (none go to them in Potato).
    pub fn passes(&self) -> u64 {
        self.pass
    }

    /// The OVERLOAD lamp after the last sample ([`Voice::overload`]).
    pub fn overload(&self) -> f64 {
        self.overload
    }

    /// The OVERLOAD lamp's brightest since the last call ([`Voice::take_overload_peak`]).
    pub fn take_overload_peak(&mut self) -> f64 {
        std::mem::take(&mut self.overload_peak)
    }

    /// `len` samples: `jacks(i)` at the jacks, each output sample to `out(i, y)`. The same
    /// samples [`Voice::tick_jacks`] gives. In Potato the parts run in turn on the caller's
    /// thread ([`Threaded::process_in_turn`]).
    pub fn process(
        &mut self,
        len: usize,
        jacks: impl Fn(usize) -> Jacks,
        mut out: impl FnMut(usize, f64),
    ) {
        if self.panel.quality == Quality::Potato || self.feedback > 0.0 {
            self.process_in_turn(len, jacks, out);
            return;
        }
        let p = self.panel;
        // Whether this thread is at real-time priority, for the workers to follow.
        let rt = self.follow.map(|f| (f.realtime)());
        let mut contacts = [Contacts::default(); CHUNK];
        let mut js = [Jacks::default(); CHUNK];
        // The external input (the preamplifier's) and EXT. LOUDNESS (the control part's,
        // NaN: empty).
        let mut ext = [[0.0; 1]; CHUNK];
        let mut loudness = [[0.0; 1]; CHUNK];
        let mut at = 0;
        while at < len {
            let n = (len - at).min(CHUNK);
            for i in 0..n {
                js[i] = jacks(at + i);
                contacts[i] = self.keys.step();
                contacts[i].s_trig = js[i].s_trig;
                ext[i][0] = js[i].ext;
                loudness[i][0] = js[i].loudness.unwrap_or(f64::NAN);
            }
            self.pass += 1;
            let pass = self.pass;
            // (The bias before the contours, which feed it from the start of their pass.)
            let bias_ok = self.bias.begin(pass, (&p, rt), n, &contacts, &[]);
            let ok = [
                self.kbd.begin(pass, (&p, rt), n, &contacts, &[[]; CHUNK]),
                self.ctl.begin(pass, (&p, rt), n, &contacts, &loudness),
                self.pre.begin(pass, (&p, rt), n, &contacts, &ext),
                self.back.begin(pass, (&p, rt), n, &contacts, &[]),
            ];
            let all = bias_ok && ok.iter().all(|&b| b);
            // The front, sample by sample, feeding the back (every sample of the pass, even
            // when a worker has stopped: the back waits for them); a sample some worker did
            // not give is silent.
            let mut given = [false; CHUNK];
            for (i, jack) in js.iter().enumerate().take(n) {
                let taken = if all {
                    (
                        self.kbd.take(i),
                        self.ctl.take(i),
                        self.pre.take(i),
                        self.bias.take(i),
                    )
                } else {
                    (None, None, None, None)
                };
                let fed = match taken {
                    (Some(k), Some(c), Some(pr), Some(b)) => {
                        let ctl = ControlOut {
                            env_f: c[0],
                            env_l: c[1],
                            noise: NoiseOut {
                                white: c[2],
                                pink: c[3],
                                red: c[4],
                            },
                        };
                        self.overload = pr[1];
                        self.overload_peak = self.overload_peak.max(pr[1]);
                        let f = self.front.tick(&p, jack, k[0], &ctl, pr[0]);
                        self.last_drive = b;
                        given[i] = true;
                        let mut fed = [0.0; BACK_INS];
                        fed[..3].copy_from_slice(&[f.i_bus, f.g_bus, f.i0]);
                        fed[3..].copy_from_slice(&b);
                        fed
                    }
                    _ => {
                        let mut fed = [0.0; BACK_INS];
                        fed[..3].copy_from_slice(&[0.0, 1.0 / 33e3, 0.0]);
                        fed[3..].copy_from_slice(&self.last_drive);
                        fed
                    }
                };
                if ok[3] {
                    self.back.feed(i, &fed);
                }
            }
            for (i, &given) in given.iter().enumerate().take(n) {
                match (given, ok[3].then(|| self.back.take(i)).flatten()) {
                    (true, Some(o)) => {
                        self.fed_back = o[1];
                        out(at + i, o[1]);
                    }
                    _ => {
                        // A worker has stopped: the sample is silent.
                        self.failed += 1;
                        out(at + i, 0.0);
                    }
                }
            }
            let mut finished = true;
            if bias_ok {
                finished &= self.bias.finish(pass);
            }
            if ok[0] {
                finished &= self.kbd.finish(pass);
            }
            if ok[1] {
                finished &= self.ctl.finish(pass);
            }
            if ok[2] {
                finished &= self.pre.finish(pass);
            }
            if ok[3] {
                finished &= self.back.finish(pass);
            }
            if !finished {
                self.failed += 1;
            }
            at += n;
        }
    }

    /// [`Threaded::process`] with every part run in turn on the caller's thread, as
    /// [`Voice::tick_jacks`] runs them, the workers left parked (Potato: a Potato voice
    /// is light enough that the engine's pool spreads many of them over its threads better
    /// than each spreading itself over five; the parts stay where they are, so the mode
    /// switches either way while it plays). Each part is taken from its worker's lock, free
    /// between passes; a part that cannot be taken (its worker stopped holding it) leaves
    /// the block silent, counted in `failed`.
    fn process_in_turn(
        &mut self,
        len: usize,
        jacks: impl Fn(usize) -> Jacks,
        mut out: impl FnMut(usize, f64),
    ) {
        let p = self.panel;
        let parts = (
            self.kbd.shared.job.try_lock(),
            self.ctl.shared.job.try_lock(),
            self.bias.shared.job.try_lock(),
            self.pre.shared.job.try_lock(),
            self.back.shared.job.try_lock(),
        );
        let (Ok(mut kbd), Ok(mut ctl), Ok(mut bias), Ok(mut pre), Ok(mut back)) = parts else {
            self.failed += len as u64;
            for i in 0..len {
                out(i, 0.0);
            }
            return;
        };
        pre.part.accurate = self.feedback > 0.0;
        for i in 0..len {
            let mut j = jacks(i);
            if self.feedback > 0.0 {
                j.ext += self.feedback * self.fed_back;
            }
            let mut c = self.keys.step();
            c.s_trig = j.s_trig;
            let v_kbd = kbd.part.tick(&p, c);
            let control = ctl.part.tick(&p, c);
            let d = bias.part.tick(control.env_l, j.loudness, p.quality);
            let i_pre = pre.part.tick(&p, j.ext);
            let (lamp, _) = pre.part.lamp();
            self.overload = lamp;
            self.overload_peak = self.overload_peak.max(lamp);
            let f = self.front.tick(&p, &j, v_kbd, &control, i_pre);
            let (_, y) = back.part.tick(&p, &f, &d);
            self.last_drive = d.to_array();
            self.fed_back = y;
            out(i, y);
        }
    }
}
