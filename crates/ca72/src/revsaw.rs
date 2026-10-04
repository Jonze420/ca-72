//! Oscillator 3's reverse sawtooth: Q37's shunt-feedback inverter on its sawtooth output
//! (`board1-osc23.lib`, mm_revsaw; board1.md, "Oscillators 2 and 3"), solved as its circuit
//! by the nodal solver ([`crate::mna`]) with C10's decoupling as a state (its ripple shows
//! when oscillator 3 runs slowly).
//!
//! R171 loads oscillator 3's sawtooth output all the time; the stage's output 7B reaches the
//! WAVEFORM switch through R172 and R176.
//!
//! In Potato the same equations are solved for Q37's two junctions alone ([`Plain`]).

use crate::contour::Q2N3392;
use crate::devices::{Bjt, BjtAt};
use crate::mna::{
    Circuit, Depletion, DepletionConsts, GND, NoConvergence, Node, Part, diffusion_charge, pnjlim,
    vcrit,
};
use crate::voice::Quality;

/// The stage's parts (Figure 9-3; the drawing labels the 7.5K "R175", Table 7-5 R172).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RevSawCircuit {
    pub r164: f64,
    pub c10: f64,
    pub r165: f64,
    pub r175: f64,
    pub r171: f64,
    pub r182: f64,
    pub r172: f64,
    pub r176: f64,
    pub q37: Bjt,
    /// The sawtooth output's source resistance: R34 4300 against R33 4700 (R146, R145).
    pub r_saw: f64,
}

impl Default for RevSawCircuit {
    fn default() -> Self {
        RevSawCircuit {
            r164: 330.0,
            c10: 2.2e-6,
            r165: 2.2e3,
            r175: 68e3,
            r171: 33e3,
            r182: 390e3,
            r172: 7.5e3,
            r176: 18e3,
            q37: Q2N3392,
            r_saw: 4300.0 * 4700.0 / 9000.0,
        }
    }
}

const P10: Node = 1;
const N10: Node = 2;
/// The sawtooth's open-circuit voltage (held), behind R34 || R33.
const SRC: Node = 3;
const HELD: usize = 4;
const SAW: Node = 4;
const NA: Node = 5;
const B37: Node = 6;
const C37: Node = 7;
const OUT: Node = 8;
const NODES: usize = 9;

/// The stage's outputs for one sample.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RevSawOut {
    /// The sawtooth output as R171 loads it (13B), V.
    pub saw: f64,
    /// The reverse sawtooth unloaded (7B), V, and its source resistance at rest, ohm.
    pub rev: f64,
    pub r_rev: f64,
}

/// The stage for Potato: Q37's two junctions solved by Newton's method with the linear
/// network folded around them, C10 and Q37's junctions' charges (the emitter junction's
/// depletion and diffusion charges, the collector junction's depletion charge: the Miller
/// capacitance R175's feedback multiplies) backward Euler states as the nodal solver steps
/// them: the circuit's equations without its node-by-node solve (Potato).
#[derive(Debug, Clone, Copy)]
struct Plain {
    /// C10's voltage; Q37's internal junctions (base-emitter, base-collector), V.
    v_na: f64,
    x: [f64; 2],
    /// The junctions' charges, C, and the references their powers are taken near
    /// ([`Depletion::charge_near`]).
    q: [f64; 2],
    near: [[f64; 2]; 2],
}

/// [`Plain`]'s Newton steps stop below this, V.
const PLAIN_TOL: f64 = 1e-9;

/// The stage.
#[derive(Debug, Clone)]
pub struct RevSaw {
    pub circuit: RevSawCircuit,
    net: Circuit,
    dt: f64,
    r_out: f64,
    /// Q37 at 25 C, its junctions' depletion (emitter, collector) with their constants, and
    /// the plain stage while in Potato.
    q37: BjtAt,
    dep: [(Depletion, DepletionConsts); 2],
    plain: Option<Plain>,
    /// Samples whose Newton iterations failed even at a sixteenth of the step (the outputs
    /// then hold): tests require none.
    pub failed: usize,
}

