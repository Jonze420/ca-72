//! Board 2's keyboard circuit in real time (circuit No. 1, docs/circuit/board2.md): the key
//! string's current source, the pitch bus, the track-and-hold amplifier with GLIDE, and
//! the trigger bus's pinch-off of the hold switch, solved as the circuit
//! (`board2-keyboard.lib`) by the nodal solver ([`crate::mna`]).
//!
//! The circuit is quiet between key events: once its nodes stop moving it is stepped in
//! blocks, not every sample (the output then changes by nanovolts).

use crate::contour::{D1N4004, Q2N3392};
use crate::devices::{Bjt, vt};
use crate::mna::{Circuit, Depletion, GND, J2N4303, Jfet, NoConvergence, Node, Part};
use crate::vca::Q2N4058;
use crate::vcf::{TIS92, TIS93};
use crate::voice::Quality;

/// The keys (F to C) and the string's resistors between them (43 of 10 ohm, 1 %).
pub const KEYS: usize = 44;

/// The circuit's values (Figure 9-7) and devices (`mm-devices.lib`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardCircuit {
    /// The current source: R1 590 (Q9's emitter), R20 and R22 1K (Q11's base divider),
    /// R25 1K (Q11's emitter load); Q9 TIS93, Q11 TIS92.
    pub r1: f64,
    pub r20: f64,
    pub r22: f64,
    pub r25: f64,
    pub q9: Bjt,
    pub q11: Bjt,
    /// The string's resistors, ohm, and a key contact's resistance (unmeasured: a gold
    /// spring on a gold bar; assumptions.md A18).
    pub r_string: f64,
    pub contact: f64,
    /// The pitch bus: C9 .33 uF, R53 4.7M to +10 V.
    pub c9: f64,
    pub r53: f64,
    /// The amplifier: Q23/Q14 (2N3392) with R52 43K, R58 6.2K, R51 560; Q24 (2N4058) with
    /// R59 3.9K and R54 1.5K; R61 330 to the GLIDE control.
    pub r52: f64,
    pub r58: f64,
    pub r51: f64,
    pub r59: f64,
    pub r54: f64,
    pub r61: f64,
    pub q23: Bjt,
    pub q24: Bjt,
    /// The hold switch Q13 and follower Q10 (2N4303); R64 100K and R31 22M on Q13's gate,
    /// CR5 (1N4004) from it to the trigger bus's R65 5.1K, C13 .01 uF and R34 100K.
    pub jfet: Jfet,
    pub r64: f64,
    pub r31: f64,
    pub r65: f64,
    pub c13: f64,
    pub r34: f64,
    /// C6 1 uF (the glide capacitor), R21 10K to Q10's gate, R2 330 in its drain, R18 3.9K
    /// to -10 V, R30 10K back to Q14.
    pub c6: f64,
    pub r21: f64,
    pub r2: f64,
    pub r18: f64,
    pub r30: f64,
}

impl Default for KeyboardCircuit {
    fn default() -> Self {
        KeyboardCircuit {
            r1: 590.0,
            r20: 1e3,
            r22: 1e3,
            r25: 1e3,
            q9: TIS93,
            q11: TIS92,
            r_string: 10.0,
            contact: 0.1,
            c9: 0.33e-6,
            r53: 4.7e6,
            r52: 43e3,
            r58: 6.2e3,
            r51: 560.0,
            r59: 3.9e3,
            r54: 1.5e3,
            r61: 330.0,
            q23: Q2N3392,
            q24: Q2N4058,
            jfet: J2N4303,
            r64: 100e3,
            r31: 22e6,
            r65: 5.1e3,
            c13: 0.01e-6,
            r34: 100e3,
            c6: 1e-6,
            r21: 10e3,
            r2: 330.0,
            r18: 3.9e3,
            r30: 10e3,
        }
    }
}

