//! Board 4's external preamplifier and overload lamp driver (circuit No. 12,
//! `board4-preamp.lib`; docs/circuit/board4.md) in real time, solved as their circuit by
//! the nodal solver ([`crate::mna`]).
//!
//! The EXTERNAL INPUT's volume (R9, 1M audio) is a Thevenin source into pin 11; the
//! preamplifier (gain about 200) runs at four trapezoidal substeps a sample (its closed loop
//! reaches past 50 kHz), its input upsampled and its outputs decimated by the halfband
//! resamplers ([`crate::resample`]; their delay is [`Preamp::latency`]). Its
//! output reaches the mixer's bus through C20, SW10 and R46 33K. The lamp driver runs once a
//! sample from the preamplifier's output (R53's load on the output is left out: the output
//! is the feedback loop's, a few ohms).

use crate::contour::Q2N3392;
use crate::devices::Bjt;
use crate::mna::{Circuit, GND, NoConvergence, Node, Part};
use crate::resample::{Decimator, Interpolator};
use crate::vca::Q2N4058;
use crate::vcf::{TIS92, TIS93, TIS97};

/// The preamplifier's parts that the panel or the harness sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreampCircuit {
    /// B1, the OVERLOAD lamp from +15 V (its hot resistance; the type is not documented).
    pub r_lamp: f64,
    /// The lamp's current at full brightness, A.
    pub i_lamp: f64,
    pub q27: Bjt,
    pub q33: Bjt,
    pub q25: Bjt,
    pub q31: Bjt,
    pub q34: Bjt,
}

impl Default for PreampCircuit {
    fn default() -> Self {
        PreampCircuit {
            r_lamp: 200.0,
            i_lamp: 0.06,
            q27: TIS97,
            q33: Q2N4058,
            q25: Q2N3392,
            q31: TIS93,
            q34: TIS92,
        }
    }
}

const P10: Node = 1;
const N10: Node = 2;
/// The jack's voltage after R9's divider (held), behind R9's resistance and R78.
const SRC: Node = 3;
const HELD: usize = 4;
const N78: Node = 4;
const B27: Node = 5;
const C27: Node = 6;
const N64: Node = 7;
const TAIL: Node = 8;
const B32: Node = 9;
const N61: Node = 10;
const N62: Node = 11;
const AMP: Node = 12;
const OUT: Node = 13;
const NODES: usize = 14;

// The lamp driver.
const L15: Node = 1;
const LN10: Node = 2;
const LAMP_IN: Node = 3;
const LHELD: usize = 4;
const B25: Node = 4;
const E25: Node = 5;
const M: Node = 6;
const E31: Node = 7;
const LAMP: Node = 8;
const C34: Node = 9;
const LNODES: usize = 10;

/// Substeps a sample for the preamplifier.
pub const SUBSTEPS: usize = 4;

/// Potato's preamplifier and lamp: the circuit's paths, not the circuit solved. The
/// input's coupling (C23 against R78, R9's source resistance and R66 with the input pair's
/// own input resistance, 96.6K together) divides and high-passes; the loop, an open-loop gain
/// of 726 driving the feedback node (between R61 from the output and R62 to C26) towards the
/// input (a gain of 157 above C26's corner, 1 at DC: the two fitted to the circuit's gain from
/// 0, 100K, 250K and 500K sources, within 0.1 %), until the output
/// clips at the circuit's -7.3 and +9.9 V (a quadratic knee of 0.3 V): C26 charges from the
/// output as it is, clipped or not (its charge shifting with an asymmetric clip); C20 into R46
/// high-passes it to the bus. The lamp as the circuit's: dark below about 1.4 V of output
/// peak, lit from 1.8 V (a smoothstep between), the peak held and falling with a time
/// constant of 0.122 s (lit about 0.21 s after a clipping drive stops, as the circuit's).
/// Each linear part a one-pole filter at the sample rate (no oversampling: its clipping
/// aliases).
#[derive(Debug, Clone, Copy, Default)]
pub struct Plain {
    /// The input's coupling's and C20's one-poles' states, C26's voltage, the output's held
    /// peak.
    lp_in: f64,
    v26: f64,
    lp_20: f64,
    peak: f64,
    /// The peak's fall over a step, and the step it is for (worked out once).
    fall: (f64, f64),
}

