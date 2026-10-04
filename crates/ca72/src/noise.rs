//! Board 3's noise generator in real time (circuit No. 3, `board3-noise.lib`;
//! docs/circuit/board3.md): the circuit's small-signal model at its operating point
//! ([`crate::linear`]), driven by a Gaussian white source standing for Q15's avalanche
//! noise, discretised at four times the output rate and decimated.
//!
//! The source's level is not in the circuit: the factory set R26 (NOISE LEVEL) for the white
//! noise's level at the output, and the voice sets the source's density the same way
//! ([`Noise::set_density`]). The circuit's own filtering (C11, the transistors' Miller
//! capacitance, the pink and red networks, the output couplings into their loads) is
//! derived; the generator's randomness is seeded, so a render repeats.

use crate::contour::Q2N3392;
use crate::devices::Bjt;
use crate::linear::{Discrete, SmallSignal};
use crate::mna::{Circuit, GND, NoConvergence, Node, Part};
use crate::resample::Decimator;

/// The circuit's trim and the noise source's stand-in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseCircuit {
    /// R26, NOISE LEVEL: ohm in circuit (a 2.5K rheostat), at mid-travel.
    pub r26: f64,
    /// Q15's emitter-base junction in breakdown: its voltage and small-signal resistance
    /// (not documented; its only effect is C12's coupling corner, below 1 Hz).
    pub vbr: f64,
    pub rd: f64,
}

impl Default for NoiseCircuit {
    fn default() -> Self {
        NoiseCircuit {
            r26: 1250.0,
            vbr: 7.5,
            rd: 1e3,
        }
    }
}

/// The outputs' loads to ground, ohm (`f64::INFINITY`: open): the NOISE switch's paths to
/// the mixer and to the modulation mix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseLoads {
    pub white: f64,
    pub pink: f64,
    pub red: f64,
}

/// One sample of the three outputs (pins 5, 6 and 2, into their loads), V.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NoiseOut {
    pub white: f64,
    pub pink: f64,
    pub red: f64,
}

const P10: Node = 1;
const N10: Node = 2;
/// Q15's junction as a source (held): the breakdown voltage plus the noise.
const SRC: Node = 3;
const HELD: usize = 4;
const NX: Node = 4;
const E15: Node = 5;
const B12: Node = 6;
const C12N: Node = 7;
const E12: Node = 8;
const C4: Node = 9;
const E4: Node = 10;
const WHITE: Node = 11;
const P1: Node = 12;
const N2: Node = 13;
const B3: Node = 14;
const Y3: Node = 15;
const C3N: Node = 16;
const E3: Node = 17;
const PINK: Node = 18;
const R1N: Node = 19;
const B6: Node = 20;
const Z6: Node = 21;
const C6N: Node = 22;
const E6: Node = 23;
const RED: Node = 24;
/// C3 and R8's junction.
const N3: Node = 25;
const NODES: usize = 26;

/// Oversampling: the white noise reaches past 20 kHz (C11's pole is near 16 kHz), where
/// the trapezoidal rule's warping at the output rate would bend it.
const OVERSAMPLE: usize = 4;

/// The circuit for the nodal solver, at its operating point.
pub fn circuit(k: &NoiseCircuit, loads: &NoiseLoads) -> Result<Circuit, NoConvergence> {
    circuit_with(k, loads, &Q2N3392)
}

