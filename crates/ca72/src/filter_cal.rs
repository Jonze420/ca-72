//! The filter's factory calibration (Folkman 1973, the owner's choice; board4.md):
//! "Regeneration Cal." (R73), "Filter Range" (R39) and "Filter Scale" (R49), done on the
//! real-time filter as the procedure does it on the instrument: by listening to the filter
//! oscillate and zero-beating it against the A-440.
//!
//! The trims depend only on the circuit, not the panel, so the procedure's result is kept
//! as [`FACTORY`]; the test `filter_calibration` runs the procedure again and compares.

use crate::expo::Input;
use crate::keyboard::{Keyboard, KeyboardCircuit};
use crate::vcf::{FilterExpo, Vcf, VcfCircuit};

/// The three trims: R39's wiper (RANGE, 0..1 from its +10 V end), R49 (SCALE, ohms in
/// circuit, 0..500) and R73's wiper (REGEN CAL, 0..1 from R76).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterTrims {
    pub r39: f64,
    pub r49: f64,
    pub r73_pos: f64,
}

/// The trims [`folkman`] gives (48 kHz; 2026-09-29).
pub const FACTORY: FilterTrims = FilterTrims {
    r39: 0.632274,
    r49: 327.0304,
    r73_pos: 0.782635,
};

impl FilterTrims {
    /// The converter with these trims.
    pub fn expo(&self) -> FilterExpo {
        FilterExpo {
            r39: self.r39,
            r49: self.r49,
            ..FilterExpo::default()
        }
    }

    /// The filter's circuit with these trims.
    pub fn circuit(&self) -> VcfCircuit {
        VcfCircuit {
            r73_pos: self.r73_pos,
            ..VcfCircuit::default()
        }
    }
}

/// The mixer's bus with every channel switched off: each series resistor loads it (three
/// oscillators' and the external input's 33K, the noise's 11K).
pub const G_BUS_OFF: f64 = 4.0 / 33e3 + 1.0 / 11e3;

/// The control node's inputs for CUTOFF (0..1), KEYBOARD CONTROL 1 and 2 on the keyboard's
/// voltage, AMOUNT OF CONTOUR at 0, FILTER MODULATION off and the external control's jack
/// empty (the voice's own inputs, [`crate::voice`]).
pub fn inputs(cutoff: f64, kb1: bool, kb2: bool, v_kbd: f64) -> [Input; 6] {
    let pos = cutoff.clamp(0.0, 1.0);
    let open = Input {
        r: f64::INFINITY,
        v: 0.0,
    };
    [
        Input {
            r: 200e3 + 5e3 * pos * (1.0 - pos),
            v: -10.0 + 20.0 * pos,
        },
        Input { r: 47e3, v: 0.0 },
        if kb1 {
            Input { r: 300e3, v: v_kbd }
        } else {
            open
        },
        if kb2 {
            Input { r: 150e3, v: v_kbd }
        } else {
            open
        },
        Input { r: 33e3, v: 0.0 },
        Input { r: 100e3, v: 0.0 },
    ]
}

/// The filter left to oscillate (a short kick on the bus at the start) with `r14` ohms of
/// EMPHASIS and the ladder current `i0`: its output from 0.1 s for `seconds`.
fn run(rate: f64, trims: &FilterTrims, r14: f64, i0: f64, seconds: f64) -> Vec<f64> {
    let mut f = Vcf::new(rate, 4);
    f.circuit = trims.circuit();
    f.circuit.r14 = r14;
    let n = ((0.1 + seconds) * rate) as usize;
    let skip = (0.1 * rate) as usize;
    let mut out = Vec::with_capacity(n - skip);
    for k in 0..n {
        let kick = if k < 8 { 20e-6 } else { 0.0 };
        let y = f.tick(kick, G_BUS_OFF, i0);
        if k >= skip {
            out.push(y);
        }
    }
    out
}

/// The oscillation's frequency (rising crossings of its mean, interpolated), Hz.
fn frequency(x: &[f64], rate: f64) -> f64 {
    let m = x.iter().sum::<f64>() / x.len() as f64;
    let ups: Vec<f64> = x
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < m && w[1] >= m)
        .map(|(i, w)| i as f64 + (m - w[0]) / (w[1] - w[0]))
        .collect();
    if ups.len() < 3 {
        return 0.0;
    }
    rate * (ups.len() - 1) as f64 / (ups[ups.len() - 1] - ups[0])
}