impl Plain {
    /// A one-pole high pass with time constant `tau` over a step `h` (trapezoidal).
    fn high(x: f64, state: &mut f64, tau: f64, h: f64) -> f64 {
        let g = h / (2.0 * tau);
        let v = (x - *state) * g / (1.0 + g);
        let y = v + *state;
        *state = y + v;
        x - y
    }

    fn clip(u: f64) -> f64 {
        const LO: f64 = -7.3;
        const HI: f64 = 9.9;
        const K: f64 = 0.3;
        if u > HI + K {
            HI
        } else if u > HI - K {
            u - (u - (HI - K)).powi(2) / (4.0 * K)
        } else if u < LO - K {
            LO
        } else if u < LO + K {
            u + (u - (LO + K)).powi(2) / (4.0 * K)
        } else {
            u
        }
    }

    fn tick(&mut self, v_in: f64, r_src: f64, on: bool, h: f64) -> PreampOut {
        const R_IN: f64 = 96.6e3;
        const A: f64 = 725.6;
        let rt = r_src + 1e3 + R_IN;
        let vb = R_IN / rt * Self::high(v_in, &mut self.lp_in, 0.1e-6 * rt, h);
        // The loop: the output A times the input less the feedback node, (R62 * output + R61
        // * C26's voltage) / (R61 + R62), then clipped; C26 charged through R62 from the node
        // as it is.
        const R61: f64 = 200e3;
        const R62: f64 = 1e3;
        let k = R62 / (R61 + R62);
        let amp = Self::clip(A * (vb - self.v26 * (1.0 - k)) / (1.0 + A * k));
        let node = (amp * R62 + self.v26 * R61) / (R61 + R62);
        self.v26 += h / 220e-6 * (node - self.v26) / R62;
        let out = if on {
            Self::high(amp, &mut self.lp_20, 1e-6 * 33e3, h)
        } else {
            0.0
        };
        if self.fall.0 != h {
            self.fall = (h, crate::ulp::exp(-h / 0.122));
        }
        self.peak = (self.peak * self.fall.1).max(amp);
        let x = ((self.peak - 1.375) / 0.40).clamp(0.0, 1.0);
        PreampOut {
            amp,
            i_bus: out / 33e3,
            lamp: 1.004 * x * x * (3.0 - 2.0 * x),
        }
    }
}

/// How the preamplifier is solved inside FEEDBACK's loop in Potato (decisions.md R8, R11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InLoop {
    /// [`Delayed`]: Potato's plain model at twice the rate, its output as late as the
    /// circuit's (R11). Within the circuit's own spread in the loop, at a fraction of its cost.
    #[default]
    Delayed,
    /// As its circuit (High Fidelity's solve; R8): about four times a voice's cost.
    Circuit,
    /// Potato's plain model as it stands: the loop, some 45 samples shorter than with the
    /// circuit, falls into a low, dark oscillation the circuit does not make.
    Plain,
}

/// Potato's preamplifier inside FEEDBACK's loop (decisions.md R11): [`Plain`]'s equations at
/// twice the sample rate between sparse halfband resamplers, its input delayed so that its
/// output comes as late as the circuit's does through its own resamplers (four substeps a
/// sample: 44.5 samples). In the loop that delay is what matters most: without it the loop
/// is some 45 samples shorter than the circuit's, and falls into another oscillation (R8).
/// The input's coupling, C20 and the lamp run at the sample rate (each a one-pole, C26's
/// and the lamp's slow).
#[derive(Debug, Clone)]
pub struct Delayed {
    up: Interpolator,
    down: Decimator,
    /// The input's delay at twice the rate: its line, where the next sample goes, and its
    /// length.
    line: [f64; 32],
    pos: usize,
    delay: usize,
    /// The input's coupling's, C26's and C20's states, the output's held peak and the last
    /// output.
    lp_in: f64,
    v26: f64,
    lp_20: f64,
    peak: f64,
    amp: f64,
    /// The coefficients for a sample of `dt` from a source of `r_src`: the input coupling's
    /// divider and one-pole, C20's one-pole, C26's step at twice the rate, the peak's fall.
    key: (f64, f64),
    k_in: (f64, f64),
    k_20: f64,
    k_26: f64,
    fall: f64,
}

