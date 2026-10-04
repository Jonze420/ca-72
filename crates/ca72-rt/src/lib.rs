//! Real-time scheduling for audio threads, for the lab and the tests: the threads that
//! process audio (a host's callback thread, the voice's workers) wake every block and must
//! not wait behind ordinary work. Taken from the real-time threads of the DAW the voice was
//! developed in; since then, inside a host's process, its watchdog watches only the threads
//! promoted here, Windows's threads join MMCSS, and the flush bits travel (decisions.md R18).
//!
//! - macOS: the Mach time-constraint policy, with the block period;
//! - Linux: `SCHED_FIFO`, which needs an rtprio allowance (`/etc/security/limits.conf`,
//!   rtkit or the `audio` group); without it the thread stays as it was;
//! - Windows: MMCSS's "Pro Audio" task at critical priority, as hosts' audio threads are
//!   (time-critical thread priority where MMCSS refuses), left when the thread ends.
//!
//! On Linux a thread at `SCHED_FIFO` that never stops (an overloaded engine, a bug)
//! keeps everything below its priority off its CPU, the kernel's interrupt threads
//! included; where the system's real-time throttling is off (`sched_rt_runtime_us` equal
//! to the period, as on the owner's machine) nothing stops it, and the machine stalls.
//! So a watchdog watches the threads promoted here ([`demotions`]): while any is alive, a
//! thread on each CPU at ordinary priority beats every 50 ms, and a thread above the audio
//! threads looks every 100 ms; when a CPU's beat is [`STARVED`] old while one of them runs
//! there, every thread promoted here is made ordinary (`SCHED_OTHER`), and it is counted
//! and said on stderr. The audio then runs on, late if it must, and the machine answers.
//! Never the host's own audio threads or another plug-in's: only threads promoted here.
//! With none alive, the watchdog's threads sleep until one is.

#![allow(unsafe_code)]

use std::time::Duration;

/// Promotes the calling thread for audio work that repeats every `period` and needs up
/// to `computation` of it. Returns an explanation when the system refuses.
pub fn promote(period: Duration, computation: Duration) -> Result<(), String> {
    // (On Linux the thread is the watchdog's, to watch and to demote, before it is promoted.)
    #[cfg(target_os = "linux")]
    let Some(new) = watchdog::register() else {
        return Err("too many real-time threads to watch".into());
    };
    let r = sys::promote(period, computation);
    #[cfg(target_os = "linux")]
    if r.is_ok() {
        watchdog::start();
    } else if new {
        watchdog::unregister();
    }
    r
}

/// Makes the calling thread ordinary again (Linux: `SCHED_OTHER`; Windows: out of MMCSS, at
/// normal priority; elsewhere nothing).
pub fn demote() {
    #[cfg(target_os = "linux")]
    {
        let param = libc::sched_param { sched_priority: 0 };
        // SAFETY: pid 0 is the calling thread; `param` is valid.
        unsafe { libc::sched_setscheduler(0, libc::SCHED_OTHER, &param) };
        watchdog::unregister();
    }
    #[cfg(windows)]
    sys::demote();
}

/// The calling thread's floating-point flush bits: on x86 MXCSR's flush-to-zero and
/// denormals-are-zero (FTZ, DAZ), on AArch64 FPCR's flush-to-zero (FZ); 0 elsewhere.
/// nih-plug sets FTZ on the host's thread for each block (a host may set DAZ too); a thread
/// that plays for it takes the same bits ([`set_flush_mode`]), so that denormals cost it
/// nothing either and its samples are the same to the bit.
pub fn flush_mode() -> u32 {
    flush::get() & FLUSH
}

/// Sets the calling thread's flush bits to those of `mode` (from [`flush_mode`]), its other
/// floating-point controls as they are. Allocates nothing.
pub fn set_flush_mode(mode: u32) {
    let now = flush::get();
    let want = (now & !FLUSH) | (mode & FLUSH);
    if want != now {
        flush::set(want);
    }
}