/// [`circuit`] with its transistors as `q` (Potato's without their series resistances: the
/// emitter's then added to each emitter resistor, where it sets the stage's gain).
fn circuit_with(k: &NoiseCircuit, loads: &NoiseLoads, q: &Bjt) -> Result<Circuit, NoConvergence> {
    let re = Q2N3392.re - q.re;
    let mut c = Circuit::new(NODES, HELD);
    c.set(P10, 10.0);
    c.set(N10, -10.0);
    c.set(SRC, -10.0 + k.vbr);
    for (n, v) in [
        (NX, 8.3),
        (E15, -2.5),
        (B12, -9.3),
        (C12N, -8.6),
        (E12, -9.99),
        (C4, 9.9),
        (E4, -9.3),
        (P1, -9.3),
        (N3, -10.0),
        (N2, -10.0),
        (B3, -9.3),
        (Y3, -8.0),
        (C3N, -2.0),
        (E3, -9.9),
        (R1N, -2.0),
        (B6, -9.3),
        (Z6, -8.0),
        (C6N, -2.0),
        (E6, -9.9),
    ] {
        c.set(n, v);
    }
    let r = |c: &mut Circuit, a, b, r: f64| {
        c.add(Part::Resistor { a, b, r });
    };
    let cap = |c: &mut Circuit, a, b, v: f64| {
        c.add(Part::Capacitor { a, b, c: v });
    };
    // The noise source.
    r(&mut c, P10, NX, 75e3);
    cap(&mut c, NX, N10, 10e-6);
    r(&mut c, NX, E15, 470e3);
    r(&mut c, SRC, E15, k.rd);
    // Q12, Q4: white.
    cap(&mut c, E15, B12, 5.6e-6);
    r(&mut c, B12, C12N, 4.7e6);
    r(&mut c, P10, C12N, 100e3);
    cap(&mut c, C12N, N10, 100e-12);
    c.add_bjt(C12N, B12, E12, q, 25.0, false);
    r(&mut c, E12, N10, k.r26.max(1e-3) + re);
    c.add_bjt(C4, C12N, E4, q, 25.0, false);
    r(&mut c, P10, C4, 470.0);
    r(&mut c, E4, N10, 3.3e3 + re);
    cap(&mut c, C12N, E4, 100e-12);
    cap(&mut c, E4, WHITE, 0.15e-6);
    r(&mut c, WHITE, GND, open(loads.white));
    // The pink filter (R16 into two series RC branches, C3-R8 and C2-R13), Q3.
    r(&mut c, E4, P1, 10e3);
    cap(&mut c, P1, N3, 0.12e-6);
    r(&mut c, N3, N10, 3.3e3);
    cap(&mut c, P1, N2, 0.033e-6);
    r(&mut c, N2, N10, 220.0);
    cap(&mut c, P1, B3, 2.5e-6);
    r(&mut c, B3, Y3, 560e3);
    r(&mut c, Y3, C3N, 560e3);
    cap(&mut c, Y3, N10, 2.5e-6);
    c.add_bjt(C3N, B3, E3, q, 25.0, false);
    r(&mut c, P10, C3N, 3.3e3);
    r(&mut c, E3, N10, 120.0 + re);
    cap(&mut c, C3N, PINK, 2.5e-6);
    r(&mut c, PINK, GND, open(loads.pink));
    // The red filter, Q6.
    r(&mut c, C3N, R1N, 10e3);
    cap(&mut c, R1N, N10, 0.15e-6);
    cap(&mut c, R1N, B6, 2.5e-6);
    r(&mut c, B6, Z6, 2.2e6);
    r(&mut c, Z6, C6N, 2.2e6);
    cap(&mut c, Z6, N10, 0.1e-6);
    c.add_bjt(C6N, B6, E6, q, 25.0, false);
    r(&mut c, P10, C6N, 10e3);
    r(&mut c, E6, N10, 3.9e3 + re);
    cap(&mut c, C6N, N10, 0.15e-6);
    cap(&mut c, C6N, RED, 10e-6);
    r(&mut c, RED, GND, 100e3);
    r(&mut c, RED, GND, open(loads.red));
    c.dc()?;
    Ok(c)
}

/// The noise generator.
#[derive(Debug, Clone)]
pub struct Noise {
    pub circuit: NoiseCircuit,
    loads: NoiseLoads,
    rate: f64,
    model: SmallSignal,
    sys: Discrete,
    out: [usize; 3],
    dec: [Decimator; 3],
    rng: Gauss,
    /// The other mode's model, its discretisation and outputs (the circuit's; Potato's,
    /// its transistors without RB, RE and RC: their internal nodes, 12 of 34, gone, and a
    /// step's work under half), and the pairs of the two models' indices of each of the
    /// circuit's solved nodes (full, plain).
    other: (SmallSignal, Discrete, [usize; 3]),
    plain: bool,
    pairs: Vec<(usize, usize)>,
    /// The source's standard deviation per oversampled step, V.
    sigma: f64,
    density: f64,
    pending: Option<NoiseLoads>,
    since_loads: usize,
    /// Steps a sample: [`OVERSAMPLE`], or one in Potato (its decimators then unused).
    os: usize,
}

