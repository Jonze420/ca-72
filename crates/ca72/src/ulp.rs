//! The model's elementary functions (No Compromises). Plain `libm` calls, except with
//! the `twins` feature, where each result can be moved by one ulp up or down ([`set`]): the
//! same model as a slightly different but equally correct library would compute it. A
//! render's twins so computed show its own spread under rounding, which the owner's
//! fidelity rule allows a chaotic render (`ca72-lab perf`; history.md). `sqrt` is correctly
//! rounded everywhere, so it is not moved.

#[cfg(feature = "twins")]
static DIR: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Moves every result from now on by one ulp up (`1`), down (`-1`), or not (`0`). Only
/// with the `twins` feature; otherwise it does nothing.
pub fn set(dir: i32) {
    #[cfg(feature = "twins")]
    DIR.store(dir.signum(), std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "twins"))]
    let _ = dir;
}

#[cfg(feature = "twins")]
#[inline]
fn nudged(r: f64) -> f64 {
    if r == 0.0 || !r.is_finite() {
        return r;
    }
    match DIR.load(std::sync::atomic::Ordering::Relaxed) {
        1 => r.next_up(),
        -1 => r.next_down(),
        _ => r,
    }
}

#[cfg(not(feature = "twins"))]
#[inline(always)]
fn nudged(r: f64) -> f64 {
    r
}

/// With the `count` feature, each call counted by function and call site on its thread
/// (for finding where a mode's time goes; `ca72-lab worst`).
#[cfg(feature = "count")]
mod count {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::panic::Location;

    type Calls = HashMap<(&'static str, &'static Location<'static>), u64>;

    thread_local! {
        static CALLS: RefCell<Calls> = RefCell::new(HashMap::new());
    }

    #[track_caller]
    pub fn note(name: &'static str) {
        let at = Location::caller();
        CALLS.with(|c| *c.borrow_mut().entry((name, at)).or_default() += 1);
    }

    pub fn take() -> Vec<(String, u64)> {
        CALLS.with(|c| {
            c.borrow_mut()
                .drain()
                .map(|((name, at), n)| (format!("{name} {}:{}", at.file(), at.line()), n))
                .collect()
        })
    }
}

/// This thread's calls since the last take, by function and call site (most first); empty
/// without the `count` feature.
pub fn calls() -> Vec<(String, u64)> {
    #[cfg(feature = "count")]
    let mut v = count::take();
    #[cfg(not(feature = "count"))]
    let mut v: Vec<(String, u64)> = Vec::new();
    v.sort_by_key(|c| std::cmp::Reverse(c.1));
    v
}

macro_rules! unary {
    ($($name:ident),*) => {$(
        #[inline(always)]
        #[cfg_attr(feature = "count", track_caller)]
        pub fn $name(x: f64) -> f64 {
            #[cfg(feature = "count")]
            count::note(stringify!($name));
            nudged(libm::$name(x))
        }
    )*};
}

unary!(expm1, log, log1p, log2, tanh, sin, cos, tan);

thread_local! {
    /// Whether this thread is in a High Fidelity or Potato part's tick ([`FastScope`]).
    static FAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While it lives, [`exp`] on this thread is the fast one ([`crate::fast::exp`], within
/// about an ulp of libm's at twice its speed) if `fast`, libm's otherwise; the thread's
/// state before is restored when it drops, a panic included (Potato's, then High
/// Fidelity's too, its audio within -196 dBFS of No Compromises' where it had been the
/// same). Each part of the voice holds one through its tick, fast in every mode but No
/// Compromises, so No Compromises, and everything outside a tick (a voice's making and
/// settling), keep libm's to the bit.
#[must_use]
#[derive(Debug)]
pub struct FastScope {
    was: bool,
}

impl FastScope {
    pub fn new(fast: bool) -> FastScope {
        FastScope {
            was: FAST.with(|p| p.replace(fast)),
        }
    }
}

impl Drop for FastScope {
    fn drop(&mut self) {
        FAST.with(|p| p.set(self.was));
    }
}

/// e^x: libm's, or the fast one inside a High Fidelity or Potato part's tick
/// ([`FastScope`]).
#[inline(always)]
#[cfg_attr(feature = "count", track_caller)]
pub fn exp(x: f64) -> f64 {
    #[cfg(feature = "count")]
    count::note("exp");
    if FAST.with(std::cell::Cell::get) {
        return crate::fast::exp(x);
    }
    nudged(libm::exp(x))
}

/// tanh as [`exp`] is: libm's, or the fast one inside a High Fidelity or Potato part's tick
/// ([`crate::fast::tanh`], within 2e-14 of libm's relatively, about twice its speed). For the
/// transistor pairs' anchors ([`crate::devices::PairWarm`]: decisions.md R13); elsewhere
/// [`tanh`] stays libm's.
#[inline(always)]
#[cfg_attr(feature = "count", track_caller)]
pub fn tanh_in_tick(x: f64) -> f64 {
    #[cfg(feature = "count")]
    count::note("tanh_in_tick");
    if FAST.with(std::cell::Cell::get) {
        return crate::fast::tanh(x);
    }
    nudged(libm::tanh(x))
}

#[inline(always)]
#[cfg_attr(feature = "count", track_caller)]
pub fn pow(x: f64, y: f64) -> f64 {
    #[cfg(feature = "count")]
    count::note("pow");
    nudged(libm::pow(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fast scope changes `exp` on its thread only while it lives, nested scopes
    /// restore the one outside, and a panic inside restores it too.
    #[test]
    fn potato_scopes_restore_libm() {
        // An argument where the two differ (by an ulp).
        let x = (1..100_000)
            .map(|k| k as f64 * 1e-3)
            .find(|&x| libm::exp(x) != crate::fast::exp(x))
            .expect("an x where they differ");
        let (libm_e, fast_e) = (libm::exp(x), crate::fast::exp(x));
        assert_eq!(exp(x), libm_e);
        {
            let _a = FastScope::new(true);
            assert_eq!(exp(x), fast_e);
            {
                let _b = FastScope::new(false);
                assert_eq!(exp(x), libm_e);
            }
            assert_eq!(exp(x), fast_e);
            // Another thread is not affected.
            assert_eq!(std::thread::spawn(move || exp(x)).join().unwrap(), libm_e);
        }
        assert_eq!(exp(x), libm_e);
        let _ = std::panic::catch_unwind(|| {
            let _a = FastScope::new(true);
            panic!("inside a tick");
        });
        assert_eq!(exp(x), libm_e);
    }
}