impl Delayed {
    /// Its delay to come out `latency` samples late (the circuit's).
    pub fn new(latency: f64) -> Delayed {
        let up = Interpolator::sparse(2);
        let down = Decimator::sparse(2);
        let delay = 2.0 * (latency - up.delay() - down.delay());
        assert!(
            delay >= 0.0 && delay.fract() == 0.0 && delay < 32.0,
            "a delay of {delay} at twice the rate"
        );
        Delayed {
            up,
            down,
            line: [0.0; 32],
            pos: 0,
            delay: delay as usize,
            lp_in: 0.0,
            v26: 0.0,
            lp_20: 0.0,
            peak: 0.0,
            amp: 0.0,
            key: (f64::NAN, f64::NAN),
            k_in: (0.0, 0.0),
            k_20: 0.0,
            k_26: 0.0,
            fall: 0.0,
        }
    }

    /// From silence (it allocates nothing).
    pub fn reset(&mut self) {
        self.up.reset();
        self.down.reset();
        self.line = [0.0; 32];
        self.pos = 0;
        (self.lp_in, self.v26, self.lp_20, self.peak, self.amp) = (0.0, 0.0, 0.0, 0.0, 0.0);
    }

    /// A trapezoidal one-pole high pass of coefficient `g / (1 + g)` (`g` = h / 2 tau): as
    /// [`Plain::high`].
    fn high(x: f64, state: &mut f64, k: f64) -> f64 {
        let v = (x - *state) * k;
        let y = v + *state;
        *state = y + v;
        x - y
    }

    fn tick(&mut self, v_in: f64, r_src: f64, on: bool, dt: f64) -> PreampOut {
        const R_IN: f64 = 96.6e3;
        const A: f64 = 725.6;
        const R61: f64 = 200e3;
        const R62: f64 = 1e3;
        if self.key != (dt, r_src) {
            self.key = (dt, r_src);
            let rt = r_src + 1e3 + R_IN;
            let pole = |tau: f64| {
                let g = dt / (2.0 * tau);
                g / (1.0 + g)
            };
            self.k_in = (R_IN / rt, pole(0.1e-6 * rt));
            self.k_20 = pole(1e-6 * 33e3);
            self.k_26 = 0.5 * dt / 220e-6 / R62;
            self.fall = crate::ulp::exp(-dt / 0.122);
        }
        // As Plain (`Plain::tick`): the input's coupling, then the loop clipped, C26 charged
        // through R62 from the feedback node; here at twice the rate, the input delayed.
        let vb = self.k_in.0 * Self::high(v_in, &mut self.lp_in, self.k_in.1);
        let mut fine = [0.0; 2];
        self.up.push(vb, &mut fine);
        let k = R62 / (R61 + R62);
        for x in fine {
            self.line[self.pos] = x;
            let x = self.line[(self.pos + 32 - self.delay) % 32];
            self.pos = (self.pos + 1) % 32;
            let a = Plain::clip(A * (x - self.v26 * (1.0 - k)) / (1.0 + A * k));
            let node = (a * R62 + self.v26 * R61) / (R61 + R62);
            self.v26 += self.k_26 * (node - self.v26);
            if let Some(v) = self.down.push(a) {
                self.amp = v;
            }
        }
        let amp = self.amp;
        let out = Self::high(amp, &mut self.lp_20, self.k_20);
        self.peak = (self.peak * self.fall).max(amp);
        let x = ((self.peak - 1.375) / 0.40).clamp(0.0, 1.0);
        PreampOut {
            amp,
            i_bus: if on { out / 33e3 } else { 0.0 },
            lamp: 1.004 * x * x * (3.0 - 2.0 * x),
        }
    }
}

/// One sample's outputs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PreampOut {
    /// The preamplifier's output (Q33's collector), V.
    pub amp: f64,
    /// The current SW10 and R46 put into the mixer's bus (at 0 V), A.
    pub i_bus: f64,
    /// The OVERLOAD lamp's current over its full-brightness current, 0..
    pub lamp: f64,
}

