//! One oscillator of board 1 in real time: the exponential converter ([`crate::expo`]),
//! the sawtooth core with its reset, and the wave shapers (docs/circuit/board1.md).
//!
//! How each part is derived:
//! - **Timing current**: [`ExpoCircuit`] solves the converter's circuit equations at the
//!   ramp's two ends once per output sample; the current is linear in the ramp's voltage
//!   (the Early effect, amplified slightly by R42's feedback), so the ramp integrates
//!   exactly between events.
//! - **Core**: C1 plus the ramp node's junction capacitances integrates the current; when
//!   the ramp falls through the Schmitt trigger's threshold the reset follows after the
//!   comparator's delay (it grows as the ramp slows), the ramp jumps to its top (which
//!   falls slightly as the ramp speeds up) and holds while Q10 releases. These constants
//!   are extracted from ngspice runs of the transcribed circuit ([`CoreParams::default`],
//!   `scripts/fit_core.py`); they explain ngspice's period within 0.11 cent from
//!   0.7 Hz to 4.2 kHz.
//! - **Shapers**: the buffer (Q7-Q3) is linear; the sawtooth is its divided output; the
//!   triangle is the DC transfer of Q2/Q1 tabulated from ngspice ([`crate::tables::TRI`]);
//!   the rectangle is Q11/Q12's comparator with its hysteresis, thresholds from ngspice's
//!   DC sweeps.
//! - **Band-limiting**: the core runs at `oversample` times the output rate; the jumps at
//!   the reset and the rectangle's edges get band-limited steps at their exact times; a
//!   halfband decimator brings each output down. The triangle's fold is sampled at the
//!   oversampled rate without correction.
//!
//! Outputs are the board's open-circuit voltages (unloaded): the mixer's loading belongs
//! to the mixer's model. The reset's sub-microsecond shape is represented by what it does
//! below 20 kHz: its timing, and the areas it adds to the triangle (the glitch as the
//! buffer sweeps through Q2's fold) and to the sawtooth (the buffer's lag), measured on
//! the reference. Left out: the core's temperature dependence (its thresholds are 25 C's)
//! and noise.

use crate::expo::ExpoCircuit;
use crate::resample::{BlepLine, Decimator};
use crate::tables;

/// The core's reset constants (oscillator 1, 25 C).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreParams {
    /// C1, the timing capacitor (0.01 uF polystyrene), F.
    pub c1: f64,
    /// Capacitance the ramp node adds (Q10's collector junction, Q7's gate, IC2's
    /// collector and substrate junctions), F.
    pub c_par: f64,
    /// Leakage out of the ramp node, A (in the reference, ngspice's gmin across junctions).
    pub i_leak: f64,
    /// The ramp voltage at which the Schmitt trigger fires, for a slow ramp, V.
    pub v_threshold: f64,
    /// The ramp's top after a reset, and how much it falls per V/s of ramp slope.
    pub v_top0: f64,
    pub top_per_slope: f64,
    /// How long the ramp is held at its top while Q10 releases, s.
    pub t_hold: f64,
    /// The comparator's delay after the threshold, against ln(slope in V/s).
    pub delay_ln_slope: [f64; 8],
    pub delay: [f64; 8],
}

impl Default for CoreParams {
    /// Extracted from `circuits/boards/reference/vco1-core.json` by
    /// `scripts/fit_core.py` (ngspice-47, `mm-devices.lib` revision 1).
    fn default() -> Self {
        CoreParams {
            c1: 0.01e-6,
            c_par: 1.594822e-11,
            i_leak: 1.246916e-11,
            v_threshold: -3.970812,
            v_top0: 0.012771,
            top_per_slope: 2.824424e-07,
            t_hold: 1.203511e-06,
            delay_ln_slope: [
                4.4359, 5.18819, 5.94048, 6.69278, 7.44507, 8.19736, 8.94966, 9.70195,
            ],
            delay: [
                8.581449e-06,
                6.858305e-06,
                5.389057e-06,
                4.194429e-06,
                3.246414e-06,
                2.513420e-06,
                1.954638e-06,
                1.532251e-06,
            ],
        }
    }
}

impl CoreParams {
    fn delay_at(&self, slope: f64) -> f64 {
        let x = crate::ulp::log(slope.max(1e-9));
        let g = &self.delay_ln_slope;
        if x <= g[0] {
            return self.delay[0];
        }
        for i in 1..g.len() {
            if x <= g[i] {
                let a = (x - g[i - 1]) / (g[i] - g[i - 1]);
                return self.delay[i - 1] + (self.delay[i] - self.delay[i - 1]) * a;
            }
        }
        self.delay[g.len() - 1]
    }
}