/// CR5 (1N4004, `D1N4004`): its depletion charge (CJO, M; VJ and FC at ngspice's
/// defaults) and transit time.
pub const CR5_CAP: Depletion = Depletion {
    cj0: 20e-12,
    vj: 1.0,
    m: 0.33,
    fc: 0.5,
};
pub const CR5_TT: f64 = 3e-6;

/// The keyboard's output load as a Thevenin source: resistance, ohm, and voltage, V.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Load {
    pub r: f64,
    pub v: f64,
}

impl Load {
    /// The load of the oscillators' keyboard inputs (R27, R80, R129: 51.1K each, to their
    /// summing junctions at -5 V; oscillator 3's reaches the -5 V line instead when OSC. 3
    /// CONTROL is off) and the filter's KEYBOARD CONTROL 1 and 2 (R53 300K, R54 150K, to its
    /// control node near 0 V; assumptions.md A19).
    pub fn of(oscillators: usize, kbd_1: bool, kbd_2: bool) -> Load {
        let g_osc = oscillators as f64 / 51.1e3;
        let g_filter =
            if kbd_1 { 1.0 / 300e3 } else { 0.0 } + if kbd_2 { 1.0 / 150e3 } else { 0.0 };
        let g = g_osc + g_filter;
        Load {
            r: 1.0 / g,
            v: -5.0 * g_osc / g,
        }
    }
}

/// The instrument's usual load: three oscillators and KEYBOARD CONTROL 1.
pub const LOAD_DEFAULT: Load = Load {
    r: 1.0 / (3.0 / 51.1e3 + 1.0 / 300e3),
    v: -5.0 * (3.0 / 51.1e3) / (3.0 / 51.1e3 + 1.0 / 300e3),
};

// The nodes: held, then solved.
const P10: Node = 1;
const N10: Node = 2;
/// The current source's Thevenin voltage (behind its output resistance).
const ISRC: Node = 3;
/// The load's Thevenin voltage.
const LOADV: Node = 4;
const HELD: usize = 5;
/// The string's top (Q9's collector), and its nodes at the highest and lowest keys held.
const KCUR: Node = 5;
const S_HI: Node = 6;
const S_LO: Node = 7;
const TRIG: Node = 8;
const T2: Node = 9;
const GN: Node = 10;
const G13: Node = 11;
const BUS: Node = 12;
const E23: Node = 13;
const C23: Node = 14;
const C14: Node = 15;
const FB: Node = 16;
const KO: Node = 17;
const GIN: Node = 18;
const C6: Node = 19;
const G10: Node = 20;
const D10: Node = 21;
const OUT: Node = 22;
const NODES: usize = 23;

/// A string segment of no resistors (between keys held together), ohm.
const SHORT: f64 = 1e-6;

/// An open contact, ohm.
const OPEN: f64 = 1e12;

/// Stepping in blocks once quiet: the largest block, samples, and what counts as quiet
/// (the hold capacitor's and the output's change per step, V).
const BLOCK_MAX: usize = 64;
const QUIET_V: f64 = 1e-7;
const QUIET_STEPS: usize = 4;

/// Substeps per sample for 2 ms after a contact changes and while the output moves more
/// than 1 mV a sample: the trigger bus's C13 (48 us) and the hold switch's turn-on are
/// faster than a sample, and backward Euler lags them by about half a step.
const SUBSTEPS: usize = 8;

/// Newton's tolerance for the keyboard circuit in each quality mode (absolute, V, and
/// relative; `Circuit::tolerance`). In High Fidelity and Potato 1e-5 V and 1e-6: its solves
/// seldom need the second iteration that confirms the first, whose own error is of the
/// order of the correction's square (the heaviest blocks' iterations 39 % fewer, the
/// pitch within 4e-8 cent of No Compromises', `ca72-lab hifi`).
fn tolerance(q: Quality) -> (f64, f64) {
    match q {
        Quality::NoCompromises => (1e-9, 1e-9),
        Quality::HighFidelity => (1e-5, 1e-6),
        Quality::Potato => (1e-4, 1e-5),
    }
}