/// Every flush bit [`flush_mode`] reports here (none elsewhere).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub const FLUSH: u32 = 1 << 15 | 1 << 6;
#[cfg(target_arch = "aarch64")]
pub const FLUSH: u32 = 1 << 24;
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
pub const FLUSH: u32 = 0;

/// The floating-point control register, read and written as nih-plug's `ScopedFtz` does
/// (`third_party/nih-plug/src/wrapper/util.rs`): MXCSR on x86, FPCR on AArch64.
mod flush {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn get() -> u32 {
        let mut csr = 0u32;
        // SAFETY: stores MXCSR (SSE, which the x86 targets have) into a writable u32.
        unsafe { std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack)) };
        csr
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn set(csr: u32) {
        // SAFETY: loads MXCSR from a u32: what `get` read, with flush bits as another thread
        // of this machine has them (so bits this processor supports).
        unsafe { std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, readonly)) };
    }

    #[cfg(target_arch = "aarch64")]
    pub fn get() -> u32 {
        let fpcr: u64;
        // SAFETY: reads FPCR, as any thread may.
        unsafe { std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack)) };
        // (Its upper 32 bits are reserved: zero.)
        fpcr as u32
    }

    #[cfg(target_arch = "aarch64")]
    pub fn set(fpcr: u32) {
        // SAFETY: writes FPCR: what `get` read, with FZ as another thread has it.
        unsafe { std::arch::asm!("msr fpcr, {}", in(reg) u64::from(fpcr), options(nostack)) };
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn get() -> u32 {
        0
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn set(_: u32) {}
}

/// Starts the real-time watchdog now (Linux; once per process). [`promote`] starts it
/// too, but a thread may be promoted where starting threads should not happen (in an audio
/// thread's scope, which counts allocations): call this first from outside.
pub fn watch() {
    #[cfg(target_os = "linux")]
    watchdog::start();
}

/// How long a CPU's ordinary threads may go without running while a thread promoted here
/// runs there before the watchdog makes those ordinary (Linux).
pub const STARVED: Duration = Duration::from_millis(500);

/// How many times the watchdog has made the threads promoted here ordinary (Linux; 0
/// elsewhere).
pub fn demotions() -> u64 {
    #[cfg(target_os = "linux")]
    return watchdog::DEMOTIONS.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(target_os = "linux"))]
    0
}

/// Whether the calling thread runs at real-time priority (Linux: `SCHED_FIFO` or
/// `SCHED_RR`; elsewhere false).
pub fn is_realtime() -> bool {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: pid 0 is the calling thread.
        let p = unsafe { libc::sched_getscheduler(0) };
        p == libc::SCHED_FIFO || p == libc::SCHED_RR
    }
    #[cfg(not(target_os = "linux"))]
    false
}

/// The calling thread's id (Linux; 0 elsewhere), for [`pin`].
#[cfg(target_os = "linux")]
pub fn tid() -> i32 {
    // SAFETY: no arguments, always succeeds.
    unsafe { libc::gettid() }
}

#[cfg(not(target_os = "linux"))]
pub fn tid() -> i32 {
    0
}