/// The wave shapers' constants (oscillator 1, unloaded, 25 C).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaperParams {
    /// The buffer: Vbuf = gain Vramp + offset (R24, R25, R4; ngspice DC sweep).
    pub buf_gain: f64,
    pub buf_offset: f64,
    /// The sawtooth output's share of the buffer: R33 / (R33 + R34).
    pub saw_ratio: f64,
    /// The rectangle's comparator: the buffer voltages at which it switches low (falling)
    /// and high (rising) for a width bias of 0 V, and how they move per volt of bias.
    pub rect_fall0: f64,
    pub rect_rise0: f64,
    pub rect_per_width: f64,
    /// The rectangle's two levels: Q12 off (0 V through R22) and on (R22/(R13+R22) of
    /// -10 V less Q12's saturation).
    pub rect_high: f64,
    pub rect_low: f64,
    /// Its edges' timing: it rises this long after the ramp's reset (Q12 leaving
    /// saturation), and falls this long after the ramp has passed the DC threshold by
    /// `rect_fall_overdrive` volts (Q11 and Q12 switching), s and V.
    pub rect_rise_delay: f64,
    pub rect_fall_delay: f64,
    pub rect_fall_overdrive: f64,
    /// What the reset adds beyond an ideal step at its instant, as areas (V s): the
    /// triangle's glitch while the buffer sweeps through Q2's fold, and the buffer's lag.
    pub tri_glitch: f64,
    pub buf_lag: f64,
}

impl Default for ShaperParams {
    fn default() -> Self {
        ShaperParams {
            buf_gain: 2.0346,
            buf_offset: 3.9098,
            saw_ratio: 4.7 / 9.0,
            rect_fall0: -0.0371,
            rect_rise0: 0.3738,
            rect_per_width: -0.9983,
            rect_high: 0.0,
            rect_low: -4.2954,
            rect_rise_delay: 2.74e-6,
            rect_fall_delay: 3.75e-6,
            rect_fall_overdrive: 4.94e-3,
            tri_glitch: -3.8e-7,
            buf_lag: -4.8e-7,
        }
    }
}

/// The triangle shaper's transfer at a buffer voltage (Catmull-Rom interpolation).
pub fn triangle(buf: f64) -> f64 {
    let t = &tables::TRI;
    let x = (buf - tables::TRI_MIN) / tables::TRI_STEP;
    let i = (x.floor() as isize).clamp(1, t.len() as isize - 3) as usize;
    let f = (x - i as f64).clamp(0.0, 1.0);
    let (p0, p1, p2, p3) = (t[i - 1], t[i], t[i + 1], t[i + 2]);
    p1 + 0.5
        * f
        * (p2 - p0 + f * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + f * (3.0 * (p1 - p2) + p3 - p0)))
}

/// The most timing current the converter's model gives, A: twice MIDI 127's at the panel's
/// centre (2.47 mA), an octave above the top key the plug-in plays, far past the audio band
/// on any range. Up to it the solve converges; above about 1e-2 A it runs away.
pub const TIMING_MAX: f64 = 5e-3;

/// The timing current at -4 V on the ramp over that at 0 V (the Early effect) at
/// [`TIMING_MAX`], as the converter's solve gives it there.
const EARLY_AT_MAX: f64 = 0.881;

/// One output sample of the oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VcoOut {
    pub saw: f64,
    pub tri: f64,
    pub rect: f64,
}