/// The substeps a sample while the keyboard circuit is active ([`SUBSTEPS`]); in Potato one
/// (its transients at key changes lag by up to a sample).
fn substeps(q: Quality) -> usize {
    match q {
        Quality::Potato => 1,
        _ => SUBSTEPS,
    }
}
const ACTIVE_S: f64 = 2e-3;
/// Potato's limit on a step's extrapolated start ([`crate::mna::Circuit::predict_limit`]),
/// V.
const PREDICT_LIMIT: f64 = 0.05;
/// Potato's most Newton iterations a step before it is halved.
const STEP_ITERATIONS: usize = 16;
const FAST_V: f64 = 1e-3;

/// The keyboard circuit.
#[derive(Debug, Clone)]
pub struct Keyboard {
    pub circuit: KeyboardCircuit,
    net: Circuit,
    rate: f64,
    string_current: f64,
    /// The string's parts: from the top to the highest key held, between the keys held,
    /// below the lowest; the contacts at the lowest and highest keys.
    r_top: usize,
    r_mid: usize,
    r_bot: usize,
    r_lo: usize,
    r_hi: usize,
    r_trig: usize,
    r_glide: usize,
    r_load: usize,
    /// The inputs in force: the lowest and highest keys held, the trigger contact, GLIDE's
    /// resistance in circuit (0 with the switch off), the load.
    keys: Option<(usize, usize)>,
    trig: bool,
    glide: f64,
    load: Load,
    quiet: usize,
    pending: usize,
    last: [f64; 4],
    /// Samples since a contact or GLIDE last changed, and whether the output moved fast.
    since_change: usize,
    fast: bool,
    /// The quality mode ([`tolerance`]).
    quality: Quality,
    /// Steps whose Newton iterations failed and were subdivided, and those that failed
    /// even so (the output then holds): tests require none.
    pub subdivided: usize,
    pub failed: usize,
}

impl KeyboardCircuit {
    /// The current source (Q9, Q11) into a load of `r_load` ohm: its DC operating point,
    /// the current, A.
    pub fn source_current(&self, r_load: f64) -> Result<f64, NoConvergence> {
        // Nodes: ground, +10 V; B11, B9, E9, KCUR.
        let mut c = Circuit::new(6, 2);
        c.set(1, 10.0);
        let (b11, b9, e9, kcur) = (2, 3, 4, 5);
        c.set(b11, 5.0);
        c.set(b9, 4.3);
        c.set(e9, 5.0);
        c.set(kcur, 3.6);
        c.add(Part::Resistor {
            a: 1,
            b: b11,
            r: self.r20,
        });
        c.add(Part::Resistor {
            a: b11,
            b: GND,
            r: self.r22,
        });
        c.add_bjt(1, b11, b9, &self.q11, 25.0, false);
        c.add(Part::Resistor {
            a: b9,
            b: GND,
            r: self.r25,
        });
        c.add_bjt(kcur, b9, e9, &self.q9, 25.0, true);
        c.add(Part::Resistor {
            a: 1,
            b: e9,
            r: self.r1,
        });
        c.add(Part::Resistor {
            a: kcur,
            b: GND,
            r: r_load,
        });
        c.dc()?;
        Ok(c.v(kcur) / r_load)
    }

    /// The string's current with no key held (the whole string as the source's load), A.
    pub fn string_current(&self) -> Result<f64, NoConvergence> {
        self.source_current(self.r_string * (KEYS - 1) as f64)
    }

    /// The current source as a Thevenin source (V, ohm) fitted between the whole string and
    /// one resistor as its load: its output resistance (Q9's Early effect, raised by R1)
    /// shows when keys held together short part of the string.
    fn source_thevenin(&self) -> Result<(f64, f64), NoConvergence> {
        let (ra, rb) = (self.r_string * (KEYS - 1) as f64, self.r_string);
        let (ia, ib) = (self.source_current(ra)?, self.source_current(rb)?);
        let (va, vb) = (ia * ra, ib * rb);
        let r_out = (va - vb) / (ib - ia);
        Ok((ia * r_out + va, r_out))
    }
}