/// Pins thread `tid` (0: the calling thread) to one logical CPU (Linux; elsewhere it does
/// nothing). For measurements (`ca72-lab worst`): a host does not pin its threads.
#[cfg(target_os = "linux")]
pub fn pin(tid: i32, cpu: usize) -> Result<(), String> {
    // SAFETY: a zeroed cpu_set_t is a valid empty set.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu, &mut set);
        if libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin(_tid: i32, _cpu: usize) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
mod sys {
    use std::time::Duration;

    // From <mach/mach_time.h> and <mach/thread_policy.h> (declared here: libc's copies are
    // deprecated in favour of another crate).
    #[repr(C)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }

    #[repr(C)]
    struct TimeConstraintPolicy {
        period: u32,
        computation: u32,
        constraint: u32,
        preemptible: i32,
    }

    const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
    const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;

    unsafe extern "C" {
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
        // (Not `mach_thread_self`, whose every call takes a reference to the thread's port
        // that must be given back: this one takes none.)
        fn pthread_mach_thread_np(thread: libc::pthread_t) -> u32;
        fn thread_policy_set(thread: u32, flavor: u32, info: *mut i32, count: u32) -> i32;
    }

    pub fn promote(period: Duration, computation: Duration) -> Result<(), String> {
        let mut tb = MachTimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: `tb` is a valid, writable timebase structure.
        if unsafe { mach_timebase_info(&mut tb) } != 0 || tb.numer == 0 {
            return Err("no Mach timebase".into());
        }
        // Nanoseconds to Mach absolute time units.
        let abs = |d: Duration| -> u32 {
            (d.as_nanos() * u128::from(tb.denom) / u128::from(tb.numer)).min(u128::from(u32::MAX))
                as u32
        };
        let mut policy = TimeConstraintPolicy {
            period: abs(period),
            computation: abs(computation.min(period)),
            constraint: abs(period),
            preemptible: 1,
        };
        // SAFETY: setting our own thread's policy with a correctly sized structure (four
        // 32-bit fields, the policy's count); the port is the calling thread's own.
        let kr = unsafe {
            thread_policy_set(
                pthread_mach_thread_np(libc::pthread_self()),
                THREAD_TIME_CONSTRAINT_POLICY,
                (&mut policy as *mut TimeConstraintPolicy).cast(),
                THREAD_TIME_CONSTRAINT_POLICY_COUNT,
            )
        };
        if kr == 0 {
            Ok(())
        } else {
            Err(format!("thread_policy_set failed ({kr})"))
        }
    }
}

/// The starvation watchdog (Linux; see the module's documentation).
#[cfg(target_os = "linux")]
mod watchdog {
    use super::STARVED;
    use std::cell::Cell;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
    use std::thread::Thread;
    use std::time::{Duration, Instant};

    pub static DEMOTIONS: AtomicU64 = AtomicU64::new(0);

    /// The threads promoted here and alive, by their kernel ids (0: a free place): the only
    /// threads the watchdog makes ordinary. Each lets go of its place when it ends (before
    /// its id can be another's) or is demoted. A table of its own, so that promoting a
    /// thread allocates nothing but, the first time, its place's clean-up.
    static OURS: [AtomicI32; 256] = [const { AtomicI32::new(0) }; 256];

    thread_local! {
        /// The calling thread's place in [`OURS`], if it has one (`usize::MAX`: none).
        static PLACE: Place = const { Place(Cell::new(usize::MAX)) };
    }

    struct Place(Cell<usize>);

    impl Place {
        fn let_go(&self) {
            if let Some(t) = OURS.get(self.0.replace(usize::MAX)) {
                t.store(0, Ordering::Release);
            }
        }
    }

    impl Drop for Place {
        fn drop(&mut self) {
            self.let_go();
        }
    }

    /// The calling thread becomes one of ours: whether it is new, or None if the table is
    /// full.
    pub fn register() -> Option<bool> {
        PLACE
            .try_with(|p| {
                if p.0.get() != usize::MAX {
                    return Some(false);
                }
                // SAFETY: no arguments, always succeeds.
                let tid = unsafe { libc::gettid() };
                let k = OURS.iter().position(|t| {
                    t.compare_exchange(0, tid, Ordering::AcqRel, Ordering::Relaxed)
                        .is_ok()
                })?;
                p.0.set(k);
                if let Some(w) = WATCHDOG.get() {
                    w.unpark();
                }
                Some(true)
            })
            .ok()
            .flatten()
    }

    /// The calling thread is no longer one of ours.
    pub fn unregister() {
        let _ = PLACE.try_with(Place::let_go);
    }

    /// The threads of ours alive now (for the tests).
    #[cfg(test)]
    pub fn ours() -> Vec<i32> {
        OURS.iter()
            .map(|t| t.load(Ordering::Acquire))
            .filter(|&t| t != 0)
            .collect()
    }

    /// Each CPU's canary's last beat: milliseconds since `epoch` (0 until its first, so a
    /// canary that never runs counts as starved from the watch's start); whether the
    /// canaries beat (while a thread of ours is alive) and since when; and the canaries.
    struct Beats {
        epoch: Instant,
        cpus: Vec<(usize, AtomicU64)>,
        active: AtomicBool,
        since: AtomicU64,
        canaries: OnceLock<Vec<Thread>>,
    }

    static STARTED: OnceLock<()> = OnceLock::new();

    /// The watchdog's thread, which a thread of ours wakes when it is promoted.
    static WATCHDOG: OnceLock<Thread> = OnceLock::new();

    const BEAT: Duration = Duration::from_millis(50);
    const LOOK: Duration = Duration::from_millis(100);
    /// How often the watchdog looks for a thread of ours while there is none (it is woken
    /// when there is; this is in case it was not yet known to wake).
    const IDLE: Duration = Duration::from_secs(1);

    /// Starts the canaries and the watchdog, once per process.
    pub fn start() {
        STARTED.get_or_init(|| {
            let cpus = allowed_cpus();
            if cpus.is_empty() {
                return;
            }
            let beats = Box::leak(Box::new(Beats {
                epoch: Instant::now(),
                cpus: cpus.iter().map(|&c| (c, AtomicU64::new(0))).collect(),
                active: AtomicBool::new(false),
                since: AtomicU64::new(0),
                canaries: OnceLock::new(),
            }));
            let beats: &'static Beats = beats;
            let mut canaries = Vec::with_capacity(beats.cpus.len());
            for k in 0..beats.cpus.len() {
                if let Ok(h) = std::thread::Builder::new()
                    .name("ca72-rt-canary".into())
                    .spawn(move || canary(beats, k))
                {
                    canaries.push(h.thread().clone());
                }
            }
            let _ = beats.canaries.set(canaries);
            if let Ok(h) = std::thread::Builder::new()
                .name("ca72-rt-watchdog".into())
                .spawn(move || watch(beats))
            {
                let _ = WATCHDOG.set(h.thread().clone());
            }
        });
    }

    fn allowed_cpus() -> Vec<usize> {
        // SAFETY: a zeroed cpu_set_t is valid; pid 0 is the calling thread.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
                return Vec::new();
            }
            (0..libc::CPU_SETSIZE as usize)
                .filter(|&c| libc::CPU_ISSET(c, &set))
                .collect()
        }
    }

    fn millis(b: &Beats) -> u64 {
        b.epoch.elapsed().as_millis() as u64
    }

    /// Beats on CPU `k` at ordinary priority (a thread made by a real-time one inherits
    /// its policy), while the watchdog watches.
    fn canary(b: &'static Beats, k: usize) {
        let param = libc::sched_param { sched_priority: 0 };
        // SAFETY: pid 0 is the calling thread; `param` is valid.
        unsafe { libc::sched_setscheduler(0, libc::SCHED_OTHER, &param) };
        let (cpu, beat) = (&b.cpus[k].0, &b.cpus[k].1);
        if super::pin(0, *cpu).is_err() {
            // Not ours to run on after all: never counted as starved.
            beat.store(u64::MAX, Ordering::Relaxed);
            return;
        }
        loop {
            if !b.active.load(Ordering::Acquire) {
                std::thread::park();
                continue;
            }
            beat.store(millis(b), Ordering::Relaxed);
            std::thread::sleep(BEAT);
        }
    }

    /// Looks at the beats from above the audio threads' priority while a thread of ours is
    /// alive.
    fn watch(b: &'static Beats) {
        // The highest priority allowed (RLIMIT_RTPRIO), at most 99.
        let top = {
            let mut r = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            // SAFETY: a valid, writable rlimit.
            if unsafe { libc::getrlimit(libc::RLIMIT_RTPRIO, &mut r) } == 0 {
                r.rlim_cur.min(99) as i32
            } else {
                0
            }
        };
        let param = libc::sched_param {
            sched_priority: top,
        };
        // SAFETY: pid 0 is the calling thread; `param` is valid.
        let ok = top > 0 && unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) } == 0;
        // (Said once, when there is first a thread to watch.)
        let mut said = false;
        // SAFETY: no arguments, always succeeds.
        let me = unsafe { libc::gettid() };
        loop {
            if !OURS.iter().any(|t| t.load(Ordering::Acquire) != 0) {
                b.active.store(false, Ordering::Release);
                std::thread::park_timeout(IDLE);
                continue;
            }
            if !b.active.load(Ordering::Relaxed) {
                // (Each CPU counted from now: the beats before are old.)
                b.since.store(millis(b), Ordering::Relaxed);
                b.active.store(true, Ordering::Release);
                for c in b.canaries.get().into_iter().flatten() {
                    c.unpark();
                }
                if (!ok || top <= 70) && !std::mem::replace(&mut said, true) {
                    eprintln!(
                        "ca72: the real-time watchdog cannot run above the audio threads (real-time priority allowed: {top})"
                    );
                }
            }
            std::thread::sleep(LOOK);
            let now = millis(b);
            let since = b.since.load(Ordering::Relaxed);
            for (cpu, beat) in &b.cpus {
                let last = beat.load(Ordering::Relaxed);
                // Not a CPU it may run on.
                if last == u64::MAX {
                    continue;
                }
                let last = last.max(since);
                if now.saturating_sub(last) < STARVED.as_millis() as u64 {
                    continue;
                }
                let ours = realtime_threads(me);
                if ours.iter().any(|t| t.running_on == Some(*cpu)) {
                    for t in &ours {
                        let param = libc::sched_param { sched_priority: 0 };
                        // SAFETY: a thread of this process; `param` is valid.
                        unsafe { libc::sched_setscheduler(t.tid, libc::SCHED_OTHER, &param) };
                    }
                    DEMOTIONS.fetch_add(1, Ordering::Relaxed);
                    eprintln!(
                        "ca72: CPU {cpu}'s ordinary threads had not run for {} ms under CA-72's real-time threads: its {} real-time threads now run at ordinary priority",
                        now.saturating_sub(last),
                        ours.len()
                    );
                    break;
                }
            }
        }
    }

    pub struct RtThread {
        pub tid: i32,
        /// The CPU it is running on, if it is running.
        running_on: Option<usize>,
    }

    /// The threads of ours at real-time priority but `me`.
    pub fn realtime_threads(me: i32) -> Vec<RtThread> {
        let mut out = Vec::new();
        for t in &OURS {
            let tid = t.load(Ordering::Acquire);
            if tid == 0 || tid == me {
                continue;
            }
            // SAFETY: a thread id; an error (the thread has gone) is -1.
            let p = unsafe { libc::sched_getscheduler(tid) };
            if p != libc::SCHED_FIFO && p != libc::SCHED_RR {
                continue;
            }
            // `/proc/self/task/<tid>/stat`: after the name's closing parenthesis, the state
            // (field 3) and, 36 fields on, the CPU it last ran on (field 39).
            let running_on = std::fs::read_to_string(format!("/proc/self/task/{tid}/stat"))
                .ok()
                .and_then(|st| {
                    let rest = &st[st.rfind(')')? + 1..];
                    let f: Vec<&str> = rest.split_whitespace().collect();
                    (f.first() == Some(&"R"))
                        .then(|| f.get(36)?.parse().ok())
                        .flatten()
                });
            out.push(RtThread { tid, running_on });
        }
        out
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::time::Duration;

    pub fn promote(_period: Duration, _computation: Duration) -> Result<(), String> {
        let param = libc::sched_param { sched_priority: 70 };
        // SAFETY: pid 0 is the calling thread; `param` is valid.
        if unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) } == 0 {
            Ok(())
        } else {
            Err(format!(
                "SCHED_FIFO refused ({}); allow real-time priority for this user to use it",
                std::io::Error::last_os_error()
            ))
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::cell::Cell;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Threading::{
        AVRT_PRIORITY_CRITICAL, AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
        AvSetMmThreadPriority, GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_NORMAL,
        THREAD_PRIORITY_TIME_CRITICAL,
    };

    /// MMCSS's task for audio threads, "Pro Audio", as a C wide string.
    const PRO_AUDIO: [u16; 10] = {
        let s = b"Pro Audio\0";
        let mut w = [0u16; 10];
        let mut i = 0;
        while i < s.len() {
            w[i] = s[i] as u16;
            i += 1;
        }
        w
    };

    thread_local! {
        /// The calling thread's MMCSS task (null: none), left when the thread ends: MMCSS
        /// holds a limited number of them.
        static TASK: Task = const { Task(Cell::new(std::ptr::null_mut())) };
    }

    struct Task(Cell<HANDLE>);

    impl Task {
        fn leave(&self) {
            let h = self.0.replace(std::ptr::null_mut());
            if !h.is_null() {
                // SAFETY: the task this thread joined, left once.
                unsafe { AvRevertMmThreadCharacteristics(h) };
            }
        }
    }

    impl Drop for Task {
        fn drop(&mut self) {
            self.leave();
        }
    }

    /// The calling thread joins MMCSS's "Pro Audio" task at critical priority, as hosts'
    /// audio threads do (priorities 16 to 31, above time-critical's 15): otherwise the
    /// host's audio threads would run before the workers its own thread waits for. Where
    /// MMCSS refuses (its service off), time-critical priority.
    pub fn promote(_period: Duration, _computation: Duration) -> Result<(), String> {
        let joined = TASK
            .try_with(|t| {
                if t.0.get().is_null() {
                    let mut index = 0u32;
                    // SAFETY: a C wide string and a writable task index.
                    let h =
                        unsafe { AvSetMmThreadCharacteristicsW(PRO_AUDIO.as_ptr(), &mut index) };
                    if h.is_null() {
                        return false;
                    }
                    // SAFETY: the task just joined.
                    unsafe { AvSetMmThreadPriority(h, AVRT_PRIORITY_CRITICAL) };
                    t.0.set(h);
                }
                true
            })
            .unwrap_or(false);
        if joined {
            return Ok(());
        }
        // SAFETY: the pseudo-handle of the calling thread.
        if unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) } != 0 {
            Ok(())
        } else {
            Err(format!(
                "SetThreadPriority failed ({})",
                std::io::Error::last_os_error()
            ))
        }
    }

    pub fn demote() {
        let _ = TASK.try_with(Task::leave);
        // SAFETY: the pseudo-handle of the calling thread.
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL) };
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod sys {
    use std::time::Duration;

    pub fn promote(_period: Duration, _computation: Duration) -> Result<(), String> {
        Err("real-time scheduling is not implemented on this platform".into())
    }
}

