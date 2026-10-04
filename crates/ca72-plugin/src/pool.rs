//! POLY's voices on worker threads of the plug-in's own (decisions.md R11): the host's audio
//! thread and a few workers (up to four) share each run of samples' voices, every voice played whole by
//! one of them, and the host's thread sums them in the voices' order, so the output is the
//! same to the bit as with every voice on the host's thread.
//!
//! Safe Rust only, as `ca72::threaded`: each voice sits behind a mutex that the audio thread
//! only ever tries (a voice is free whenever no one has taken it for the run at hand); a run
//! is offered through atomics. One word holds the run's number, its voices and the next one
//! to take, so whoever takes a voice takes it by a compare-and-swap on the word, and a
//! worker late from an earlier run can take nothing from a later one. The audio thread takes
//! voices too, until none is left: any voice no worker has started, it plays itself. It then
//! waits for the workers' voices until the block's deadline, spinning, then yielding, then
//! sleeping a little at a time ([`Backoff`]; on Windows it goes on yielding); a voice not
//! done by then is silent for that run and sits out until its worker lets it go. Once the
//! deadline has passed, the block's further runs are not offered: the audio thread plays them.
//! Nothing here allocates once the workers are started.
//!
//! A run is over when its player returns, and the word says so: a worker held up between
//! taking a voice and locking it, which finds its run over (the engine may have listed the
//! voice again meanwhile, its keys and panel brought up), leaves the voice as it is. The mark
//! of the run a voice last played for is written only by the worker holding the voice, so a
//! late worker cannot overwrite a later run's (decisions.md R18).
//!
//! The workers are started off the audio thread and run at the audio threads' priority
//! (`ca72_rt::promote`: on macOS the time-constraint policy for the host's block period). On
//! macOS they also join a work interval of their own, which the first of them opens at a
//! block's first run and closes when the block's runs are done: without it the system ran
//! them, busy a third of each period, about half as fast as a thread busy for most of it
//! (decisions.md R11). A plug-in cannot join its host's audio workgroup:
//! neither VST3 nor nih-plug's CLAP hands one over. They spin a moment after a run (the next
//! run of the block follows at once) and park after it; the audio thread wakes them for each
//! run. A run carries the audio thread's floating-point flush bits (nih-plug's flush-to-zero),
//! which a worker takes before it plays.
//!
//! Workers made audio threads come out of one budget for the process: all instances' together
//! are at most [`crate::engine::default_workers`]. An instance given fewer plays the rest on
//! the host's thread; a pool's go back when it stops (decisions.md R18). An instance holds a
//! pool only while POLY is on: its audio thread asks for one and lets it go through a
//! [`Crew`], which the plug-in's helper thread serves, starting and stopping pools
//! (decisions.md R21, R23).

use crate::engine::{CHUNK, Mix, Playing};
use ca72::threaded::Backoff;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant};

/// The most voices a run holds.
pub const MAX: usize = 10;

/// A voice's mark when it could not be played (its lock poisoned by a panic): done, but
/// silent.
const FAILED: u64 = 1 << 63;

/// The claim word's mark of a run over: its player has returned.
const CLOSED: u64 = 1 << 16;

/// How long an idle worker spins for the next run before it parks.
const SPIN: Duration = Duration::from_micros(200);

/// A voice as the engine and the workers share it.
pub type Shared<V = Playing> = Arc<Mutex<V>>;

/// What a voice does for a run: the engine's [`Playing`] (stand-ins in the tests).
pub trait Play: Send + 'static {
    /// `n` samples, `ext` at EXTERNAL INPUT, into the voice's own buffer.
    fn play(&mut self, n: usize, ext: &[f64; CHUNK], rate: f64, mix: &Mix);
}

impl Play for Playing {
    fn play(&mut self, n: usize, ext: &[f64; CHUNK], rate: f64, mix: &Mix) {
        Playing::play(self, n, ext, rate, mix);
    }
}

/// How many workers made audio threads the pools of the process may hold together, and how
/// many they hold.
pub(crate) struct Budget {
    most: fn() -> usize,
    held: AtomicUsize,
}