impl RevSaw {
    /// The stage at `rate` Hz, settled with the sawtooth at 0 V.
    pub fn new(circuit: RevSawCircuit, rate: f64) -> Result<RevSaw, NoConvergence> {
        let k = &circuit;
        let mut c = Circuit::new(NODES, HELD);
        c.set(P10, 10.0);
        c.set(N10, -10.0);
        for (n, v) in [(SAW, 0.0), (NA, 9.5), (B37, 0.65), (C37, 4.2), (OUT, 0.0)] {
            c.set(n, v);
        }
        c.add(Part::Resistor {
            a: SRC,
            b: SAW,
            r: k.r_saw,
        });
        c.add(Part::Resistor {
            a: P10,
            b: NA,
            r: k.r164,
        });
        c.add(Part::Capacitor {
            a: NA,
            b: GND,
            c: k.c10,
        });
        c.add(Part::Resistor {
            a: NA,
            b: C37,
            r: k.r165,
        });
        c.add(Part::Resistor {
            a: C37,
            b: B37,
            r: k.r175,
        });
        c.add(Part::Resistor {
            a: SAW,
            b: B37,
            r: k.r171,
        });
        c.add(Part::Resistor {
            a: B37,
            b: N10,
            r: k.r182,
        });
        c.add_bjt(C37, B37, GND, &k.q37, 25.0, false);
        c.add(Part::Resistor {
            a: C37,
            b: OUT,
            r: k.r172,
        });
        c.add(Part::Resistor {
            a: OUT,
            b: N10,
            r: k.r176,
        });
        c.dc()?;
        // The output's source resistance at rest: its drop under a 10K load to -10 V over
        // the load's current (the output rests near 0 V, so a load to ground would draw
        // nothing).
        let v0 = c.v(OUT);
        let mut probe = c.clone();
        probe.add(Part::Resistor {
            a: OUT,
            b: N10,
            r: 10e3,
        });
        probe.dc()?;
        let v1 = probe.v(OUT);
        let r_out = (v0 - v1) / ((v1 + 10.0) / 10e3);
        let dep = |cj0, vj, m| {
            let d = Depletion {
                cj0,
                vj,
                m,
                fc: 0.5,
            };
            (d, d.consts())
        };
        let q = &k.q37;
        Ok(RevSaw {
            q37: q.at(25.0),
            dep: [dep(q.cje, q.vje, q.mje), dep(q.cjc, q.vjc, q.mjc)],
            circuit,
            net: c,
            dt: 1.0 / rate,
            r_out,
            plain: None,
            failed: 0,
        })
    }

    /// The quality mode, from the next sample: in Potato the plain stage, from the
    /// circuit's state; back to the circuit, it resumes where it stopped (only C10's ripple
    /// is remembered).
    pub fn set_quality(&mut self, q: Quality) {
        let potato = q == Quality::Potato;
        if potato != self.plain.is_some() {
            self.plain = potato.then(|| {
                let (b, c) = (self.net.v(B37), self.net.v(C37));
                let x = [b, b - c];
                let mut near = [[f64::NAN; 2]; 2];
                let q = [0, 1].map(|j| self.charge(j, x[j], &mut near[j]).0);
                Plain {
                    v_na: self.net.v(NA),
                    x,
                    q,
                    near,
                }
            });
        }
    }

    /// The output's source resistance at rest, ohm.
    pub fn r_out(&self) -> f64 {
        self.r_out
    }

    /// One sample: the sawtooth's open-circuit voltage (oscillator 3's `VcoOut::saw`).
    pub fn tick(&mut self, saw_open: f64) -> RevSawOut {
        if let Some(p) = self.plain {
            return self.tick_plain(p, saw_open);
        }
        self.net.set(SRC, saw_open);
        if self.net.step(self.dt).is_err() {
            let mut ok = true;
            for _ in 0..16 {
                if self.net.step(self.dt / 16.0).is_err() {
                    ok = false;
                    break;
                }
            }
            if !ok {
                self.failed += 1;
            }
        }
        RevSawOut {
            saw: self.net.v(SAW),
            rev: self.net.v(OUT),
            r_rev: self.r_out,
        }
    }

