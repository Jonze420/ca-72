//! The plug-in's character (decisions.md, "ANALOG and SPREAD"; the same as that of the DAW's
//! instrument built on the same model):
//! ENTROPY, each voice its own parts and its oscillators' and cutoff's slow movements, and
//! SPREAD, where a voice sits in the stereo field. Neither is the instrument's; at 0 neither
//! does anything (the circuit as drawn).

/// ENTROPY's figures at 100 %: the standard deviations of a voice's own tolerances, and of
/// its oscillators' movements.
pub mod entropy {
    /// Each oscillator's tuning off its calibration, cents.
    pub const CENTS: f64 = 4.0;
    /// Each oscillator's slow drift, cents, wandering with this time constant, s.
    pub const DRIFT_CENTS: f64 = 3.0;
    pub const DRIFT_TAU: f64 = 4.0;
    /// Each oscillator's waver: noise under this frequency (Hz), cents.
    pub const WAVER_CENTS: f64 = 0.4;
    pub const WAVER_HZ: f64 = 30.0;
    /// The cutoff off its calibration, and its drift, octaves.
    pub const CUTOFF: f64 = 0.04;
    pub const CUTOFF_DRIFT: f64 = 0.015;
    /// A pot's (or the parts' behind it) offset, of its rotation (about 5 % of a time on
    /// an audio-taper pot).
    pub const KNOB: f64 = 0.012;
    /// Samples between drift steps (the drift moves over seconds).
    pub const STEP: u32 = 64;
}

/// The most oscillators and knobs a voice's character covers.
const OSCILLATORS: usize = 3;
const KNOBS: usize = 10;

/// Where voice `k` sits at full SPREAD: the first in the centre, then out to either side in
/// turn, a chord of any size spread evenly-ish (the quarters, then an eighth).
const PANS: [f64; 10] = [0.0, -1.0, 1.0, -0.5, 0.5, -0.75, 0.75, -0.25, 0.25, -0.125];

/// A voice's character: its own tolerances, drawn once from its seed, its oscillators' and
/// cutoff's movements, and where it sits in the stereo field.
#[derive(Debug, Clone)]
pub struct Character {
    /// How many oscillators and knobs the instrument has.
    oscillators: usize,
    knobs: usize,
    /// Per unit of ENTROPY (standard normal draws): each oscillator's tuning, the cutoff,
    /// and the knobs (each instrument's own list).
    tune: [f64; OSCILLATORS],
    cutoff: f64,
    knob: [f64; KNOBS],
    /// The drifts (standard normal, moving: the oscillators', then the cutoff's), the
    /// wavers (in cents at full ENTROPY), the noise generator, samples to the next drift
    /// step.
    drift: [f64; OSCILLATORS + 1],
    waver: [f64; OSCILLATORS],
    rng: u64,
    count: u32,
    /// Where the voice sits at full SPREAD, -1 (left) to 1 (right).
    pan: f64,
    /// The rate the movements' constants below are for, and they: the drift's step and its
    /// noise's share, the waver's pole and its noise's scale (worked out once a rate, not
    /// each sample: the same values).
    rate: f64,
    drift_ab: (f64, f64),
    waver_ws: (f64, f64),
}

impl Character {
    /// Voice `voice`'s character from `seed`, for an instrument of `oscillators` (at most
    /// 3) and `knobs` (at most 10) tolerances. The draws are made in a fixed order (the
    /// tunings, the cutoff, the knobs, the drifts' starts), so an instrument's voices are
    /// the same whenever they are made from the same seeds.
    pub fn new(seed: u64, voice: usize, oscillators: usize, knobs: usize) -> Character {
        let oscillators = oscillators.min(OSCILLATORS);
        let knobs = knobs.min(KNOBS);
        let mut c = Character {
            oscillators,
            knobs,
            tune: [0.0; OSCILLATORS],
            cutoff: 0.0,
            knob: [0.0; KNOBS],
            drift: [0.0; OSCILLATORS + 1],
            waver: [0.0; OSCILLATORS],
            // (Never zero, as xorshift needs.)
            rng: (seed ^ 0x5EED_0FA1_A106_0000) | 1,
            count: 0,
            pan: PANS[voice % PANS.len()],
            rate: f64::NAN,
            drift_ab: (0.0, 0.0),
            waver_ws: (0.0, 0.0),
        };
        for k in 0..oscillators {
            c.tune[k] = c.normal();
        }
        c.cutoff = c.normal();
        for k in 0..knobs {
            c.knob[k] = c.normal();
        }
        for k in 0..=oscillators {
            c.drift[k] = c.normal();
        }
        c
    }