impl Budget {
    /// One of the tests' own, of `most` workers.
    #[cfg(test)]
    pub(crate) const fn new(most: fn() -> usize) -> Budget {
        Budget {
            most,
            held: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    pub(crate) fn held(&self) -> usize {
        self.held.load(Ordering::Acquire)
    }

    /// Up to `want` workers: as many as are left.
    fn claim(&self, want: usize) -> usize {
        let most = (self.most)();
        let mut held = self.held.load(Ordering::Acquire);
        loop {
            let k = want.min(most.saturating_sub(held));
            if k == 0 {
                return 0;
            }
            match self.held.compare_exchange_weak(
                held,
                held + k,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return k,
                Err(h) => held = h,
            }
        }
    }

    fn give_back(&self, k: usize) {
        self.held.fetch_sub(k, Ordering::AcqRel);
    }
}

/// The plug-in's, for all its instances in the process.
pub(crate) static BUDGET: Budget = Budget {
    most: crate::engine::default_workers,
    held: AtomicUsize::new(0),
};

/// What a run offers the workers.
struct Board<V> {
    /// The run's number (bits 32 up), whether it is over ([`CLOSED`]), its voices (bits 8
    /// to 15) and the next to take (bits 0 to 7).
    claim: AtomicU64,
    /// The voices' places among the engine's, in the order their outputs are summed.
    list: [AtomicUsize; MAX],
    /// Each voice's run number once a worker has played it for that run (written while the
    /// worker holds the voice).
    done: Box<[AtomicU64]>,
    /// The run's samples, the voices' rate and ENTROPY and SPREAD (f64 bits), EXTERNAL
    /// INPUT's samples, and the audio thread's flush bits ([`ca72_rt::flush_mode`]).
    n: AtomicUsize,
    rate: AtomicU64,
    mix: [AtomicU64; 4],
    ext: [AtomicU64; CHUNK],
    flush: AtomicU32,
    quit: AtomicBool,
    voices: Vec<Shared<V>>,
    /// The host's blocks so far, and the work interval the workers join, the first of them
    /// opening it at a block's first run and closing it when the block's runs are done
    /// (macOS; [`ca72_rt::WorkInterval`]).
    block: AtomicU64,
    interval: Option<ca72_rt::WorkInterval>,
}

fn word(run: u32, count: usize, next: usize) -> u64 {
    (u64::from(run) << 32) | ((count as u64) << 8) | next as u64
}

/// Whether run `run` is the one on the board, and not over.
fn current<V>(b: &Board<V>, run: u32) -> bool {
    let w = b.claim.load(Ordering::Acquire);
    (w >> 32) as u32 == run && w & CLOSED == 0
}

/// The next voice of run `run`, taken (its list place and its place among the voices), or
/// None once none is left (or the run is over, or another has begun).
fn take<V>(b: &Board<V>, run: u32) -> Option<(usize, usize)> {
    let mut w = b.claim.load(Ordering::Acquire);
    loop {
        let (r, count, next) = (
            (w >> 32) as u32,
            ((w >> 8) & 0xff) as usize,
            (w & 0xff) as usize,
        );
        if r != run || w & CLOSED != 0 || next >= count {
            return None;
        }
        // (The place is read before the swap: while the swap succeeds, the run has voices
        // left, so its list is not yet being written for the next.)
        let slot = b.list[next].load(Ordering::Relaxed);
        match b
            .claim
            .compare_exchange_weak(w, w + 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => return Some((next, slot)),
            Err(x) => w = x,
        }
    }
}

/// The workers and their board.
pub struct Pool<V: Play = Playing> {
    board: Arc<Board<V>>,
    threads: Vec<(Thread, Option<JoinHandle<()>>)>,
    run: u32,
    /// The budget its workers came out of, and how many.
    budget: Option<(&'static Budget, usize)>,
}

impl<V: Play> std::fmt::Debug for Pool<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("workers", &self.threads.len())
            .finish()
    }
}

impl<V: Play> Pool<V> {
    /// Up to `workers` threads for these voices, each made an audio thread for blocks of
    /// `period` (None: left ordinary, for measurements that run as fast as they go). Audio
    /// threads come out of the process's budget, so it may start fewer, or none
    /// ([`Pool::workers`]; decisions.md R18). Not on the audio thread.
    pub fn start(
        workers: usize,
        voices: &[Shared<V>],
        period: Option<Duration>,
    ) -> std::io::Result<Pool<V>> {
        // On macOS the standard library makes a mutex's lock on its first use: each voice's
        // taken once here, off the audio thread (as `ca72::threaded`'s workers; history.md,
        // "The worker locks").
        for v in voices {
            drop(v.lock());
        }
        Pool::start_from(&BUDGET, workers, voices, period)
    }

    /// As [`Pool::start`], from `budget`, the voices' locks not taken: a [`Crew`] starts a
    /// pool while the audio thread may be trying them (they were taken once when the voices
    /// were built).
    fn start_from(
        budget: &'static Budget,
        workers: usize,
        voices: &[Shared<V>],
        period: Option<Duration>,
    ) -> std::io::Result<Pool<V>> {
        let workers = match period {
            Some(_) => budget.claim(workers),
            None => workers,
        };
        let board = Arc::new(Board {
            claim: AtomicU64::new(0),
            list: std::array::from_fn(|_| AtomicUsize::new(0)),
            done: voices.iter().map(|_| AtomicU64::new(0)).collect(),
            n: AtomicUsize::new(0),
            rate: AtomicU64::new(0),
            mix: std::array::from_fn(|_| AtomicU64::new(0)),
            ext: std::array::from_fn(|_| AtomicU64::new(0)),
            flush: AtomicU32::new(0),
            quit: AtomicBool::new(false),
            voices: voices.to_vec(),
            block: AtomicU64::new(0),
            interval: period
                .filter(|_| workers > 0)
                .and_then(|_| ca72_rt::WorkInterval::new("CA-72 voices")),
        });
        if period.is_some() && workers > 0 {
            ca72_rt::watch();
        }
        // (The pool first: should a worker not start, those started stop and are joined and
        // the budget is given back as it drops.)
        let mut pool = Pool {
            board,
            threads: Vec::with_capacity(workers),
            run: 0,
            budget: period.map(|_| (budget, workers)),
        };
        for k in 0..workers {
            #[cfg(test)]
            if tests::REFUSE.get() == Some(k) {
                return Err(std::io::Error::other("refused for the test"));
            }
            let b = pool.board.clone();
            let handle = std::thread::Builder::new()
                .name(format!("ca72-voices-{k}"))
                .spawn(move || work(&b, period, k == 0))?;
            pool.threads.push((handle.thread().clone(), Some(handle)));
        }
        Ok(pool)
    }