impl Keyboard {
    /// The circuit at `rate` Hz, settled with no key held (the hold capacitor at 0 V).
    pub fn new(circuit: KeyboardCircuit, rate: f64) -> Keyboard {
        let k = &circuit;
        let string_current = k.string_current().unwrap_or(8.41e-3);
        let (v_src, r_src) = k.source_thevenin().unwrap_or((8.41e-3 * 1.8e6, 1.8e6));
        let mut c = Circuit::new(NODES, HELD);
        c.set(P10, 10.0);
        c.set(N10, -10.0);
        c.set(ISRC, v_src);
        c.set(LOADV, LOAD_DEFAULT.v);
        // Starting guesses (before the parts: transistors' internal nodes start at their
        // terminals'): no key, the hold switch pinched off.
        for (n, v) in [
            (KCUR, 3.6),
            (S_HI, 0.0),
            (S_LO, 0.0),
            (TRIG, -9.5),
            (T2, -10.0),
            (GN, -9.4),
            (G13, -9.4),
            (BUS, 9.8),
            (E23, 9.1),
            (C23, 9.3),
            (C14, 10.0),
            (FB, 0.0),
            (KO, 9.9),
            (GIN, 9.9),
            (C6, 0.0),
            (G10, 0.0),
            (D10, 9.2),
            (OUT, 1.2),
        ] {
            c.set(n, v);
        }
        // The current source into the string (no key held: the whole string below the
        // top), the key contacts, the trigger contact.
        c.add(Part::Resistor {
            a: ISRC,
            b: KCUR,
            r: r_src,
        });
        let r_top = c.add(Part::Resistor {
            a: KCUR,
            b: S_HI,
            r: k.r_string * (KEYS - 1) as f64,
        });
        let r_mid = c.add(Part::Resistor {
            a: S_HI,
            b: S_LO,
            r: SHORT,
        });
        let r_bot = c.add(Part::Resistor {
            a: S_LO,
            b: GND,
            r: SHORT,
        });
        let r_lo = c.add(Part::Resistor {
            a: S_LO,
            b: BUS,
            r: OPEN,
        });
        let r_hi = c.add(Part::Resistor {
            a: S_HI,
            b: BUS,
            r: OPEN,
        });
        let r_trig = c.add(Part::Resistor {
            a: P10,
            b: TRIG,
            r: OPEN,
        });
        // The trigger bus's load on board 2 and the hold switch's gate.
        c.add(Part::Resistor {
            a: TRIG,
            b: T2,
            r: k.r65,
        });
        c.add(Part::Capacitor {
            a: T2,
            b: N10,
            c: k.c13,
        });
        c.add(Part::Resistor {
            a: T2,
            b: N10,
            r: k.r34,
        });
        let d = D1N4004;
        c.add(Part::Diode {
            a: GN,
            k: T2,
            is: d.is,
            nvt: d.n * vt(25.0),
            cap: CR5_CAP,
            tt: CR5_TT,
        });
        c.add(Part::Resistor {
            a: GN,
            b: GIN,
            r: k.r31,
        });
        c.add(Part::Resistor {
            a: GN,
            b: G13,
            r: k.r64,
        });
        c.add_jfet(C6, G13, GIN, &k.jfet);
        // The pitch bus.
        c.add(Part::Capacitor {
            a: BUS,
            b: GND,
            c: k.c9,
        });
        c.add(Part::Resistor {
            a: P10,
            b: BUS,
            r: k.r53,
        });
        // The amplifier.
        c.add_bjt(C23, BUS, E23, &k.q23, 25.0, false);
        c.add_bjt(C14, FB, E23, &k.q23, 25.0, false);
        c.add(Part::Resistor {
            a: E23,
            b: N10,
            r: k.r52,
        });
        c.add(Part::Resistor {
            a: P10,
            b: C23,
            r: k.r58,
        });
        c.add(Part::Resistor {
            a: P10,
            b: C14,
            r: k.r51,
        });
        c.add_bjt(KO, C23, P10, &k.q24, 25.0, true);
        c.add(Part::Resistor {
            a: P10,
            b: KO,
            r: k.r59,
        });
        c.add(Part::Resistor {
            a: KO,
            b: N10,
            r: k.r54,
        });
        // R61 and GLIDE in series (the pot's node carries nothing else).
        let r_glide = c.add(Part::Resistor {
            a: KO,
            b: GIN,
            r: k.r61,
        });
        // The hold capacitor and the follower.
        c.add(Part::Capacitor {
            a: C6,
            b: GND,
            c: k.c6,
        });
        c.add(Part::Resistor {
            a: C6,
            b: G10,
            r: k.r21,
        });
        c.add_jfet(D10, G10, OUT, &k.jfet);
        c.add(Part::Resistor {
            a: P10,
            b: D10,
            r: k.r2,
        });
        c.add(Part::Resistor {
            a: OUT,
            b: N10,
            r: k.r18,
        });
        c.add(Part::Resistor {
            a: OUT,
            b: FB,
            r: k.r30,
        });
        let r_load = c.add(Part::Resistor {
            a: OUT,
            b: LOADV,
            r: LOAD_DEFAULT.r,
        });
        let mut kb = Keyboard {
            circuit,
            net: c,
            rate,
            string_current,
            r_top,
            r_mid,
            r_bot,
            r_lo,
            r_hi,
            r_trig,
            r_glide,
            r_load,
            keys: None,
            trig: false,
            glide: 0.0,
            load: LOAD_DEFAULT,
            quiet: 0,
            pending: 0,
            last: [0.0; 4],
            since_change: usize::MAX,
            fast: false,
            quality: Quality::NoCompromises,
            subdivided: 0,
            failed: 0,
        };
        if kb.net.dc().is_err() {
            kb.failed += 1;
        }
        kb.last = [kb.net.v(C6), kb.net.v(OUT), kb.net.v(T2), kb.net.v(G13)];
        kb
    }