/// An oscillator of board 1.
#[derive(Debug, Clone)]
pub struct Vco {
    pub expo: ExpoCircuit,
    pub core: CoreParams,
    pub shaper: ShaperParams,
    /// Circuit temperature, C (the converter follows it; the core's constants are 25 C's).
    pub celsius: f64,
    oversample: usize,
    dt: f64,
    /// Another oversampling, its decimators prepared ([`Vco::prepare`], [`Vco::set_oversample`]).
    other: usize,
    dec_other: [Decimator; 3],
    /// Potato's converter ([`ConverterTable`], made by [`Vco::prepare`]) and whether it is
    /// in use.
    table: Option<std::sync::Arc<crate::expo::ConverterTable>>,
    pub table_on: bool,
    // State.
    v: f64,
    hold_left: f64,
    reset_in: Option<f64>,
    rise_in: Option<f64>,
    fall_in: Option<f64>,
    rect_is_low: bool,
    guess: Option<f64>,
    saw: BlepLine,
    tri: BlepLine,
    rect: BlepLine,
    dec: [Decimator; 3],
    /// Output samples since the last `reset`, and the time of the latest ramp reset in
    /// output samples (fractional), with how many there have been.
    ticks: u64,
    last_reset: f64,
    resets: u64,
    sub: usize,
    /// The converter's last two solves with their exact arguments (the drive, the warm
    /// start, the circuit, the temperature): the same arguments give the same currents, so
    /// a steady drive, whose solves settle into a fixed point or a two-cycle, is not solved
    /// again (exactly; performance).
    solved: [Option<Solved>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Solved {
    i_in: u64,
    guess: Option<u64>,
    celsius: u64,
    expo: ExpoCircuit,
    top: f64,
    bottom: f64,
}

impl Vco {
    /// An oscillator at `sample_rate`, running its core at `oversample` (1, 2, 4 or 8)
    /// times that rate.
    pub fn new(sample_rate: f64, oversample: usize) -> Vco {
        let oversample = match oversample {
            0 | 1 => 1,
            2 | 3 => 2,
            4..=7 => 4,
            _ => 8,
        };
        Vco {
            expo: ExpoCircuit::default(),
            core: CoreParams::default(),
            shaper: ShaperParams::default(),
            celsius: 25.0,
            oversample,
            dt: 1.0 / (sample_rate * oversample as f64),
            other: oversample,
            table: None,
            table_on: false,
            dec_other: [
                Decimator::new(oversample),
                Decimator::new(oversample),
                Decimator::new(oversample),
            ],
            v: CoreParams::default().v_top0,
            hold_left: 0.0,
            reset_in: None,
            rise_in: None,
            fall_in: None,
            rect_is_low: false,
            guess: None,
            saw: BlepLine::default(),
            tri: BlepLine::default(),
            rect: BlepLine::default(),
            dec: [
                Decimator::new(oversample),
                Decimator::new(oversample),
                Decimator::new(oversample),
            ],
            ticks: 0,
            last_reset: 0.0,
            resets: 0,
            sub: 0,
            solved: [None; 2],
        }
    }

    /// Prepares Potato: a second oversampling factor (1, 2, 4 or 8) to switch to while it
    /// plays ([`Vco::set_oversample`]) and the converter's table for its trims as they are
    /// ([`crate::expo::ConverterTable`]; not on the audio thread: both are made here).
    pub fn prepare(&mut self, oversample: usize) {
        let d = || Decimator::new(oversample);
        self.dec_other = [d(), d(), d()];
        self.other = oversample;
        // (One a process for each converter's circuit and temperature.)
        use crate::expo::{ConverterTable, ExpoCircuit};
        static TABLES: crate::shared::Shared<(ExpoCircuit, u64), ConverterTable> =
            crate::shared::Shared::new();
        let (expo, celsius) = (self.expo, self.celsius);
        self.table = Some(TABLES.get((expo, celsius.to_bits()), || {
            ConverterTable::new(&expo, celsius)
        }));
    }

    /// Runs at this oversampling from the next sample, if it is this one's or the prepared
    /// one's (otherwise nothing changes): the ramp and its events carry on; the decimators
    /// and the band-limited steps start again from silence (Potato).
    pub fn set_oversample(&mut self, oversample: usize) {
        if oversample == self.oversample || oversample != self.other {
            return;
        }
        let rate_dt = self.dt * self.oversample as f64;
        std::mem::swap(&mut self.dec, &mut self.dec_other);
        self.other = self.oversample;
        self.oversample = oversample;
        self.dt = rate_dt / oversample as f64;
        self.dec.iter_mut().for_each(Decimator::reset);
        self.saw = BlepLine::default();
        self.tri = BlepLine::default();
        self.rect = BlepLine::default();
        self.sub = 0;
    }

    /// The latest ramp reset's time in output samples since [`Vco::reset`] (exact, before
    /// the output's latency), and how many resets there have been.
    pub fn last_reset(&self) -> (f64, u64) {
        (self.last_reset, self.resets)
    }

    /// The outputs' delay through band-limiting and decimation, in output samples.
    pub fn latency(&self) -> f64 {
        self.dec[0].delay() + 1.0 / self.oversample as f64
    }

    /// Starts again from the top of the ramp.
    pub fn reset(&mut self) {
        self.v = self.core.v_top0;
        self.hold_left = 0.0;
        self.reset_in = None;
        self.rise_in = None;
        self.fall_in = None;
        self.rect_is_low = false;
        self.guess = None;
        self.saw = BlepLine::default();
        self.tri = BlepLine::default();
        self.rect = BlepLine::default();
        self.dec.iter_mut().for_each(Decimator::reset);
        self.ticks = 0;
        self.last_reset = 0.0;
        self.resets = 0;
    }

    /// The ramp's voltage now.
    pub fn ramp(&self) -> f64 {
        self.v
    }

    /// One output sample, for `i_in`, the current the control inputs send into the
    /// summing junction (see [`ExpoCircuit::input_current`]), and the rectangle's width
    /// bias in volts (0, -1.5 or -2.5 from the waveform switch).
    pub fn tick(&mut self, i_in: f64, width: f64) -> VcoOut {
        // The timing current is linear in the ramp voltage (the Early effect, amplified a
        // little by R42's feedback): I(v) = i0 + b v, from the converter at 0 V and -4 V.
        let key = (
            i_in.to_bits(),
            self.guess.map(f64::to_bits),
            self.celsius.to_bits(),
        );
        // (Potato: from the converter's table, where it has the drive; the solves' cache
        // looked in only without it.)
        let from_table = if self.table_on {
            self.table.as_ref().and_then(|t| {
                t.lookup(
                    &self.expo,
                    self.celsius,
                    self.expo.drive_u(i_in, self.celsius),
                )
            })
        } else {
            None
        };
        let hit = || {
            self.solved
                .iter()
                .flatten()
                .find(|s| (s.i_in, s.guess, s.celsius) == key && s.expo == self.expo)
                .map(|s| (s.top, s.bottom))
        };
        let (top, bottom) = match from_table.or_else(hit) {
            Some(x) => x,
            None => {
                let top = self.expo.solve_current(i_in, 0.0, self.celsius, self.guess);
                let bottom = self
                    .expo
                    .solve_current(i_in, -4.0, self.celsius, Some(top.ic_exp));
                self.solved = [
                    Some(Solved {
                        i_in: key.0,
                        guess: key.1,
                        celsius: key.2,
                        expo: self.expo,
                        top: top.ic_exp,
                        bottom: bottom.ic_exp,
                    }),
                    self.solved[0],
                ];
                (top.ic_exp, bottom.ic_exp)
            }
        };
        // Far above the instrument's range (keys beyond its 44, the plug-in's extension, with
        // RANGE, FREQUENCY and the PITCH wheel high) the converter's model leaves its ground:
        // above about 1e-2 A its tail node runs past any rail and the solve diverges. The
        // timing current is held at [`TIMING_MAX`] there, the ramp's slope as the converter
        // gives it at that current; below it, as solved.
        let (top, bottom) = if top.is_finite() && bottom.is_finite() && top <= TIMING_MAX {
            (top, bottom)
        } else {
            (TIMING_MAX, TIMING_MAX * EARLY_AT_MAX)
        };
        self.guess = Some(top);
        let b = (top - bottom) / 4.0;
        self.tick_current(top, b, width)
    }

    /// The last sample's timing current with the ramp at 0 V, A: the converter's output,
    /// which the oscillator's frequency follows (a probe of the pitch path).
    pub fn timing_current(&self) -> f64 {
        self.guess.unwrap_or(0.0)
    }

    /// One output sample for a timing current given directly: `i0` at a ramp of 0 V and its
    /// slope `b` per volt (the Early effect). [`Vco::tick`] computes them from the inputs.
    pub fn tick_current(&mut self, i0: f64, b: f64, width: f64) -> VcoOut {
        let a = i0 - self.core.i_leak;
        let c = self.core.c1 + self.core.c_par;
        let sh = self.shaper;
        let fall_buf = sh.rect_fall0 + sh.rect_per_width * width;
        let rise_buf = sh.rect_rise0 + sh.rect_per_width * width;
        let v_fall = (fall_buf - sh.buf_offset) / sh.buf_gain - sh.rect_fall_overdrive;
        let mut out = VcoOut::default();
        for k in 0..self.oversample {
            self.sub = k;
            self.step(a, b, c, v_fall, rise_buf);
            let buf = sh.buf_gain * self.v + sh.buf_offset;
            let level = if self.rect_is_low {
                sh.rect_low
            } else {
                sh.rect_high
            };
            let s = self.saw.push(sh.saw_ratio * buf);
            let t = self.tri.push(triangle(buf));
            let r = self.rect.push(level);
            if let Some(v) = self.dec[0].push(s) {
                out.saw = v;
            }
            if let Some(v) = self.dec[1].push(t) {
                out.tri = v;
            }
            if let Some(v) = self.dec[2].push(r) {
                out.rect = v;
            }
        }
        self.ticks += 1;
        out
    }

    /// One oversampled step: the ramp (C dv/dt = -(a + b v)) and its events, taken in
    /// time order: the threshold, the reset after the comparator's delay, the end of the
    /// hold, the ramp passing the rectangle's threshold, and the rectangle's delayed edges.
    fn step(&mut self, a: f64, b: f64, c: f64, v_fall: f64, rise_buf: f64) {
        let dt = self.dt;
        let mut t = 0.0;
        // v(t) = -A + (v0 + A) e^(-b t / C), A = a / b.
        let big_a = a / b;
        let advance = |v: f64, h: f64| -big_a + (v + big_a) * crate::ulp::exp(-b * h / c);
        let cross = |v0: f64, v1: f64| (c / b) * crate::ulp::log((v0 + big_a) / (v1 + big_a));
        let th = self.core.v_threshold;
        loop {
            let left = dt - t;
            let holding = self.hold_left > 0.0;
            let v_end = if holding {
                self.v
            } else {
                advance(self.v, left)
            };
            // The earliest event in what is left of the step.
            let mut next = left;
            let mut which = 0u8;
            let mut take = |at: f64, w: u8| {
                if at < next {
                    next = at;
                    which = w;
                }
            };
            if holding {
                take(self.hold_left, 1);
            }
            if let Some(r) = self.reset_in {
                take(r, 2);
            }
            if let Some(r) = self.rise_in {
                take(r, 3);
            }
            if let Some(f) = self.fall_in {
                take(f, 4);
            }
            if !holding && self.reset_in.is_none() && self.v > th && v_end <= th {
                take(cross(self.v, th).clamp(0.0, left), 5);
            }
            if !holding
                && !self.rect_is_low
                && self.fall_in.is_none()
                && self.v > v_fall
                && v_end <= v_fall
            {
                take(cross(self.v, v_fall).clamp(0.0, left), 6);
            }
            // Move everything to it.
            if !holding {
                self.v = advance(self.v, next);
            }
            self.hold_left = (self.hold_left - next).max(0.0);
            for x in [&mut self.reset_in, &mut self.rise_in, &mut self.fall_in]
                .into_iter()
                .flatten()
            {
                *x -= next;
            }
            t += next;
            let frac = (t / dt).clamp(1e-9, 1.0);
            let sh = self.shaper;
            match which {
                2 => {
                    self.reset_in = None;
                    self.do_reset(a, b, c, frac, rise_buf);
                }
                3 => {
                    self.rise_in = None;
                    if self.rect_is_low {
                        self.rect_is_low = false;
                        self.rect.step(sh.rect_high - sh.rect_low, frac);
                    }
                }
                4 => {
                    self.fall_in = None;
                    if !self.rect_is_low {
                        self.rect_is_low = true;
                        self.rect.step(sh.rect_low - sh.rect_high, frac);
                    }
                }
                5 => {
                    self.v = th;
                    let slope = (a + b * th) / c;
                    self.reset_in = Some(self.core.delay_at(slope));
                }
                6 => {
                    self.v = v_fall;
                    self.fall_in = Some(sh.rect_fall_delay);
                }
                _ => {}
            }
            if which == 0 {
                break;
            }
        }
    }

    fn do_reset(&mut self, a: f64, b: f64, c: f64, frac: f64, rise_buf: f64) {
        let sh = self.shaper;
        let slope = (a + b * self.v) / c;
        let v_top = self.core.v_top0 - self.core.top_per_slope * slope;
        let buf0 = sh.buf_gain * self.v + sh.buf_offset;
        let buf1 = sh.buf_gain * v_top + sh.buf_offset;
        self.saw.step(sh.saw_ratio * (buf1 - buf0), frac);
        self.saw.impulse(sh.saw_ratio * sh.buf_lag / self.dt, frac);
        self.tri.step(triangle(buf1) - triangle(buf0), frac);
        self.tri.impulse(sh.tri_glitch / self.dt, frac);
        if self.rect_is_low && buf1 >= rise_buf {
            self.rise_in = Some(sh.rect_rise_delay);
        }
        // A fall still pending when the ramp resets is overtaken by the reset.
        self.fall_in = None;
        self.v = v_top;
        self.hold_left = self.core.t_hold;
        self.last_reset =
            self.ticks as f64 + (self.sub as f64 + frac - 1.0) / self.oversample as f64;
        self.resets += 1;
    }
}