    pub fn workers(&self) -> usize {
        self.threads.len()
    }

    /// A host's block begins.
    pub fn block(&self) {
        self.board.block.fetch_add(1, Ordering::Release);
    }

    /// Plays the voices at `list` (places among the voices, in the order they are summed)
    /// for `n` samples: offered to the workers and taken by this thread too, or, once
    /// `deadline` has passed, played here. Returns which list places played by `deadline`
    /// (None: every one waited for, as long as it takes).
    pub fn play(
        &mut self,
        list: &[usize],
        n: usize,
        ext: &[f64; CHUNK],
        rate: f64,
        mix: &Mix,
        deadline: Option<Instant>,
    ) -> [bool; MAX] {
        let count = list.len().min(MAX);
        let mut played = [false; MAX];
        // Past the deadline the run is not offered: the workers would take voices only for
        // this thread to give them up at once, silent for the run. Late, rather than silent
        // (decisions.md R18).
        if self.threads.is_empty() || deadline.is_some_and(|d| Instant::now() >= d) {
            for (ok, &slot) in played.iter_mut().zip(&list[..count]) {
                if let Ok(mut p) = self.board.voices[slot].try_lock() {
                    p.play(n, ext, rate, mix);
                    *ok = true;
                }
            }
            return played;
        }
        let run = self.offer(&list[..count], n, ext, rate, mix);
        if count > 1 {
            self.wake();
        }
        let b = &*self.board;
        // This thread's share: whatever no worker has taken.
        let mut here = [false; MAX];
        while let Some((i, slot)) = take(b, run) {
            here[i] = true;
            if let Ok(mut p) = b.voices[slot].try_lock() {
                p.play(n, ext, rate, mix);
                played[i] = true;
            }
        }
        for (i, &slot) in list.iter().enumerate().take(count) {
            if here[i] {
                continue;
            }
            let mut wait = Backoff::new();
            loop {
                let d = b.done[slot].load(Ordering::Acquire);
                if d & !FAILED == u64::from(run) {
                    played[i] = d & FAILED == 0;
                    break;
                }
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    break;
                }
                wait.snooze();
            }
        }
        self.close();
        played
    }

    /// Puts a run of the voices at `list` on the board (the workers not yet woken); returns
    /// its number.
    fn offer(&mut self, list: &[usize], n: usize, ext: &[f64; CHUNK], rate: f64, mix: &Mix) -> u32 {
        let b = &*self.board;
        // (Run 0 is no run: the workers start having seen it.)
        self.run = self.run.wrapping_add(1).max(1);
        for (place, &slot) in b.list.iter().zip(list) {
            place.store(slot, Ordering::Relaxed);
        }
        b.n.store(n, Ordering::Relaxed);
        b.rate.store(rate.to_bits(), Ordering::Relaxed);
        for (m, v) in b.mix.iter().zip(mix.to_array()) {
            m.store(v.to_bits(), Ordering::Relaxed);
        }
        for (e, v) in b.ext.iter().zip(&ext[..n]) {
            e.store(v.to_bits(), Ordering::Relaxed);
        }
        b.flush.store(ca72_rt::flush_mode(), Ordering::Relaxed);
        b.claim
            .store(word(self.run, list.len(), 0), Ordering::Release);
        self.run
    }

    fn wake(&self) {
        for (t, _) in &self.threads {
            t.unpark();
        }
    }

    /// The run is over, before the engine next tries any of its voices' locks: a worker
    /// that locks one after the engine has (and so after this) finds it so.
    fn close(&self) {
        self.board.claim.fetch_or(CLOSED, Ordering::AcqRel);
    }
}

impl<V: Play> Drop for Pool<V> {
    fn drop(&mut self) {
        self.board.quit.store(true, Ordering::Release);
        for (t, h) in &mut self.threads {
            t.unpark();
            if let Some(h) = h.take() {
                let _ = h.join();
            }
        }
        if let Some((budget, k)) = self.budget {
            budget.give_back(k);
        }
    }
}

/// An instance's workers, held only while POLY is on (decisions.md R21): its audio thread asks
/// for a pool and lets it go ([`Crew::follow`]), never waiting and never starting or stopping a
/// thread; the plug-in's helper thread stops the pools let go and starts the one asked for
/// ([`Crew::serve`]), for the voices last planned ([`Crew::plan`]). A pool started for a plan
/// no longer asked for (POLY off again meanwhile, the voices built anew, the plug-in
/// deactivated) is stopped where it was started, so no worker is kept from the budget.
pub struct Crew<V: Play = Playing> {
    budget: &'static Budget,
    /// Held while the helper thread serves, so that a new plan waits for a pool being
    /// started for an earlier one, and stops it before a pool of its own claims workers.
    serving: Mutex<()>,
    plan: Mutex<Plan<V>>,
    hand: Mutex<Hand<V>>,
}