    /// The string's current, A (the volts per key over the string's 10 ohm).
    pub fn string_current(&self) -> f64 {
        self.string_current
    }

    /// The pitch bus's voltage with key `k` held alone and nothing drawn from it.
    pub fn key_voltage(&self, k: usize) -> f64 {
        self.string_current * self.circuit.r_string * k as f64
    }

    /// The output, V.
    /// The nodal circuit (for profiles).
    pub fn circuit(&self) -> &Circuit {
        &self.net
    }

    /// The quality mode, from the next sample ([`tolerance`]; switchable while it plays).
    pub fn set_quality(&mut self, q: Quality) {
        if q != self.quality {
            self.quality = q;
            self.net.tolerance = tolerance(q);
            self.net.predict_limit = (q == Quality::Potato).then_some(PREDICT_LIMIT);
            self.net.step_iterations = if q == Quality::Potato {
                STEP_ITERATIONS
            } else {
                100
            };
        }
    }

    /// Ticks at `rate` Hz from the next one (the voice's sub-rate for it: Potato); the
    /// time pending at the old rate is stepped first.
    pub fn set_rate(&mut self, rate: f64) {
        if rate != self.rate {
            self.flush();
            self.rate = rate;
        }
    }

    pub fn out(&self) -> f64 {
        self.net.v(OUT)
    }

    /// A node's voltage, for probes: "bus", "c6", "ko", "t2", "g13", "out".
    pub fn probe(&self, name: &str) -> Option<f64> {
        let n = match name {
            "bus" => BUS,
            "c6" => C6,
            "ko" => KO,
            "t2" => T2,
            "g13" => G13,
            "out" => OUT,
            "trig" => TRIG,
            _ => return None,
        };
        Some(self.net.v(n))
    }