    /// The next of the noise generator's numbers, uniform in -0.5..0.5 (xorshift64*).
    fn uniform(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let x = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (x >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    /// A standard normal draw (Box-Muller).
    fn normal(&mut self) -> f64 {
        let a = self.uniform() + 0.5;
        let b = self.uniform() + 0.5;
        libm::sqrt(-2.0 * libm::log(a.max(1e-300))) * libm::cos(std::f64::consts::TAU * b)
    }

    /// Knob `k`'s rotation `x` (0..1) with this voice's tolerance, by `amount` (0..1) of
    /// ENTROPY.
    pub fn knob(&self, k: usize, x: f64, amount: f64) -> f64 {
        (x + amount * entropy::KNOB * self.knob[k.min(self.knobs.max(1) - 1)]).clamp(0.0, 1.0)
    }

    /// One output sample's offsets, by `amount` (0..1) of ENTROPY, at `rate` Hz: each
    /// oscillator's off its calibration, cents (its tolerance, drift and waver), and the
    /// cutoff's, octaves (its tolerance and drift).
    pub fn offsets(&mut self, amount: f64, rate: f64) -> ([f64; OSCILLATORS], f64) {
        if rate.to_bits() != self.rate.to_bits() {
            self.rate = rate;
            let a = libm::exp(-f64::from(entropy::STEP) / (rate * entropy::DRIFT_TAU));
            self.drift_ab = (a, libm::sqrt(1.0 - a * a));
            let w = libm::exp(-std::f64::consts::TAU * entropy::WAVER_HZ / rate);
            self.waver_ws = (
                w,
                entropy::WAVER_CENTS * libm::sqrt(12.0 * (1.0 + w) / (1.0 - w)),
            );
        }
        if self.count == 0 {
            // The drifts' next step: each a mean-reverting random walk, its spread one.
            self.count = entropy::STEP;
            let (a, b) = self.drift_ab;
            for k in 0..=self.oscillators {
                // (Four uniforms: near enough normal, at a quarter of the cost.)
                let g = (self.uniform() + self.uniform() + self.uniform() + self.uniform())
                    * 3f64.sqrt();
                self.drift[k] = a * self.drift[k] + b * g;
            }
        }
        self.count -= 1;
        // The waver: white noise through a one-pole low-pass, scaled to its spread.
        let (w, scale) = self.waver_ws;
        for k in 0..self.oscillators {
            let x = self.uniform() * scale;
            self.waver[k] = w * self.waver[k] + (1.0 - w) * x;
        }
        let mut cents = [0.0; OSCILLATORS];
        for (k, c) in cents.iter_mut().enumerate().take(self.oscillators) {
            *c = amount
                * (entropy::CENTS * self.tune[k]
                    + entropy::DRIFT_CENTS * self.drift[k]
                    + self.waver[k]);
        }
        let d = self.drift[self.oscillators];
        (
            cents,
            amount * (entropy::CUTOFF * self.cutoff + entropy::CUTOFF_DRIFT * d),
        )
    }

    /// The gains (left, right) that put the voice in its place, by `spread` (0..1) of
    /// SPREAD, by a pan law that keeps both channels whole at the centre (as at 0).
    pub fn gains(&self, spread: f64) -> (f32, f32) {
        if spread > 0.0 {
            pan_gains(0.5 + 0.5 * spread * self.pan)
        } else {
            (1.0, 1.0)
        }
    }
}

/// The pan law: at `pan` (0 left, 0.5 centre, 1 right) each side's gain, both whole at the
/// centre (the DAW's instrument's, so the two sound alike).
pub fn pan_gains(pan: f64) -> (f32, f32) {
    let p = pan.clamp(0.0, 1.0);
    ((2.0 * (1.0 - p)).min(1.0) as f32, (2.0 * p).min(1.0) as f32)
}