/// What a pool is started for: the voices, how many workers and the period they are made
/// audio threads for; a new generation each time it is set.
struct Plan<V> {
    generation: u64,
    voices: Vec<Shared<V>>,
    workers: usize,
    period: Option<Duration>,
}

/// What passes between the audio thread and the helper thread.
struct Hand<V: Play> {
    /// The plan's generation the audio thread asks a pool for (0: none).
    want: u64,
    /// The answer for a generation: its pool, or none (the system would not start one).
    ready: Option<(u64, Option<Pool<V>>)>,
    /// A pool the audio thread let go, to be stopped.
    back: Option<Pool<V>>,
}

/// Where the audio thread's ask for workers stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Ask {
    #[default]
    Not,
    /// Asked; until the answer comes, the voices play on the audio thread.
    Waiting,
    /// Answered: a pool, or none, until POLY is next switched on.
    Answered,
}

impl<V: Play> std::fmt::Debug for Crew<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Crew").finish_non_exhaustive()
    }
}

impl<V: Play> Default for Crew<V> {
    fn default() -> Self {
        Crew::new()
    }
}

/// A lock the audio thread never waits for (it only tries it), taken off it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<V: Play> Crew<V> {
    /// With nothing planned, its workers from the process's budget.
    pub fn new() -> Crew<V> {
        Crew::with_budget(&BUDGET)
    }

    pub(crate) fn with_budget(budget: &'static Budget) -> Crew<V> {
        let crew = Crew {
            budget,
            serving: Mutex::new(()),
            plan: Mutex::new(Plan {
                generation: 0,
                voices: Vec::new(),
                workers: 0,
                period: None,
            }),
            hand: Mutex::new(Hand {
                want: 0,
                ready: None,
                back: None,
            }),
        };
        // (Its lock taken once here, off the audio thread: on macOS the standard library may
        // make a mutex's lock on its first use.)
        drop(crew.hand.try_lock());
        crew
    }

    /// Pools from now on for `workers` workers sharing `voices`, made audio threads for blocks
    /// of `period` ([`Pool::start`]); returns the plan's generation. Any pool started or let go
    /// for an earlier plan is stopped, here (one being started is waited for), and its ask
    /// withdrawn. Not on the audio thread.
    pub fn plan(&self, voices: &[Shared<V>], workers: usize, period: Option<Duration>) -> u64 {
        let _serving = lock(&self.serving);
        let (generation, old) = {
            let mut p = lock(&self.plan);
            p.generation += 1;
            p.workers = workers;
            p.period = period;
            (
                p.generation,
                std::mem::replace(&mut p.voices, voices.to_vec()),
            )
        };
        drop(old);
        let stop = {
            let mut h = lock(&self.hand);
            h.want = 0;
            (h.ready.take(), h.back.take())
        };
        drop(stop);
        generation
    }

    /// Nothing planned: as [`Crew::plan`], for no voices.
    pub fn clear(&self) {
        self.plan(&[], 0, None);
    }

    /// A pool for plan `generation` while it is the plan, or none (another planned since, or
    /// the system would not start one). Not on the audio thread.
    pub fn start(&self, generation: u64) -> Option<Pool<V>> {
        let (voices, workers, period) = {
            let p = lock(&self.plan);
            if p.generation != generation || p.workers == 0 || p.voices.is_empty() {
                return None;
            }
            (p.voices.clone(), p.workers, p.period)
        };
        Pool::start_from(self.budget, workers, &voices, period).ok()
    }

    /// The audio thread's asks served: the pools it let go stopped (their workers back in the
    /// budget), then a pool started for the plan it asks for, unless one is ready. Not on the
    /// audio thread: the plug-in's helper thread (decisions.md R21, R23).
    pub fn serve(&self) {
        let _serving = lock(&self.serving);
        if let Some(generation) = self.tidy() {
            self.deliver(generation, self.start(generation));
        }
    }

    /// The pools let go, or ready for an ask withdrawn, stopped; the generation asked for, if a
    /// pool is to be started for it.
    fn tidy(&self) -> Option<u64> {
        let (stop, start) = {
            let mut h = lock(&self.hand);
            let want = h.want;
            let stale = h.ready.take_if(|(g, _)| *g != want);
            let start = (want != 0 && h.ready.is_none()).then_some(want);
            ((h.back.take(), stale), start)
        };
        // (Stopped once the lock is let go: the audio thread may be trying it.)
        drop(stop);
        start
    }

    /// The answer for plan `generation` to the audio thread, while it still asks for it and
    /// has none; else its pool stopped here.
    fn deliver(&self, generation: u64, pool: Option<Pool<V>>) {
        let late = {
            let mut h = lock(&self.hand);
            if h.want == generation && h.ready.is_none() {
                h.ready = Some((generation, pool));
                None
            } else {
                pool
            }
        };
        drop(late);
    }

    /// At a block's start, on the audio thread, without waiting (decisions.md R21): its `pool`
    /// and `ask` brought towards what POLY asks (`on`) of plan `generation`. On, a pool
    /// started for the plan is taken once it is ready, or asked for, once (until it comes, the
    /// caller plays every voice itself); off, the pool is let go to be stopped off this thread
    /// and the ask withdrawn. A pool moves; none is started or stopped here. Should the
    /// helper thread hold the lock, this is tried again at the next block. Returns whether
    /// [`Crew::serve`] is due.
    pub(crate) fn follow(
        &self,
        on: bool,
        generation: u64,
        pool: &mut Option<Pool<V>>,
        ask: &mut Ask,
    ) -> bool {
        let settled = if on {
            pool.is_some() || *ask == Ask::Answered
        } else {
            pool.is_none() && *ask == Ask::Not
        };
        if settled || generation == 0 {
            return false;
        }
        let Ok(mut h) = self.hand.try_lock() else {
            return false;
        };
        if on {
            // (With no pool here: taking the answer drops nothing. One may be ready for an ask
            // withdrawn and made again since.)
            if let Some((_, answer)) = h.ready.take_if(|(g, _)| *g == generation) {
                *pool = answer;
                h.want = 0;
                *ask = Ask::Answered;
                return false;
            }
            if *ask == Ask::Not {
                h.want = generation;
                *ask = Ask::Waiting;
                return true;
            }
            return false;
        }
        // A pool started for an ask withdrawn is stopped by the helper thread: a pool
        // ready, at once, or one being started, as it comes.
        if h.want == generation {
            h.want = 0;
        }
        *ask = Ask::Not;
        let mut due = h.ready.is_some();
        if pool.is_some() && h.back.is_none() {
            h.back = pool.take();
            due = true;
        }
        due
    }
}