/// The preamplifier and lamp driver.
#[derive(Debug, Clone)]
pub struct Preamp {
    pub circuit: PreampCircuit,
    net: Circuit,
    lamp: Circuit,
    r_src: usize,
    r46: usize,
    dt: f64,
    up: Interpolator,
    down_amp: Decimator,
    down_out: Decimator,
    last: (f64, f64),
    r_in: f64,
    on: bool,
    /// Potato: the preamplifier and lamp as [`Plain`] from their parts, not the circuit
    /// solved.
    plain: Option<Plain>,
    /// Potato's model inside FEEDBACK's loop ([`Delayed`]), and whether it runs.
    delayed: Delayed,
    delayed_on: bool,
    /// Steps whose Newton iterations failed even at a sixteenth of the step (the outputs
    /// then hold): tests require none.
    pub failed: usize,
}

impl Preamp {
    /// The preamplifier's and the lamp driver's nodal circuits (for profiles).
    pub fn circuits(&self) -> (&Circuit, &Circuit) {
        (&self.net, &self.lamp)
    }

    /// The quality mode, from the next sample (switchable while it plays): Newton's
    /// tolerance for both circuits, in High Fidelity and Potato 1e-5 V (and relative:
    /// the heaviest blocks' iterations a third fewer, the audio within 4.3e-8 of full scale
    /// of No Compromises', `ca72-lab hifi`).
    pub fn set_quality(&mut self, q: crate::voice::Quality) {
        // Potato runs the plain model (from silence); back to the circuit, its resamplers
        // start again from silence too.
        let potato = q == crate::voice::Quality::Potato;
        if potato != self.plain.is_some() {
            self.plain = potato.then(Plain::default);
            self.up.reset();
            self.down_amp.reset();
            self.down_out.reset();
        }
        let t = match q {
            crate::voice::Quality::NoCompromises => (1e-9, 1e-9),
            crate::voice::Quality::HighFidelity | crate::voice::Quality::Potato => (1e-5, 1e-5),
        };
        self.net.tolerance = t;
        self.lamp.tolerance = t;
    }

    /// The circuit at `rate` Hz at its operating point, its input at 0 V behind `r_src`
    /// (R9's source resistance), SW10 on.
    pub fn new(circuit: PreampCircuit, rate: f64, r_src: f64) -> Result<Preamp, NoConvergence> {
        let k = &circuit;
        let mut c = Circuit::new(NODES, HELD);
        c.set(P10, 10.0);
        c.set(N10, -10.0);
        for (n, v) in [(TAIL, -0.6), (C27, 9.3), (N64, 9.3), (AMP, 0.0), (B32, 0.0)] {
            c.set(n, v);
        }
        let r = |c: &mut Circuit, a, b, r: f64| c.add(Part::Resistor { a, b, r });
        let cap = |c: &mut Circuit, a, b, v: f64| c.add(Part::Capacitor { a, b, c: v });
        let r_src_part = r(&mut c, SRC, N78, r_src + 1e3);
        cap(&mut c, N78, B27, 0.1e-6);
        r(&mut c, B27, GND, 100e3);
        c.add_bjt(C27, B27, TAIL, &k.q27, 25.0, false);
        r(&mut c, P10, C27, 27e3);
        r(&mut c, C27, N64, 100.0);
        cap(&mut c, N64, P10, 100e-12);
        c.add_bjt(P10, B32, TAIL, &k.q27, 25.0, false);
        r(&mut c, TAIL, N10, 150e3);
        c.add_bjt(AMP, C27, P10, &k.q33, 25.0, true);
        r(&mut c, AMP, N10, 10e3);
        r(&mut c, B32, N61, 1e3);
        r(&mut c, N61, AMP, 200e3);
        r(&mut c, N61, N62, 1e3);
        cap(&mut c, N62, GND, 220e-6);
        cap(&mut c, B32, AMP, 10e-12);
        cap(&mut c, AMP, OUT, 1e-6);
        let r46 = r(&mut c, OUT, GND, 33e3);
        c.dc()?;
        // The trapezoidal rule: backward Euler's damping at these steps upsets the loop near
        // its bandwidth (-9 dB at 20 kHz against ngspice at four substeps; numerics.md).
        c.set_theta(0.5);
        let mut l = Circuit::new(LNODES, LHELD);
        l.set(L15, 15.0);
        l.set(LN10, -10.0);
        l.set(LAMP_IN, c.v(AMP));
        for (n, v) in [
            (E25, -0.5),
            (M, -1.8),
            (E31, -1.2),
            (LAMP, 15.0),
            (C34, 15.0),
        ] {
            l.set(n, v);
        }
        let lr = |c: &mut Circuit, a, b, r: f64| c.add(Part::Resistor { a, b, r });
        lr(&mut l, LAMP_IN, B25, 10e3);
        l.add_bjt(L15, B25, E25, &k.q25, 25.0, false);
        l.add(Part::Capacitor {
            a: E25,
            b: GND,
            c: 0.47e-6,
        });
        lr(&mut l, E25, M, 100e3);
        lr(&mut l, M, LN10, 680e3);
        l.add_bjt(LN10, M, E31, &k.q31, 25.0, true);
        lr(&mut l, L15, E31, 10e3);
        l.add_bjt(C34, E31, GND, &k.q34, 25.0, false);
        lr(&mut l, LAMP, C34, 47.0);
        lr(&mut l, L15, LAMP, k.r_lamp);
        l.dc()?;
        let up = Interpolator::new(SUBSTEPS);
        let down_amp = Decimator::new(SUBSTEPS);
        let latency = up.delay() + down_amp.delay();
        Ok(Preamp {
            circuit,
            net: c,
            lamp: l,
            r_src: r_src_part,
            r46,
            dt: 1.0 / rate,
            up,
            down_amp,
            down_out: Decimator::new(SUBSTEPS),
            last: (0.0, 0.0),
            r_in: r_src,
            on: true,
            plain: None,
            delayed: Delayed::new(latency),
            delayed_on: false,
            failed: 0,
        })
    }