/// The self-oscillation's frequency at EMPHASIS 10 for the control node's inputs, Hz.
pub fn oscillation(rate: f64, trims: &FilterTrims, ins: &[Input]) -> f64 {
    let i0 = trims.expo().current(ins, 25.0);
    frequency(
        &run(rate, trims, 0.0, i0, 0.1)[(0.05 * rate) as usize..],
        rate,
    )
}

/// Whether the filter regenerates (its oscillation grows) with EMPHASIS at `emphasis`
/// (0..1) for the control node's inputs.
pub fn regenerates(rate: f64, trims: &FilterTrims, ins: &[Input], emphasis: f64) -> bool {
    let i0 = trims.expo().current(ins, 25.0);
    let x = run(rate, trims, crate::voice::emphasis_r14(emphasis), i0, 0.4);
    let peak = |a: f64, b: f64| {
        x[(a * rate) as usize..(b * rate) as usize]
            .iter()
            .fold(0.0f64, |m, v| m.max(v.abs()))
    };
    // Growing, or already at its limit (the kick's own response has died away by then).
    let (early, late) = (peak(0.05, 0.15), peak(0.3, 0.4));
    late > early || late > 0.05
}

/// Finds `x` in `[lo, hi]` where `f(x)` crosses `target` (bisection, its direction from the
/// ends); `None` if it does not.
fn bisect(lo: f64, hi: f64, iters: usize, f: impl Fn(f64) -> f64, target: f64) -> Option<f64> {
    let (f_lo, f_hi) = (f(lo), f(hi));
    if (f_lo - target) * (f_hi - target) > 0.0 {
        return None;
    }
    let rising = f_hi > f_lo;
    let (mut a, mut b) = (lo, hi);
    for _ in 0..iters {
        let m = 0.5 * (a + b);
        if (f(m) < target) == rising {
            a = m;
        } else {
            b = m;
        }
    }
    Some(0.5 * (a + b))
}

/// The keyboard circuit's output for key `k` (0 the lowest F), settled.
fn key_volts(kb: &Keyboard, k: usize) -> f64 {
    kb.static_out(k)
        .unwrap_or(k as f64 * crate::tuning::KEY_STEP)
}

/// Folkman's filter procedure on the real-time filter at `rate` Hz.
pub fn folkman(rate: f64) -> FilterTrims {
    // From the drawn values' middle: R39 and R73 centred, R49 at half its 500 ohm.
    let mut t = FilterTrims {
        r39: 0.5,
        r49: 250.0,
        r73_pos: 0.5,
    };
    // "Regeneration Cal.": mixer switches off, CUTOFF at -1 (0.4), EMPHASIS at 7.5; R73
    // turned until regeneration starts.
    let ins = inputs(0.4, false, false, 0.0);
    if let Some(p) = bisect(
        0.0,
        1.0,
        20,
        |p| {
            if regenerates(rate, &FilterTrims { r73_pos: p, ..t }, &ins, 0.75) {
                1.0
            } else {
                0.0
            }
        },
        0.5,
    ) {
        t.r73_pos = p;
    }
    // "Filter Range": KEYBOARD CONTROL off, CUTOFF at -1, EMPHASIS at 10; R39 for zero beats
    // with the A-440.
    if let Some(r) = bisect(
        0.0,
        1.0,
        30,
        |r| oscillation(rate, &FilterTrims { r39: r, ..t }, &ins),
        440.0,
    ) {
        t.r39 = r;
    }
    // "Filter Scale": KEYBOARD CONTROL 1 and 2 on; the third A from the bottom tuned to
    // 1760 Hz with CUTOFF, the low A to 440 Hz with R49, "repeated until the filter will
    // track". Alternating the two as written diverges on the model (R49 swings wider each
    // round); the state it aims at is solved directly: R49 such that, with CUTOFF tuning the
    // low A to 440 Hz, the third A sounds 1760 Hz (the two octaves depend on R49 alone).
    let kb = Keyboard::new(KeyboardCircuit::default(), rate);
    let (low, third) = (key_volts(&kb, 4), key_volts(&kb, 28));
    // (CUTOFF's search stays below its top, where the oscillation can pass Nyquist.)
    let tuned = |t: &FilterTrims| {
        bisect(
            0.05,
            0.85,
            26,
            |c| oscillation(rate, t, &inputs(c, true, true, low)),
            440.0,
        )
    };
    let third_at = |r49: f64| {
        let tt = FilterTrims { r49, ..t };
        tuned(&tt).map_or(0.0, |c| {
            oscillation(rate, &tt, &inputs(c, true, true, third))
        })
    };
    if let Some(r) = bisect(0.0, 500.0, 20, third_at, 1760.0) {
        t.r49 = r;
    }
    t
}