/// A worker: made an audio thread, it waits for a run (spinning a moment, then parked) and
/// plays the voices it takes.
fn work<V: Play>(b: &Board<V>, period: Option<Duration>, driver: bool) {
    // What a thread sets up the first time it parks or reads the clock is done before it
    // becomes an audio thread.
    std::thread::park_timeout(Duration::from_nanos(1));
    let _ = Instant::now().elapsed();
    if let Some(period) = period {
        let _ = ca72_rt::promote(period, period / 2);
    }
    let token = b.interval.as_ref().and_then(ca72_rt::WorkInterval::join);
    // (The first worker opens the interval at a block's first run and closes it when it
    // parks, the block's runs done.)
    let interval = if driver && token.is_some() {
        b.interval.as_ref().zip(period)
    } else {
        None
    };
    let (mut open, mut block) = (false, 0u64);
    let mut seen = 0u32;
    let mut ext = [0.0f64; CHUNK];
    let mut flush = ca72_rt::flush_mode();
    // A thread must leave its work interval before it ends (macOS stops the process
    // otherwise), the interval it opened closed first.
    struct Member<'a>(Option<(&'a ca72_rt::WorkInterval, ca72_rt::JoinToken)>);
    impl Drop for Member<'_> {
        fn drop(&mut self) {
            if let Some((i, t)) = self.0.take() {
                i.leave(t);
            }
        }
    }
    let _member = Member(b.interval.as_ref().zip(token));
    loop {
        let since = Instant::now();
        let mut spins = 0u32;
        let run = loop {
            if b.quit.load(Ordering::Acquire) {
                if let Some((i, _)) = interval
                    && open
                {
                    i.finish();
                }
                return;
            }
            let run = (b.claim.load(Ordering::Acquire) >> 32) as u32;
            if run != seen {
                break run;
            }
            spins = spins.wrapping_add(1);
            if !spins.is_multiple_of(64) || since.elapsed() < SPIN {
                std::hint::spin_loop();
            } else {
                if let Some((i, _)) = interval
                    && open
                {
                    i.finish();
                    open = false;
                }
                std::thread::park();
            }
        };
        seen = run;
        if let Some((i, period)) = interval {
            let now = b.block.load(Ordering::Acquire);
            if now != block {
                block = now;
                if open {
                    i.finish();
                }
                open = i.start(period);
            }
        }
        // The run's settings, read before any voice is taken (while one can be taken, they
        // are this run's), and the audio thread's flush bits taken: a voice's samples are
        // the same whichever thread plays it.
        let n = b.n.load(Ordering::Relaxed).min(CHUNK);
        let rate = f64::from_bits(b.rate.load(Ordering::Relaxed));
        let mix = Mix::from_array(std::array::from_fn(|k| {
            f64::from_bits(b.mix[k].load(Ordering::Relaxed))
        }));
        for (x, e) in ext.iter_mut().zip(&b.ext[..n]) {
            *x = f64::from_bits(e.load(Ordering::Relaxed));
        }
        let want = b.flush.load(Ordering::Relaxed);
        if want != flush {
            ca72_rt::set_flush_mode(want);
            flush = want;
        }
        while let Some((_, slot)) = take(b, run) {
            let (mut p, poisoned) = match b.voices[slot].lock() {
                Ok(p) => (p, false),
                Err(e) => (e.into_inner(), true),
            };
            // Held up between taking the voice and locking it, past the run's end: the
            // voice is left as it is, neither played nor marked (decisions.md R18).
            if !current(b, run) {
                break;
            }
            let mark = if poisoned {
                u64::from(run) | FAILED
            } else {
                p.play(n, &ext, rate, &mix);
                u64::from(run)
            };
            // The voice let go before it is marked: once marked, the caller's thread may close
            // the run and take the voice in the next, and a lock still held here would fail its
            // `try_lock` there, the voice silent for that run (seen in CI as samples that
            // differed from one thread's).
            drop(p);
            b.done[slot].store(mark, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::ThreadId;

    thread_local! {
        /// The worker [`Pool::start`] fails to start, as if the system had refused it.
        pub(super) static REFUSE: std::cell::Cell<Option<usize>> =
            const { std::cell::Cell::new(None) };
    }

    /// A voice that counts its runs and keeps where and how it played the last.
    #[derive(Default)]
    struct Fake {
        runs: usize,
        n: usize,
        thread: Option<ThreadId>,
        flush: u32,
        /// Set as its next run starts, the run then held until `hold` is set.
        started: Option<Arc<AtomicBool>>,
        hold: Option<Arc<AtomicBool>>,
        /// Set as it plays, and a pause after.
        release: Option<(Arc<AtomicBool>, Duration)>,
        /// A pause when played on this thread (so that the workers take the others).
        slow_on: Option<ThreadId>,
    }

    impl Play for Fake {
        fn play(&mut self, n: usize, _: &[f64; CHUNK], _: f64, _: &Mix) {
            self.runs += 1;
            self.n = n;
            self.thread = Some(std::thread::current().id());
            self.flush = ca72_rt::flush_mode();
            if let Some(s) = self.started.take() {
                s.store(true, Ordering::Release);
                if let Some(h) = self.hold.take() {
                    while !h.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_micros(200));
                    }
                }
            }
            if let Some((r, pause)) = &self.release {
                r.store(true, Ordering::Release);
                std::thread::sleep(*pause);
            }
            if self.slow_on.is_some() && self.slow_on == self.thread {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }

    fn fakes(k: usize) -> Vec<Shared<Fake>> {
        (0..k)
            .map(|_| Arc::new(Mutex::new(Fake::default())))
            .collect()
    }

    fn mix() -> Mix {
        Mix::from_array([0.0; 4])
    }

    const EXT: [f64; CHUNK] = [0.0; CHUNK];

    /// Waits up to 5 s for `f`.
    fn until(what: &str, f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(5), "{what}");
            std::thread::sleep(Duration::from_micros(100));
        }
    }

    /// A worker held up between taking a voice and locking it, which locks it only after
    /// the run is over (the voice listed again meanwhile), leaves it alone: it neither plays
    /// it with the old run's samples and settings nor marks it.
    #[test]
    fn a_worker_late_for_a_run_over_leaves_its_voice_alone() {
        let voices = fakes(2);
        let mut pool = Pool::start(1, &voices, None).unwrap();
        let held = voices[0].lock().unwrap();
        let run = pool.offer(&[0], 64, &EXT, 48_000.0, &mix());
        pool.wake();
        until("the worker took nothing", || {
            pool.board.claim.load(Ordering::Acquire) & 0xff == 1
        });
        // (The worker waits for the voice's lock; the run's player gives up on it.)
        std::thread::sleep(Duration::from_millis(10));
        pool.close();
        drop(held);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(voices[0].lock().unwrap().runs, 0, "played for a run over");
        assert_ne!(pool.board.done[0].load(Ordering::Acquire), u64::from(run));
        // The next run plays it as usual.
        let played = pool.play(&[0, 1], 32, &EXT, 48_000.0, &mix(), None);
        assert_eq!(&played[..2], &[true, true]);
        let v = voices[0].lock().unwrap();
        assert_eq!((v.runs, v.n), (1, 32));
    }

    /// A worker late from a run over that finishes its voice while a later run is played
    /// marks that voice, never the later run's voice at the same list place: the later run's
    /// voices are all heard, and nothing is waited for until the deadline.
    #[test]
    fn a_late_worker_does_not_unmark_a_later_runs_voice() {
        let voices = fakes(3);
        let (started, go) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        {
            let mut v = voices[0].lock().unwrap();
            v.started = Some(started.clone());
            v.hold = Some(go.clone());
        }
        voices[2].lock().unwrap().release = Some((go.clone(), Duration::from_millis(20)));
        let mut pool = Pool::start(1, &voices, None).unwrap();
        // Voice 0 alone, taken by the worker, which is held up playing it past the run's end.
        pool.offer(&[0], 64, &EXT, 48_000.0, &mix());
        pool.wake();
        until("the worker took nothing", || {
            started.load(Ordering::Acquire)
        });
        pool.close();
        // Voices 1 and 2 here (the worker is busy): voice 2 lets voice 0 go, and its worker
        // finishes it while this thread plays voice 2.
        let t = Instant::now();
        let deadline = Some(t + Duration::from_millis(500));
        let played = pool.play(&[1, 2], 32, &EXT, 48_000.0, &mix(), deadline);
        assert_eq!(&played[..2], &[true, true]);
        assert!(
            t.elapsed() < Duration::from_millis(400),
            "{:?}",
            t.elapsed()
        );
        // (Its own mark, the run it was taken for.)
        until("voice 0 not marked", || {
            pool.board.done[0].load(Ordering::Acquire) == 1
        });
    }

    /// Once the block's deadline has passed, a run is not offered to the workers: the
    /// caller's thread plays every voice, and every voice is heard.
    #[test]
    fn past_the_deadline_the_callers_thread_plays_every_voice() {
        let voices = fakes(4);
        let mut pool = Pool::start(2, &voices, None).unwrap();
        let past = Instant::now();
        std::thread::sleep(Duration::from_millis(1));
        let played = pool.play(&[0, 1, 2, 3], 64, &EXT, 48_000.0, &mix(), Some(past));
        assert_eq!(&played[..4], &[true; 4]);
        let me = std::thread::current().id();
        for v in &voices {
            assert_eq!(v.lock().unwrap().thread, Some(me));
        }
        assert_eq!(pool.board.claim.load(Ordering::Acquire), 0, "offered");
    }

    /// The workers play with the caller's flush bits, whatever they are.
    #[test]
    fn the_workers_take_the_callers_flush_bits() {
        let voices = fakes(3);
        let me = std::thread::current().id();
        for v in &voices {
            v.lock().unwrap().slow_on = Some(me);
        }
        let mut pool = Pool::start(2, &voices, None).unwrap();
        let was = ca72_rt::flush_mode();
        for mode in [ca72_rt::FLUSH, 0, ca72_rt::FLUSH] {
            ca72_rt::set_flush_mode(mode);
            let mut elsewhere = 0;
            for _ in 0..50 {
                let played = pool.play(&[0, 1, 2], 16, &EXT, 48_000.0, &mix(), None);
                assert_eq!(&played[..3], &[true; 3]);
                for v in &voices {
                    let v = v.lock().unwrap();
                    assert_eq!(v.flush, mode);
                    elsewhere += usize::from(v.thread != Some(me));
                }
                if elsewhere > 0 {
                    break;
                }
            }
            assert!(elsewhere > 0, "no voice played on a worker");
        }
        ca72_rt::set_flush_mode(was);
    }

    /// Audio-thread workers come out of one budget: pools together hold at most its size,
    /// one given none plays every voice on the caller's thread, and a pool's go back when it
    /// stops. Ordinary workers (for measurements) are not counted.
    #[test]
    fn workers_come_out_of_one_budget() {
        static THREE: Budget = Budget {
            most: || 3,
            held: AtomicUsize::new(0),
        };
        let voices = fakes(2);
        let period = Some(Duration::from_millis(5));
        let a = Pool::start_from(&THREE, 2, &voices, period).unwrap();
        let b = Pool::start_from(&THREE, 2, &voices, period).unwrap();
        let mut c = Pool::start_from(&THREE, 2, &voices, period).unwrap();
        assert_eq!((a.workers(), b.workers(), c.workers()), (2, 1, 0));
        assert_eq!(THREE.held.load(Ordering::Acquire), 3);
        let played = c.play(&[0, 1], 16, &EXT, 48_000.0, &mix(), None);
        assert_eq!(&played[..2], &[true, true]);
        let ordinary = Pool::start_from(&THREE, 4, &voices, None).unwrap();
        assert_eq!(ordinary.workers(), 4);
        drop(a);
        assert_eq!(THREE.held.load(Ordering::Acquire), 1);
        let d = Pool::start_from(&THREE, 4, &voices, period).unwrap();
        assert_eq!(d.workers(), 2);
        drop((b, c, d, ordinary));
        assert_eq!(THREE.held.load(Ordering::Acquire), 0);
    }

    /// A worker the system will not start: those started stop and are joined (nothing holds
    /// the voices any more), and the budget is given back.
    #[test]
    fn a_worker_not_started_stops_the_others() {
        static FOUR: Budget = Budget {
            most: || 4,
            held: AtomicUsize::new(0),
        };
        let voices = fakes(2);
        REFUSE.set(Some(2));
        let r = Pool::start_from(&FOUR, 3, &voices, Some(Duration::from_millis(5)));
        REFUSE.set(None);
        assert!(r.is_err());
        assert_eq!(FOUR.held.load(Ordering::Acquire), 0);
        assert_eq!(Arc::strong_count(&voices[0]), 1, "a worker runs on");
    }

    /// A crew holds workers only while POLY is on (decisions.md R21): none asked for while it
    /// is off; switched on, asked for once and taken once the helper thread has started
    /// them (the caller plays every voice itself until then); switched off, let go and stopped
    /// only when the helper thread serves, their workers then back in the budget. Each
    /// step here is the audio thread's or the helper thread's, run in turn.
    #[test]
    fn a_crew_holds_workers_only_while_poly_is_on() {
        static THREE: Budget = Budget::new(|| 3);
        let voices = fakes(3);
        let crew = Crew::with_budget(&THREE);
        let plan = crew.plan(&voices, 2, Some(Duration::from_millis(5)));
        let (mut pool, mut ask) = (None, Ask::Not);
        assert!(!crew.follow(false, plan, &mut pool, &mut ask));
        crew.serve();
        assert_eq!(THREE.held(), 0);
        assert!(
            crew.follow(true, plan, &mut pool, &mut ask),
            "not asked for"
        );
        assert!(
            !crew.follow(true, plan, &mut pool, &mut ask),
            "asked for twice"
        );
        assert!(pool.is_none());
        crew.serve();
        assert_eq!(THREE.held(), 2);
        assert!(!crew.follow(true, plan, &mut pool, &mut ask));
        assert_eq!(pool.as_ref().map(Pool::workers), Some(2));
        let played = pool
            .as_mut()
            .unwrap()
            .play(&[0, 1, 2], 16, &EXT, 48_000.0, &mix(), None);
        assert_eq!(&played[..3], &[true; 3]);
        assert!(crew.follow(false, plan, &mut pool, &mut ask), "not let go");
        assert!(pool.is_none());
        assert_eq!(THREE.held(), 2, "stopped on the audio thread");
        crew.serve();
        assert_eq!(THREE.held(), 0);
        // (The voices held now by the test and the plan alone: no worker runs on.)
        assert_eq!(Arc::strong_count(&voices[0]), 2);
    }

    /// POLY switched on and off again quickly: an ask withdrawn before it is served starts
    /// nothing; a pool ready for an ask withdrawn is stopped when next served, or taken if POLY
    /// is switched on again first; a pool still being started when the ask is withdrawn is
    /// stopped as it comes. No worker is kept from the budget.
    #[test]
    fn poly_switched_quickly_keeps_no_worker() {
        static FOUR: Budget = Budget::new(|| 4);
        let voices = fakes(3);
        let crew = Crew::with_budget(&FOUR);
        let plan = crew.plan(&voices, 3, Some(Duration::from_millis(5)));
        let (mut pool, mut ask) = (None, Ask::Not);
        let mut follow =
            |on: bool, pool: &mut Option<Pool<Fake>>| crew.follow(on, plan, pool, &mut ask);
        // On and off before the helper thread serves.
        assert!(follow(true, &mut pool));
        follow(false, &mut pool);
        crew.serve();
        assert_eq!(FOUR.held(), 0);
        // Served, then off before the pool is taken: stopped at the next serve.
        assert!(follow(true, &mut pool));
        crew.serve();
        assert_eq!(FOUR.held(), 3);
        assert!(follow(false, &mut pool), "the pool ready left running");
        assert!(pool.is_none());
        crew.serve();
        assert_eq!(FOUR.held(), 0);
        // Served, off, and on again before the next serve: the pool ready taken after all.
        assert!(follow(true, &mut pool));
        crew.serve();
        follow(false, &mut pool);
        follow(true, &mut pool);
        assert_eq!(pool.as_ref().map(Pool::workers), Some(3));
        crew.serve();
        assert_eq!(FOUR.held(), 3);
        assert!(follow(false, &mut pool));
        crew.serve();
        assert_eq!(FOUR.held(), 0);
        // Off while the helper thread is starting the pool: stopped as it comes.
        assert!(follow(true, &mut pool));
        let asked = crew.tidy().unwrap();
        let started = crew.start(asked);
        assert_eq!(FOUR.held(), 3);
        follow(false, &mut pool);
        crew.deliver(asked, started);
        assert_eq!(FOUR.held(), 0);
        follow(true, &mut pool);
        assert!(pool.is_none(), "a pool taken for an ask withdrawn");
    }

    /// A new plan (the voices built anew at a new rate, the plug-in deactivated) stops a pool
    /// ready or let go for the old one, and an ask for the old one starts nothing; a pool for
    /// the old plan that comes later is stopped. The audio thread asks again for the new.
    #[test]
    fn a_new_plan_stops_the_old_ones_pools() {
        static TWO: Budget = Budget::new(|| 2);
        let (voices, others) = (fakes(2), fakes(2));
        let period = Some(Duration::from_millis(5));
        let crew = Crew::with_budget(&TWO);
        let old = crew.plan(&voices, 2, period);
        let (mut pool, mut ask) = (None, Ask::Not);
        assert!(crew.follow(true, old, &mut pool, &mut ask));
        crew.serve();
        assert_eq!(TWO.held(), 2);
        let new = crew.plan(&others, 2, period);
        assert_eq!(TWO.held(), 0, "the old plan's pool kept");
        // (The engine forgets its ask with its plan.)
        (pool, ask) = (None, Ask::Not);
        assert!(crew.follow(true, new, &mut pool, &mut ask));
        let asked = crew.tidy().unwrap();
        crew.clear();
        assert!(crew.start(asked).is_none(), "started for a plan gone");
        crew.deliver(asked, None);
        // A pool for the old plan that comes after the new one: stopped.
        let new = crew.plan(&others, 2, period);
        (pool, ask) = (None, Ask::Not);
        assert!(crew.follow(true, new, &mut pool, &mut ask));
        let asked = crew.tidy().unwrap();
        let late = crew.start(asked);
        let newer = crew.plan(&voices, 2, period);
        crew.deliver(asked, late);
        assert_eq!(TWO.held(), 0);
        (pool, ask) = (None, Ask::Not);
        assert!(crew.follow(true, newer, &mut pool, &mut ask));
        crew.serve();
        crew.follow(true, newer, &mut pool, &mut ask);
        assert_eq!(pool.as_ref().map(Pool::workers), Some(2));
        drop(pool);
        assert_eq!(TWO.held(), 0);
    }
}