    /// R9's source resistance at its present setting, ohm.
    pub fn set_source_resistance(&mut self, r_src: f64) {
        if r_src != self.r_in {
            self.r_in = r_src;
            self.net.set_resistance(self.r_src, r_src + 1e3);
        }
    }

    /// SW10: on, R46 takes the output to the bus; off, C20's far side is open.
    pub fn set_switch(&mut self, on: bool) {
        if on != self.on {
            self.on = on;
            self.net
                .set_resistance(self.r46, if on { 33e3 } else { 1e12 });
        }
    }

    fn step(net: &mut Circuit, h: f64) -> bool {
        if net.step(h).is_ok() {
            return true;
        }
        (0..16).all(|_| net.step(h / 16.0).is_ok())
    }

    /// The resamplers' delay, samples.
    pub fn latency(&self) -> f64 {
        self.up.delay() + self.down_amp.delay()
    }

    /// Potato's model inside FEEDBACK's loop ([`Delayed`]) on, from silence, or off (the
    /// quality mode's model again; Potato's plain one from silence). It allocates nothing.
    pub fn set_delayed(&mut self, on: bool) {
        if on != self.delayed_on {
            self.delayed_on = on;
            if on {
                self.delayed.reset();
            } else if self.plain.is_some() {
                self.plain = Some(Plain::default());
            }
        }
    }

    /// One sample: the jack's voltage after R9's divider (the Thevenin source's voltage).
    pub fn tick(&mut self, v_in: f64) -> PreampOut {
        if self.delayed_on {
            let o = self.delayed.tick(v_in, self.r_in, self.on, self.dt);
            self.last = (o.amp, o.i_bus * 33e3);
            return o;
        }
        if let Some(mut p) = self.plain {
            let o = p.tick(v_in, self.r_in, self.on, self.dt);
            self.plain = Some(p);
            self.last = (o.amp, o.i_bus * 33e3);
            return o;
        }
        let h = self.dt / SUBSTEPS as f64;
        let mut fine = [0.0; SUBSTEPS];
        self.up.push(v_in, &mut fine);
        let (mut amp, mut out) = self.last;
        for &x in &fine {
            self.net.set(SRC, x);
            if !Self::step(&mut self.net, h) {
                self.failed += 1;
            }
            if let Some(a) = self.down_amp.push(self.net.v(AMP)) {
                amp = a;
            }
            if let Some(o) = self.down_out.push(self.net.v(OUT)) {
                out = o;
            }
        }
        self.last = (amp, out);
        self.outputs()
    }

