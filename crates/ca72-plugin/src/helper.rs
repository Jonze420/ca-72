//! The plug-in's own helper thread (decisions.md R23): it does for the audio thread what must
//! not be done there, mending the engine's spare voices ([`Spares::mend`]; R18) and starting
//! and stopping POLY's workers ([`Crew::serve`]; R21), when the audio thread asks, which
//! allocates nothing and does not wait (a bit set, an unpark).
//!
//! nih-plug's background thread did this before. Shared by every instance, it holds the
//! plug-in alive while it runs a task: a host that destroyed the plug-in then got back at once
//! and could unload its library under that thread and the voices' workers (on Windows, a
//! crash). The helper is the plug-in's own: dropped, the plug-in stops it (a mend in progress
//! stopping before its next voice) and joins it, then stops and joins its voices' workers, so
//! no thread of the plug-in's runs once the host's destroy returns.

use crate::engine::Spares;
use crate::pool::Crew;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;

/// What the audio thread asks of the helper: the spares mended.
pub const MEND: u32 = 1;
/// POLY's workers started or stopped as the audio thread asked.
pub const SERVE: u32 = 2;

/// What the helper and the plug-in share: what is asked, and whether to stop.
#[derive(Debug, Default)]
struct Shared {
    asks: AtomicU32,
    quit: AtomicBool,
    /// While set, a round waits before it begins, `held` set meanwhile; `ended`, set as the
    /// thread ends (the tests').
    #[cfg(test)]
    hold: AtomicBool,
    #[cfg(test)]
    held: AtomicBool,
    #[cfg(test)]
    ended: AtomicBool,
}

/// The helper thread, stopped and joined when dropped.
#[derive(Debug)]
pub struct Helper {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Helper {
    /// The helper started for these spares and this crew. Not on the audio thread.
    pub fn start(spares: Arc<Spares>, crew: Arc<Crew>) -> std::io::Result<Helper> {
        let shared = Arc::new(Shared::default());
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("ca72-helper".to_owned())
            .spawn(move || help(&s, &spares, &crew))?;
        Ok(Helper {
            shared,
            thread: Some(thread),
        })
    }

    /// `asks` ([`MEND`], [`SERVE`]) done as soon as the helper can. On the audio thread:
    /// allocates nothing and does not wait.
    pub fn ask(&self, asks: u32) {
        if asks == 0 {
            return;
        }
        self.shared.asks.fetch_or(asks, Ordering::Release);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }
}

impl Drop for Helper {
    /// Stopped (a mend in progress before its next voice, a serve once done) and joined. Not
    /// on the audio thread.
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

/// The helper: parked until asked, then the asks done, until told to stop.
fn help(s: &Shared, spares: &Spares, crew: &Crew) {
    let quit = || s.quit.load(Ordering::Acquire);
    while !quit() {
        let asks = s.asks.swap(0, Ordering::AcqRel);
        if asks == 0 {
            // (An ask made since the swap leaves the unpark's token: this returns at once.)
            std::thread::park();
            continue;
        }
        #[cfg(test)]
        hold(s);
        if asks & MEND != 0 {
            spares.mend_until(quit);
        }
        // (Stopping, a pool started would only be stopped again.)
        if asks & SERVE != 0 && !quit() {
            crew.serve();
        }
    }
    #[cfg(test)]
    s.ended.store(true, Ordering::Release);
}

/// A round held while the test holds it.
#[cfg(test)]
fn hold(s: &Shared) {
    while s.hold.load(Ordering::Acquire) {
        s.held.store(true, Ordering::Release);
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
}

/// What the tests see of a helper, and hold it by, after it is dropped too.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct Watch(Arc<Shared>);

#[cfg(test)]
impl Watch {
    /// Its next round held before it begins (`true`), or let go.
    pub(crate) fn hold(&self, on: bool) {
        self.0.hold.store(on, Ordering::Release);
    }

    /// Whether a round is being held.
    pub(crate) fn held(&self) -> bool {
        self.0.held.load(Ordering::Acquire)
    }

    /// Whether the thread has ended.
    pub(crate) fn ended(&self) -> bool {
        self.0.ended.load(Ordering::Acquire)
    }
}

#[cfg(test)]
impl Helper {
    pub(crate) fn watch(&self) -> Watch {
        Watch(self.shared.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Waits up to 10 s for `f` (bounded by what happens, not by time: it fails only if the
    /// helper never gets there).
    fn until(what: &str, f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(10), "{what}");
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    /// Asked, the helper serves the crew; dropped while a round is held, it is waited for: the
    /// drop returns only once the thread has ended, the round let go meanwhile.
    #[test]
    fn a_helper_dropped_mid_round_is_waited_for() {
        let (spares, crew) = (Arc::new(Spares::default()), Arc::new(Crew::new()));
        let helper = Helper::start(spares.clone(), crew.clone()).unwrap();
        let watch = helper.watch();
        watch.hold(true);
        helper.ask(MEND | SERVE);
        until("the round never began", || watch.held());
        let (tx, rx) = std::sync::mpsc::channel();
        let w = watch.clone();
        let dropper = std::thread::spawn(move || {
            drop(helper);
            tx.send(w.ended()).unwrap();
        });
        // (The drop must not return while the round is held: given a moment to, it has not.)
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "the drop returned with the helper's round still running"
        );
        watch.hold(false);
        assert!(
            rx.recv().unwrap(),
            "the drop returned before the thread ended"
        );
        dropper.join().unwrap();
        // (Nothing of the helper's holds the spares or the crew any more.)
        assert_eq!(
            (Arc::strong_count(&spares), Arc::strong_count(&crew)),
            (1, 1)
        );
    }
}