    /// A junction's charge and capacitance (0 the emitter's with its diffusion charge, 1 the
    /// collector's) at its internal voltage `v`, as the nodal solver has them.
    fn charge(&self, j: usize, v: f64, near: &mut [f64; 2]) -> (f64, f64) {
        let (d, k) = &self.dep[j];
        let (q, c) = if d.cj0 > 0.0 {
            d.charge_near(v, near, k)
        } else {
            (0.0, 0.0)
        };
        let tf = self.circuit.q37.tf;
        if j == 0 && tf > 0.0 {
            let (qd, cd) = diffusion_charge(v, self.q37.is, self.q37.vt, tf);
            (q + qd, c + cd)
        } else {
            (q, c)
        }
    }

    /// One sample of [`Plain`]: the source through R34 || R33 and R171 into Q37's base,
    /// R175 from its collector and R182 to -10 V; the collector fed from C10's node (its
    /// backward Euler companion against R164 from +10 V, then R165) and loaded by R172 and
    /// R176 to -10 V; RB, RE and RC in series with the junctions, the collector junction's
    /// charge current from the internal base to the internal collector.
    fn tick_plain(&mut self, mut p: Plain, saw_open: f64) -> RevSawOut {
        let k = &self.circuit;
        let (q, m) = (&k.q37, &self.q37);
        let (q_old, h) = (p.q, self.dt);
        let g_c = k.c10 / self.dt;
        let g_na = g_c + 1.0 / k.r164;
        let v_th = (g_c * p.v_na + 10.0 / k.r164) / g_na;
        let r_th = k.r165 + 1.0 / g_na;
        let (r_s, r_o) = (k.r_saw + k.r171, k.r172 + k.r176);
        // The two nodes' currents' slopes against the base and collector voltages.
        let (f0b, f0c) = (-1.0 / r_s - 1.0 / k.r175 - 1.0 / k.r182, 1.0 / k.r175);
        let (f1b, f1c) = (1.0 / k.r175, -1.0 / r_th - 1.0 / k.r175 - 1.0 / r_o);
        let crit = vcrit(m.vt, m.is);
        // The terminals' currents (the charges' currents added to the base's, the collector
        // junction's taken from the collector's) with their slopes, and the base's and
        // collector's voltages.
        let nodes = |x: [f64; 2], near: &mut [[f64; 2]; 2]| {
            let mut d = m.currents_d(x[0], x[1], q.vaf);
            let (qe, ce) = self.charge(0, x[0], &mut near[0]);
            let (qc, cc) = self.charge(1, x[1], &mut near[1]);
            let (ie_q, ic_q) = ((qe - q_old[0]) / h, (qc - q_old[1]) / h);
            d.ib += ie_q + ic_q;
            d.ic -= ic_q;
            d.dib[0] += ce / h;
            d.dib[1] += cc / h;
            d.dic[1] -= cc / h;
            let ie = d.ic + d.ib;
            let vb = x[0] + q.re * ie + q.rb * d.ib;
            let vc = x[0] + q.re * ie - x[1] + q.rc * d.ic;
            (d, vb, vc, [qe, qc])
        };
        let mut near = p.near;
        let mut x = p.x;
        let mut converged = false;
        for _ in 0..60 {
            let (d, vb, vc, _) = nodes(x, &mut near);
            let f = [
                (saw_open - vb) / r_s + (vc - vb) / k.r175 + (-10.0 - vb) / k.r182 - d.ib,
                (v_th - vc) / r_th - (vc - vb) / k.r175 - (vc + 10.0) / r_o - d.ic,
            ];
            let die = [d.dic[0] + d.dib[0], d.dic[1] + d.dib[1]];
            let dvb = [
                1.0 + q.re * die[0] + q.rb * d.dib[0],
                q.re * die[1] + q.rb * d.dib[1],
            ];
            let dvc = [
                1.0 + q.re * die[0] + q.rc * d.dic[0],
                -1.0 + q.re * die[1] + q.rc * d.dic[1],
            ];
            let j = [
                [
                    f0b * dvb[0] + f0c * dvc[0] - d.dib[0],
                    f0b * dvb[1] + f0c * dvc[1] - d.dib[1],
                ],
                [
                    f1b * dvb[0] + f1c * dvc[0] - d.dic[0],
                    f1b * dvb[1] + f1c * dvc[1] - d.dic[1],
                ],
            ];
            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
            let dx = [
                (f[0] * j[1][1] - f[1] * j[0][1]) / det,
                (j[0][0] * f[1] - j[1][0] * f[0]) / det,
            ];
            let next = [
                pnjlim(x[0] - dx[0], x[0], m.vt, crit),
                pnjlim(x[1] - dx[1], x[1], m.vt, crit),
            ];
            let moved = (next[0] - x[0]).abs().max((next[1] - x[1]).abs());
            x = next;
            if moved < PLAIN_TOL {
                converged = true;
                break;
            }
        }
        // A failed sample holds, as the circuit's failed steps do (tests require none).
        let failed = !converged || !x[0].is_finite() || !x[1].is_finite();
        if failed {
            x = p.x;
        }
        let (_, vb, vc, charges) = nodes(x, &mut near);
        self.failed += usize::from(failed);
        p.x = x;
        p.q = charges;
        p.near = near;
        p.v_na = (g_c * p.v_na + 10.0 / k.r164 + vc / k.r165) / (g_na + 1.0 / k.r165);
        self.plain = Some(p);
        RevSawOut {
            saw: (saw_open / k.r_saw + vb / k.r171) / (1.0 / k.r_saw + 1.0 / k.r171),
            rev: -10.0 + (vc + 10.0) * k.r176 / r_o,
            r_rev: self.r_out,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plain stage (Potato) against the circuit, on sawtooths from 20 Hz to 5 kHz over
    /// the oscillator's swing and beyond it: the reverse sawtooth and the loaded sawtooth
    /// within 1 uV away from the resets, within 10 uV in a reset's sample and the next (the
    /// jump of volts in one step), and no failed sample.
    #[test]
    fn the_plain_stage_follows_the_circuit() {
        let rate = 48_000.0;
        for (f, lo, hi) in [
            (20.0, -2.5, 2.5),
            (440.0, -2.5, 2.5),
            (5000.0, -2.5, 2.5),
            (110.0, -6.0, 6.0),
        ] {
            let mut a = RevSaw::new(RevSawCircuit::default(), rate).unwrap();
            let mut b = a.clone();
            b.set_quality(Quality::Potato);
            // The largest differences in the samples of a reset and the next, and away.
            let (mut away, mut at) = (0.0f64, 0.0f64);
            // (The stage starts settled at 0 V: the first sample is a jump too.)
            let (mut last, mut since) = (0.0, 0);
            for i in 0..48_000 {
                let v = lo + (hi - lo) * (i as f64 * f / rate).fract();
                let (x, y) = (a.tick(v), b.tick(v));
                let d = (x.rev - y.rev).abs().max((x.saw - y.saw).abs());
                since = if v < last { 0 } else { since + 1 };
                if since < 2 {
                    at = at.max(d);
                } else {
                    away = away.max(d);
                }
                last = v;
            }
            eprintln!("{f} Hz, {lo}..{hi} V: {away:.2e} V away from the resets, {at:.2e} at them");
            assert!(away < 1e-6 && at < 1e-5, "{f} Hz: {away:e} {at:e}");
            assert_eq!((a.failed, b.failed), (0, 0));
        }
    }
}