/// The fewest output samples between two changes of the outputs' loads.
pub const LOAD_EVERY: usize = 64;

/// A load's resistance in the circuit: none as 1e12 ohm.
fn open(r: f64) -> f64 {
    if r.is_finite() { r } else { 1e12 }
}

impl Noise {
    /// The generator at `rate` Hz with its outputs so loaded, its randomness seeded.
    pub fn new(
        circuit: NoiseCircuit,
        loads: NoiseLoads,
        rate: f64,
        seed: u64,
    ) -> Result<Noise, NoConvergence> {
        let (model, out, full_nodes) = Self::model_with(&circuit, &loads, &Q2N3392)?;
        let sys = model.discrete(1.0 / (rate * OVERSAMPLE as f64));
        let plain_q = Bjt {
            rb: 0.0,
            re: 0.0,
            rc: 0.0,
            ..Q2N3392
        };
        let (plain, plain_out, plain_nodes) = Self::model_with(&circuit, &loads, &plain_q)?;
        let pairs = full_nodes
            .iter()
            .zip(&plain_nodes)
            .filter_map(|(a, b)| a.zip(*b))
            .collect();
        let plain_sys = plain.discrete(1.0 / rate);
        Ok(Noise {
            other: (plain, plain_sys, plain_out),
            plain: false,
            pairs,
            circuit,
            loads,
            rate,
            model,
            sys,
            out,
            dec: [
                Decimator::new(OVERSAMPLE),
                Decimator::new(OVERSAMPLE),
                Decimator::new(OVERSAMPLE),
            ],
            rng: Gauss::new(seed),
            sigma: 0.0,
            density: 0.0,
            pending: None,
            since_loads: LOAD_EVERY,
            os: OVERSAMPLE,
        })
    }

    /// The circuit's small-signal model with its transistors as `q`, its outputs' indices
    /// (white, pink, red), and each of the circuit's own solved nodes' index.
    #[allow(clippy::type_complexity)]
    fn model_with(
        k: &NoiseCircuit,
        loads: &NoiseLoads,
        q: &Bjt,
    ) -> Result<(SmallSignal, [usize; 3], Vec<Option<usize>>), NoConvergence> {
        let mut c = circuit_with(k, loads, q)?;
        let (g, cm, g_in) = c.small_signal(SRC);
        let n = g_in.len();
        let idx = |node| c.solved_index(node).unwrap_or(0);
        let nodes = (0..NODES).map(|node| c.solved_index(node)).collect();
        Ok((
            SmallSignal { n, g, c: cm, g_in },
            [idx(WHITE), idx(PINK), idx(RED)],
            nodes,
        ))
    }

    /// New loads (the switches or the mixer's VOLUME moved). The outputs are AC coupled, so
    /// their loads leave the operating point where it is: only the outputs' conductances
    /// change, and the nodes keep their state. Applied at most every [`LOAD_EVERY`] output
    /// samples (a knob turning), the latest loads winning.
    pub fn set_loads(&mut self, loads: NoiseLoads) {
        self.pending = (loads != self.loads).then_some(loads);
    }

    fn apply_loads(&mut self, loads: NoiseLoads) {
        let n = self.model.n;
        let g = |r: f64| 1.0 / open(r);
        let old = [self.loads.white, self.loads.pink, self.loads.red];
        let new = [loads.white, loads.pink, loads.red];
        for k in 0..3 {
            let i = self.out[k];
            self.model.g[i * n + i] += g(new[k]) - g(old[k]);
            // (The other model's too, for a switch.)
            let (m, _, out) = &mut self.other;
            let (i, n) = (out[k], m.n);
            m.g[i * n + i] += g(new[k]) - g(old[k]);
        }
        let h = 1.0 / (self.rate * self.os as f64);
        self.sys.rebuild(&self.model, h);
        self.loads = loads;
        self.since_loads = 0;
    }