    /// The lamp driver over the sample, and the outputs.
    fn outputs(&mut self) -> PreampOut {
        let (amp, out) = self.last;
        self.lamp.set(LAMP_IN, self.net.v(AMP));
        if !Self::step(&mut self.lamp, self.dt) {
            self.failed += 1;
        }
        let i_lamp = (15.0 - self.lamp.v(LAMP)) / self.circuit.r_lamp;
        PreampOut {
            amp,
            i_bus: if self.on { out / 33e3 } else { 0.0 },
            lamp: (i_lamp / self.circuit.i_lamp).max(0.0),
        }
    }
}

#[cfg(test)]
mod plain_tests {
    use super::*;

    /// Potato's plain preamplifier against the circuit: a sine at 1 kHz at each level (at the
    /// pin, V peak) from a 100K source, the output's peaks and RMS after a quarter second.
    #[test]
    #[ignore]
    fn the_plain_preamplifier_against_the_circuit() {
        let rate = 48e3;
        // (The lamp's threshold: 0.018 to 0.033 V, the output's peak 1.4 to 2.6 V.)
        let levels: Vec<(f64, f64)> = [
            (1000.0, 0.005),
            (1000.0, 0.03),
            (1000.0, 0.06),
            (1000.0, 0.2),
            (100.0, 0.03),
            (10_000.0, 0.03),
        ]
        .into_iter()
        .chain((0..16).map(|k| (1000.0, 0.018 + 0.001 * k as f64)))
        .collect();
        for (hz, amp) in levels {
            let mut c = Preamp::new(PreampCircuit::default(), rate, 100e3).unwrap();
            let mut p = c.clone();
            p.set_quality(crate::voice::Quality::Potato);
            let (mut pk, mut rms) = ([[f64::NEG_INFINITY, f64::INFINITY]; 2], [0.0; 2]);
            let n = (rate * 0.5) as usize;
            for i in 0..n {
                let x = amp * (2.0 * std::f64::consts::PI * hz * i as f64 / rate).sin();
                let o = [c.tick(x), p.tick(x)];
                if i > n / 2 {
                    for k in 0..2 {
                        pk[k] = [pk[k][0].max(o[k].amp), pk[k][1].min(o[k].amp)];
                        rms[k] += o[k].i_bus * o[k].i_bus;
                    }
                }
            }
            let r = |x: f64| (x / (n / 2) as f64).sqrt() * 33e3;
            println!(
                "{hz} Hz, {amp} V: circuit amp {:.3}/{:.3}, bus rms {:.4} V; plain {:.3}/{:.3}, {:.4} V; lamp {:.3} / {:.3}",
                pk[0][0],
                pk[0][1],
                r(rms[0]),
                pk[1][0],
                pk[1][1],
                r(rms[1]),
                c.tick(0.0).lamp,
                p.tick(0.0).lamp
            );
        }
    }
}

#[cfg(test)]
mod lamp_decay {
    use super::*;

    /// The circuit's lamp after a 1 kHz drive stops (a strong one and a light one): its level
    /// every 20 ms.
    #[test]
    #[ignore]
    fn the_lamp_after_the_drive_stops() {
        let rate = 48e3;
        for amp in [0.2, 0.024] {
            let mut c = Preamp::new(PreampCircuit::default(), rate, 100e3).unwrap();
            let mut line = String::new();
            for i in 0..(rate as usize) {
                let x = if i < 4800 {
                    amp * (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / rate).sin()
                } else {
                    0.0
                };
                let o = c.tick(x);
                if i >= 4800 && (i - 4800) % 960 == 0 && i < 4800 + 960 * 25 {
                    line.push_str(&format!("{:.2} ", o.lamp));
                }
            }
            println!("{amp} V: {line}");
        }
    }
}

#[cfg(test)]
mod plain_harmonics {
    use super::*;