    /// GLIDE: the pot's resistance in circuit, ohm (0 with the GLIDE switch off).
    pub fn set_glide(&mut self, r: f64) {
        if r != self.glide {
            self.flush();
            self.glide = r;
            self.net
                .set_resistance(self.r_glide, self.circuit.r61 + r.max(0.0));
            self.quiet = 0;
            self.since_change = 0;
        }
    }

    /// The output's load (the oscillators' and the filter's keyboard inputs).
    pub fn set_load(&mut self, load: Load) {
        if load != self.load {
            self.flush();
            self.load = load;
            self.net.set_resistance(self.r_load, load.r);
            self.net.set(LOADV, load.v);
            self.quiet = 0;
        }
    }

    /// Sets the contacts without stepping (for [`Keyboard::settle`]): the lowest and
    /// highest keys held (their contacts join the pitch bus to the string; keys between
    /// are left out) and whether the trigger contact is closed.
    pub fn contacts(&mut self, keys: Option<(usize, usize)>, trig: bool) {
        self.apply(keys, trig);
    }

    fn apply(&mut self, keys: Option<(usize, usize)>, trig: bool) {
        let keys = keys.map(|(lo, hi)| (lo.min(KEYS - 1), hi.min(KEYS - 1).max(lo.min(KEYS - 1))));
        if keys != self.keys {
            self.flush();
            self.keys = keys;
            let rs = self.circuit.r_string;
            let seg = |n: usize| (rs * n as f64).max(SHORT);
            match keys {
                Some((lo, hi)) => {
                    self.net.set_resistance(self.r_top, seg(KEYS - 1 - hi));
                    self.net.set_resistance(self.r_mid, seg(hi - lo));
                    self.net.set_resistance(self.r_bot, seg(lo));
                    self.net.set_resistance(self.r_lo, self.circuit.contact);
                    let r = if hi > lo { self.circuit.contact } else { OPEN };
                    self.net.set_resistance(self.r_hi, r);
                }
                None => {
                    self.net.set_resistance(self.r_lo, OPEN);
                    self.net.set_resistance(self.r_hi, OPEN);
                }
            }
            self.quiet = 0;
            self.since_change = 0;
        }
        if trig != self.trig {
            self.flush();
            self.trig = trig;
            let r = if trig { self.circuit.contact } else { OPEN };
            self.net.set_resistance(self.r_trig, r);
            self.quiet = 0;
            self.since_change = 0;
        }
    }

    /// Steps over the time left pending while quiet.
    fn flush(&mut self) {
        if self.pending > 0 {
            let h = self.pending as f64 / self.rate;
            self.pending = 0;
            self.step(h);
        }
    }

    /// One step of `h` s; a step whose Newton iterations fail is halved, down to 1/1024 of
    /// it (then the output holds for the rest of the step, counted in `failed`).
    fn step(&mut self, h: f64) {
        if self.net.step(h).is_ok() {
            return;
        }
        self.subdivided += 1;
        if !self.halve(h, 0) {
            self.failed += 1;
        }
    }

    fn halve(&mut self, h: f64, depth: usize) -> bool {
        for _ in 0..2 {
            if self.net.step(h / 2.0).is_err() && (depth >= 10 || !self.halve(h / 2.0, depth + 1)) {
                return false;
            }
        }
        true
    }

