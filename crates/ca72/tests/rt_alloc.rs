//! The voice allocates nothing once built (DESIGN 9.2): notes, a moving cutoff and a moving
//! noise volume (which rebuilds the noise model) through thousands of samples, then the
//! external input driven, GLIDE, the keyboard's control switches and every rear jack
//! plugged (which move the nodal solver's pivots, so it records its eliminations again),
//! counted by a global allocator. A failure names where the first allocation came from.

#![allow(unsafe_code, clippy::unwrap_used)]

use ca72::voice::{Jacks, Voice};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;
static ARMED: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicUsize = AtomicUsize::new(0);
static FIRST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// The counters are global: one test at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Whether only the threads marked `HERE` count (the one-thread test: meanwhile the test
/// harness's own thread may print the other test's result, which allocates; history.md,
/// "The worker locks").
static ONLY_HERE: AtomicBool = AtomicBool::new(false);
thread_local! {
    static HERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// SAFETY: forwards to the system allocator; only counts (and records a backtrace) while armed.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let counted = !ONLY_HERE.load(Ordering::SeqCst) || HERE.with(std::cell::Cell::get);
        if counted && ARMED.swap(false, Ordering::SeqCst) {
            COUNT.fetch_add(1, Ordering::SeqCst);
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            if let Ok(mut f) = FIRST.lock() {
                f.get_or_insert(bt);
            }
            ARMED.store(true, Ordering::SeqCst);
        }
        // SAFETY: the caller's contract is passed through.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn the_voice_does_not_allocate_while_it_plays() {
    let _serial = SERIAL.lock();
    COUNT.store(0, Ordering::SeqCst);
    // Built as the plug-in builds it: a copy of the rate's prototype.
    let mut v = Voice::prototype(48_000.0);
    // Counted from the copy's first sample, as a host plays it (its first step records
    // the solvers' eliminations for a step afresh: that must not allocate either), on this
    // thread only.
    HERE.with(|h| h.set(true));
    ONLY_HERE.store(true, Ordering::SeqCst);
    ARMED.store(true, Ordering::SeqCst);
    for i in 0..48_000usize {
        if i % 4800 == 0 {
            v.note(45 + (i / 4800) as i32 % 12, true);
        }
        if i % 4800 == 3000 {
            v.note(45 + (i / 4800) as i32 % 12, false);
        }
        v.panel.cutoff = 0.5 + 0.3 * ((i as f64) * 1e-4).sin();
        v.panel.noise_volume = 0.5 + 0.3 * ((i as f64) * 3e-4).sin();
        v.tick();
    }
    // Everything moving at once: the external input driven hard, GLIDE, the keyboard's
    // control switches, the rear jacks.
    v.panel.ext_on = true;
    v.panel.ext_volume = 1.0;
    v.panel.glide_on = true;
    v.panel.glide = 0.4;
    let tau = 2.0 * std::f64::consts::PI;
    for i in 0..48_000usize {
        let t = i as f64 / 48_000.0;
        if i % 2400 == 0 {
            v.note(41 + (i / 2400 * 7) as i32 % 44, true);
        }
        if i % 2400 == 1800 {
            v.note(41 + (i / 2400 * 7) as i32 % 44, false);
        }
        if i % 4000 == 0 {
            v.panel.keyboard_control_1 = !v.panel.keyboard_control_1;
            v.panel.glide_on = !v.panel.glide_on;
        }
        v.tick_jacks(&Jacks {
            ext: 0.9 * (tau * 220.0 * t).sin(),
            osc: Some(0.25 * (tau * 110.0 * t).sin()),
            filter: Some(3.0 * (tau * 0.7 * t).sin()),
            loudness: Some(4.0 + 4.0 * (tau * 5.0 * t).sin()),
            s_trig: t.rem_euclid(0.35) < 0.005,
            // The plug-in's ENTROPY moving too.
            detune: [3.0 * (tau * 0.3 * t).sin(), -2.0, 1.5],
            cutoff: 0.03 * (tau * 0.2 * t).sin(),
        });
    }
    ARMED.store(false, Ordering::SeqCst);
    ONLY_HERE.store(false, Ordering::SeqCst);
    let n = COUNT.load(Ordering::SeqCst);
    let first = FIRST
        .lock()
        .ok()
        .and_then(|f| f.clone())
        .unwrap_or_default();
    assert_eq!(
        n, 0,
        "the voice allocated {n} times while playing; the first:\n{first}"
    );
}

/// The same on its threads (its workers' parts moved into them, its pieces processed a block
/// at a time).
#[test]
fn the_voice_on_threads_does_not_allocate_while_it_plays() {
    use ca72::threaded::Threaded;
    let _serial = SERIAL.lock();
    let mut v = Threaded::new(Voice::prototype(48_000.0), || {}).unwrap();
    // The workers' own start (the thread's name registered by std) before counting: played
    // live they are audio threads only from their first pass.
    std::thread::sleep(std::time::Duration::from_millis(200));
    v.panel.ext_on = true;
    v.panel.ext_volume = 1.0;
    v.panel.glide = 0.4;
    let tau = 2.0 * std::f64::consts::PI;
    COUNT.store(0, Ordering::SeqCst);
    ARMED.store(true, Ordering::SeqCst);
    let mut at = 0usize;
    let mut sink = 0.0;
    while at < 96_000 {
        if at % 2400 < 128 {
            v.note(41 + (at / 2400 * 7) as i32 % 44, true);
        }
        if at % 2400 >= 1800 && at % 2400 < 1928 {
            v.note(41 + (at / 2400 * 7) as i32 % 44, false);
        }
        if at % 4000 < 128 {
            v.panel.keyboard_control_1 = !v.panel.keyboard_control_1;
            v.panel.glide_on = !v.panel.glide_on;
        }
        v.process(
            128,
            |i| {
                let t = (at + i) as f64 / 48_000.0;
                Jacks {
                    ext: 0.9 * (tau * 220.0 * t).sin(),
                    osc: Some(0.25 * (tau * 110.0 * t).sin()),
                    filter: Some(3.0 * (tau * 0.7 * t).sin()),
                    loudness: Some(4.0 + 4.0 * (tau * 5.0 * t).sin()),
                    s_trig: t.rem_euclid(0.35) < 0.005,
                    detune: [3.0 * (tau * 0.3 * t).sin(), -2.0, 1.5],
                    cutoff: 0.03 * (tau * 0.2 * t).sin(),
                }
            },
            |_, y| sink += y,
        );
        at += 128;
    }
    ARMED.store(false, Ordering::SeqCst);
    std::hint::black_box(sink);
    let n = COUNT.load(Ordering::SeqCst);
    let first = FIRST
        .lock()
        .ok()
        .and_then(|f| f.clone())
        .unwrap_or_default();
    assert_eq!(
        n, 0,
        "the voice on threads allocated {n} times while playing; the first:\n{first}"
    );
}