/// On macOS any thread may take the time-constraint policy.
#[cfg(all(test, target_os = "macos"))]
#[test]
fn a_thread_is_promoted() {
    let r = std::thread::spawn(|| promote(Duration::from_millis(5), Duration::from_millis(1)))
        .join()
        .map_err(|_| "panicked".to_owned());
    assert_eq!(r, Ok(Ok(())));
}

/// A work interval of the process's own (macOS: an `os_workgroup_interval` from
/// `AudioWorkIntervalCreate`, macOS 11 on): threads that join it and declare each period's
/// start and deadline tell the system they work to that deadline together, so it runs them
/// fast enough to meet it (as it does an audio device's own threads). A plug-in cannot join
/// its host's (VST3 has no way to hand it over), so its own workers join one of their own.
/// Elsewhere it does nothing.
#[derive(Debug)]
pub struct WorkInterval {
    #[cfg(target_os = "macos")]
    wg: workgroup::Handle,
}

impl WorkInterval {
    /// A new interval named `name`; None where there is none to have.
    pub fn new(name: &str) -> Option<WorkInterval> {
        #[cfg(target_os = "macos")]
        return workgroup::Handle::new(name).map(|wg| WorkInterval { wg });
        #[cfg(not(target_os = "macos"))]
        {
            let _ = name;
            None
        }
    }