    /// The source's noise density, V per root hertz (one-sided): Q15's avalanche noise as
    /// the factory's trim left it.
    pub fn set_density(&mut self, v_per_rt_hz: f64) {
        self.density = v_per_rt_hz;
        // White noise of one-sided density S sampled at fs: variance S fs / 2.
        self.sigma = v_per_rt_hz * libm::sqrt(self.rate * self.os as f64 / 2.0);
    }

    /// The quality mode, from the next sample (switchable while it plays): in Potato the
    /// model without the transistors' series resistances stepped once a sample, not the
    /// circuit's four times and decimated (the same colouring, its top octave aliased).
    /// The nodes keep their state across a switch.
    pub fn set_quality(&mut self, q: crate::voice::Quality) {
        let potato = q == crate::voice::Quality::Potato;
        if potato != self.plain {
            self.plain = potato;
            self.os = if potato { 1 } else { OVERSAMPLE };
            let (m, sys, out) = &mut self.other;
            std::mem::swap(&mut self.model, m);
            std::mem::swap(&mut self.out, out);
            let h = 1.0 / (self.rate * self.os as f64);
            sys.rebuild(&self.model, h);
            // From the full model's nodes to the plain's, or back.
            sys.carry(&self.sys, &self.pairs, !potato);
            std::mem::swap(&mut self.sys, sys);
            self.set_density(self.density);
            self.dec.iter_mut().for_each(Decimator::reset);
        }
    }

    /// Seeds the generator again (a device's own stream).
    pub fn reseed(&mut self, seed: u64) {
        self.rng = Gauss::new(seed);
    }

    pub fn density(&self) -> f64 {
        self.density
    }

    /// The small-signal model (for the tests' frequency responses) and the outputs' indices
    /// (white, pink, red).
    pub fn model_of(&self) -> (&SmallSignal, &Discrete, [usize; 3]) {
        (&self.model, &self.sys, self.out)
    }

    /// One output sample.
    pub fn tick(&mut self) -> NoiseOut {
        self.since_loads = self.since_loads.saturating_add(1);
        // (In Potato loads taken every 512 samples: each takes the model's rebuilding.)
        let every = if self.os == 1 {
            8 * LOAD_EVERY
        } else {
            LOAD_EVERY
        };
        if self.since_loads >= every
            && let Some(loads) = self.pending.take()
        {
            self.apply_loads(loads);
        }
        let mut y = [0.0; 3];
        if self.os == 1 {
            let u = self.sigma * self.rng.next();
            self.sys.step(u);
            for (yk, &node) in y.iter_mut().zip(&self.out) {
                *yk = self.sys.x(node);
            }
        } else {
            for _ in 0..OVERSAMPLE {
                let u = self.sigma * self.rng.next();
                self.sys.step(u);
                for ((yk, dec), &node) in y.iter_mut().zip(&mut self.dec).zip(&self.out) {
                    if let Some(v) = dec.push(self.sys.x(node)) {
                        *yk = v;
                    }
                }
            }
        }
        NoiseOut {
            white: y[0],
            pink: y[1],
            red: y[2],
        }
    }

    /// The decimators' delay, output samples.
    pub fn latency(&self) -> f64 {
        self.dec[0].delay()
    }
}

/// A seeded Gaussian generator: PCG32 and the Box-Muller transform.
#[derive(Debug, Clone)]
pub struct Gauss {
    state: u64,
    spare: Option<f64>,
}

impl Gauss {
    pub fn new(seed: u64) -> Gauss {
        let mut g = Gauss {
            state: seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407),
            spare: None,
        };
        g.next_u32();
        g
    }

    fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// A uniform number in (0, 1].
    fn uniform(&mut self) -> f64 {
        (f64::from(self.next_u32()) + 1.0) / 4294967296.0
    }

    /// A standard normal number.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        if let Some(s) = self.spare.take() {
            return s;
        }
        let (u1, u2) = (self.uniform(), self.uniform());
        let r = libm::sqrt(-2.0 * crate::ulp::log(u1));
        let th = 2.0 * core::f64::consts::PI * u2;
        self.spare = Some(r * crate::ulp::sin(th));
        r * crate::ulp::cos(th)
    }
}