    /// One output sample: the contacts for this sample (the lowest and highest keys held,
    /// the trigger contact), the output, V.
    pub fn tick(&mut self, keys: Option<(usize, usize)>, trig: bool) -> f64 {
        self.apply(keys, trig);
        if self.quiet >= QUIET_STEPS {
            self.pending += 1;
            if self.pending < BLOCK_MAX {
                return self.net.v(OUT);
            }
            let h = self.pending as f64 / self.rate;
            self.pending = 0;
            self.step(h);
        } else if self.fast || (self.since_change as f64) < ACTIVE_S * self.rate {
            let n = substeps(self.quality);
            for _ in 0..n {
                self.step(1.0 / (self.rate * n as f64));
            }
        } else {
            self.step(1.0 / self.rate);
        }
        self.since_change = self.since_change.saturating_add(1);
        // Quiet: 2 ms after the last change, with the hold capacitor, the output, the
        // trigger bus's node and the hold switch's gate still (the pitch bus may drift
        // while the switch holds: slowly, and stepping in blocks follows it).
        let now = [
            self.net.v(C6),
            self.net.v(OUT),
            self.net.v(T2),
            self.net.v(G13),
        ];
        let moved = (0..4).fold(0.0f64, |a, k| a.max((now[k] - self.last[k]).abs()));
        let out_moved = (now[1] - self.last[1])
            .abs()
            .max((now[0] - self.last[0]).abs());
        self.fast = out_moved > FAST_V;
        self.last = now;
        if moved < QUIET_V && (self.since_change as f64) >= ACTIVE_S * self.rate {
            self.quiet += 1;
        } else {
            self.quiet = 0;
        }
        now[1]
    }

    /// Settles the circuit with its contacts as they are: steps growing from 1 us (the
    /// path the circuit takes, which Newton's method follows where a DC solve from far
    /// away need not converge) until nothing moves, then the DC operating point.
    pub fn settle(&mut self) -> Result<(), NoConvergence> {
        self.flush();
        let mut h = 1e-6;
        let mut quiet = 0;
        for _ in 0..400 {
            let before = (self.net.v(C6), self.net.v(OUT), self.net.v(BUS));
            let failed = self.failed;
            self.step(h);
            if self.failed > failed {
                return Err(NoConvergence {
                    iterations: 0,
                    residual: f64::INFINITY,
                });
            }
            let moved = (self.net.v(C6) - before.0)
                .abs()
                .max((self.net.v(OUT) - before.1).abs())
                .max((self.net.v(BUS) - before.2).abs());
            quiet = if moved < 1e-12 { quiet + 1 } else { 0 };
            if quiet >= 3 {
                break;
            }
            h = (h * 2.0).min(1.0);
        }
        self.net.dc()?;
        self.last = [
            self.net.v(C6),
            self.net.v(OUT),
            self.net.v(T2),
            self.net.v(G13),
        ];
        self.quiet = QUIET_STEPS;
        self.since_change = usize::MAX;
        Ok(())
    }

    /// The output with key `k` held alone, settled (the circuit's state is left
    /// unchanged).
    pub fn static_out(&self, k: usize) -> Result<f64, NoConvergence> {
        let mut kb = self.clone();
        kb.apply(Some((k, k)), true);
        kb.settle()?;
        Ok(kb.net.v(OUT))
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    #[test]
    #[ignore]
    fn solver_parts() {
        let mut k = Keyboard::new(KeyboardCircuit::default(), 48_000.0);
        k.contacts(Some((20, 20)), true);
        for _ in 0..100 {
            k.tick(Some((20, 20)), true);
        }
        crate::mna::tests::time_parts(&mut k.net, 1.0 / (8.0 * 48_000.0), "keyboard");
        println!(
            "  a part is {} bytes",
            std::mem::size_of::<crate::mna::Part>()
        );
        // A sample's eight substeps with the keys changing every 1000 samples (GLIDE's
        // regime): the time a sample, and the Newton iterations.
        let reps: usize = std::env::var("MM_BENCH_REPS")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(20_000);
        let it0 = k.net.total_iterations;
        let t0 = std::time::Instant::now();
        for s in 0..reps {
            let key = if (s / 1000) % 2 == 0 { 20 } else { 32 };
            k.since_change = 0;
            std::hint::black_box(k.tick(Some((key, key)), true));
        }
        let t = t0.elapsed().as_secs_f64() / reps as f64 * 1e6;
        let its = (k.net.total_iterations - it0) as f64 / reps as f64;
        println!(
            "  eight substeps a sample: {t:.2} us a sample, {its:.2} iterations ({:.3} us each, all in)",
            t / its
        );
    }
}