    /// The circuit's and the plain model's bus current at perf-ext's settings (220 Hz, VOLUME
    /// 0.85: 0.051 V at the pin from 250K; VOLUME 1: 0.1 V from 0 ohm): the first harmonics'
    /// amplitudes (a second after the start), relative to the circuit's fundamental, dB.
    #[test]
    #[ignore]
    fn the_plain_preamplifier_harmonics() {
        let rate = 48e3;
        for (pin, r_src) in [
            (0.051, 250e3),
            (0.1, 0.0),
            (0.002, 0.0),
            (0.002, 100e3),
            (0.002, 500e3),
        ] {
            let mut c = Preamp::new(PreampCircuit::default(), rate, r_src).unwrap();
            let mut p = c.clone();
            p.set_quality(crate::voice::Quality::Potato);
            let n = rate as usize;
            let (mut xc, mut xp) = (Vec::new(), Vec::new());
            for i in 0..2 * n {
                let x = pin * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / rate).sin();
                let (a, b) = (c.tick(x), p.tick(x));
                if i >= n {
                    xc.push(a.i_bus);
                    xp.push(b.i_bus);
                }
            }
            let harm = |x: &[f64], k: f64| {
                let w = 2.0 * std::f64::consts::PI * 220.0 * k / rate;
                let (re, im) = x.iter().enumerate().fold((0.0, 0.0), |(r, m), (i, v)| {
                    (r + v * (w * i as f64).cos(), m + v * (w * i as f64).sin())
                });
                (re * re + im * im).sqrt()
            };
            let f = harm(&xc, 1.0);
            let line: Vec<String> = (1..=6)
                .map(|k| {
                    let db = |x: &[f64]| 20.0 * (harm(x, k as f64) / f).log10();
                    format!("h{k} {:.1}/{:.1}", db(&xc), db(&xp))
                })
                .collect();
            // The circuit's gain from the source (its fundamental's amplitude, the bus
            // current into 33K, over the source's).
            let gain = f * 2.0 / xc.len() as f64 * 33e3 / pin;
            println!(
                "pin {pin} V from {r_src} ohm (circuit/plain dB): {}; circuit gain {gain:.2}",
                line.join(", ")
            );
        }
    }
}

#[cfg(test)]
mod plain_calibration {
    use super::*;

    /// Potato's plain preamplifier follows the circuit: its gain from sources of 0 to 500K
    /// within 0.1 dB at 220 Hz, its clipping levels within 0.05 V, and its lamp dark at an
    /// output peak of 1.3 V and lit at 2.3 V as the circuit's is.
    #[test]
    fn the_plain_preamplifier_follows_the_circuit() {
        let rate = 48e3;
        let run = |pin: f64, r_src: f64| {
            let mut c = Preamp::new(PreampCircuit::default(), rate, r_src).unwrap();
            let mut p = c.clone();
            p.set_quality(crate::voice::Quality::Potato);
            let n = rate as usize / 2;
            let mut out = [(0.0f64, f64::NEG_INFINITY, f64::INFINITY, 0.0); 2];
            for i in 0..2 * n {
                let x = pin * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / rate).sin();
                for (k, o) in [c.tick(x), p.tick(x)].into_iter().enumerate() {
                    if i >= n {
                        let (rms, hi, lo, _) = out[k];
                        out[k] = (
                            rms + o.i_bus * o.i_bus,
                            hi.max(o.amp),
                            lo.min(o.amp),
                            o.lamp,
                        );
                    }
                }
            }
            out
        };
        for r_src in [0.0, 100e3, 250e3, 500e3] {
            let o = run(0.002, r_src);
            let db = 10.0 * (o[1].0 / o[0].0).log10();
            assert!(db.abs() < 0.1, "from {r_src} ohm: {db:.3} dB");
        }
        let o = run(0.2, 0.0);
        assert!(
            (o[1].1 - o[0].1).abs() < 0.05 && (o[1].2 - o[0].2).abs() < 0.05,
            "{o:?}"
        );
        for (pin, lit) in [(0.017, false), (0.03, true)] {
            let o = run(pin, 100e3);
            assert_eq!((o[0].3 > 0.5, o[1].3 > 0.5), (lit, lit), "{pin} V: {o:?}");
        }
    }
}