    /// The calling thread joins it (until it leaves: [`WorkInterval::leave`]); false if the
    /// system refuses.
    pub fn join(&self) -> Option<JoinToken> {
        #[cfg(target_os = "macos")]
        return self.wg.join().map(|t| JoinToken { t });
        #[cfg(not(target_os = "macos"))]
        None
    }

    pub fn leave(&self, token: JoinToken) {
        #[cfg(target_os = "macos")]
        self.wg.leave(token.t);
        #[cfg(not(target_os = "macos"))]
        let _ = token;
    }

    /// A period of work starts now, to be done within `deadline` (by a member thread).
    pub fn start(&self, deadline: Duration) -> bool {
        #[cfg(target_os = "macos")]
        return self.wg.start(deadline);
        #[cfg(not(target_os = "macos"))]
        {
            let _ = deadline;
            false
        }
    }

    /// The period's work is done (by a member thread).
    pub fn finish(&self) -> bool {
        #[cfg(target_os = "macos")]
        return self.wg.finish();
        #[cfg(not(target_os = "macos"))]
        false
    }
}

/// A thread's membership of a [`WorkInterval`].
#[derive(Debug)]
pub struct JoinToken {
    #[cfg(target_os = "macos")]
    t: Box<workgroup::Token>,
}

#[cfg(target_os = "macos")]
mod workgroup {
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::time::Duration;

    /// `os_workgroup_join_token_s`: its signature and 36 opaque bytes.
    #[repr(C)]
    #[derive(Debug)]
    pub struct Token {
        sig: u32,
        opaque: [u8; 36],
    }

    #[repr(C)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }

    /// `OS_CLOCK_MACH_ABSOLUTE_TIME`.
    const MACH_ABSOLUTE_TIME: u32 = 32;

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioWorkIntervalCreate(
            name: *const c_char,
            clock: u32,
            attr: *mut c_void,
        ) -> *mut c_void;
    }

    unsafe extern "C" {
        fn os_workgroup_join(wg: *mut c_void, token: *mut Token) -> c_int;
        fn os_workgroup_leave(wg: *mut c_void, token: *mut Token);
        fn os_workgroup_interval_start(
            wg: *mut c_void,
            start: u64,
            deadline: u64,
            data: *mut c_void,
        ) -> c_int;
        fn os_workgroup_interval_finish(wg: *mut c_void, data: *mut c_void) -> c_int;
        fn os_release(object: *mut c_void);
        fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    }

    /// The interval (retained; released when dropped).
    #[derive(Debug)]
    pub struct Handle {
        wg: *mut c_void,
        /// Mach absolute time units a nanosecond: numerator and denominator.
        tb: (u32, u32),
    }

    // SAFETY: an os_workgroup is a thread-safe object, used from any thread by its API.
    unsafe impl Send for Handle {}
    // SAFETY: as above.
    unsafe impl Sync for Handle {}

    impl Handle {
        pub fn new(name: &str) -> Option<Handle> {
            let name = CString::new(name).ok()?;
            let mut tb = MachTimebaseInfo { numer: 0, denom: 0 };
            // SAFETY: `tb` is a valid, writable timebase structure.
            if unsafe { mach_timebase_info(&mut tb) } != 0 || tb.numer == 0 {
                return None;
            }
            // SAFETY: a valid C string, the Mach clock and no attributes (the default).
            let wg = unsafe {
                AudioWorkIntervalCreate(name.as_ptr(), MACH_ABSOLUTE_TIME, std::ptr::null_mut())
            };
            (!wg.is_null()).then_some(Handle {
                wg,
                tb: (tb.numer, tb.denom),
            })
        }

        pub fn join(&self) -> Option<Box<Token>> {
            let mut t = Box::new(Token {
                sig: 0,
                opaque: [0; 36],
            });
            // SAFETY: the workgroup is alive (held by `self`) and the token is writable.
            (unsafe { os_workgroup_join(self.wg, &mut *t) } == 0).then_some(t)
        }

        #[allow(clippy::boxed_local)]
        pub fn leave(&self, mut t: Box<Token>) {
            // (Boxed so that the token stays where `join` filled it.)
            // SAFETY: the token is the one `join` filled on this thread.
            unsafe { os_workgroup_leave(self.wg, &mut *t) };
        }

        pub fn start(&self, deadline: Duration) -> bool {
            let (numer, denom) = self.tb;
            let ticks = (deadline.as_nanos() * u128::from(denom) / u128::from(numer)) as u64;
            // SAFETY: the clock read has no preconditions; the workgroup is alive.
            unsafe {
                let now = mach_absolute_time();
                os_workgroup_interval_start(self.wg, now, now + ticks, std::ptr::null_mut()) == 0
            }
        }

        pub fn finish(&self) -> bool {
            // SAFETY: the workgroup is alive.
            unsafe { os_workgroup_interval_finish(self.wg, std::ptr::null_mut()) == 0 }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the workgroup was created (retained) by `new` and is released once.
            unsafe { os_release(self.wg) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    /// The flush bits are set and read back, the other controls kept, and flush a denormal
    /// result.
    #[test]
    fn the_flush_bits_round_trip_and_flush() {
        // (The controls: on x86 MXCSR's low six bits are flags the arithmetic sets.)
        let controls = || {
            let x = flush::get();
            if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
                x & !0x3f
            } else {
                x
            }
        };
        let was = controls();
        let half = || black_box(black_box(f64::MIN_POSITIVE) * black_box(0.5));
        set_flush_mode(FLUSH);
        assert_eq!(flush_mode(), FLUSH);
        assert_eq!(controls() & !FLUSH, was & !FLUSH);
        if FLUSH != 0 {
            assert_eq!(half(), 0.0);
        }
        set_flush_mode(0);
        assert_eq!(flush_mode(), 0);
        assert!(half() > 0.0);
        set_flush_mode(was);
        assert_eq!(controls(), was);
    }

    /// Only threads promoted here are the watchdog's: one that ends lets go of its place
    /// (its id may be another thread's next), and a real-time thread made so otherwise (the
    /// host's own, say) is never one it would make ordinary.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_watchdog_demotes_only_threads_promoted_here() {
        let ended = std::thread::spawn(|| {
            assert_eq!(watchdog::register(), Some(true));
            assert_eq!(watchdog::register(), Some(false));
            let me = tid();
            assert!(watchdog::ours().contains(&me));
            me
        })
        .join()
        .unwrap_or_default();
        assert!(!watchdog::ours().contains(&ended));
        // A thread that runs, real-time if it may, until told to stop.
        let thread = |promoted: bool| {
            let (tx, rx) = std::sync::mpsc::channel();
            let (stop, stopped) = std::sync::mpsc::channel::<()>();
            let h = std::thread::spawn(move || {
                let ok = if promoted {
                    promote(Duration::from_millis(5), Duration::from_millis(1)).is_ok()
                } else {
                    let param = libc::sched_param { sched_priority: 10 };
                    // SAFETY: pid 0 is the calling thread; `param` is valid.
                    unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) == 0 }
                };
                let _ = tx.send((tid(), ok));
                let _ = stopped.recv();
            });
            let (t, ok) = rx.recv().unwrap_or_default();
            (t, ok, stop, h)
        };
        let (host, host_rt, stop_host, h1) = thread(false);
        let (ours, our_rt, stop_ours, h2) = thread(true);
        let listed: Vec<i32> = watchdog::realtime_threads(0)
            .iter()
            .map(|t| t.tid)
            .collect();
        assert!(!listed.contains(&host) && !watchdog::ours().contains(&host));
        assert_eq!(watchdog::ours().contains(&ours), our_rt);
        assert_eq!(listed.contains(&ours), our_rt);
        if !(host_rt && our_rt) {
            eprintln!("real-time priority is not allowed here: checked in part");
        }
        let _ = (stop_host.send(()), stop_ours.send(()));
        let _ = (h1.join(), h2.join());
        assert!(!watchdog::ours().contains(&ours));
    }
}
