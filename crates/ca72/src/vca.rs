//! Board 4's VCAs and output stage (circuit No. 7) in real time, derived from its circuit
//! (docs/circuit/board4.md, `circuits/boards/board4-vca.lib`). Figure 9-11's reference
//! designators.
//!
//! - **Signal path**: the filter's output through R2 and C6 against R34 (a 1/197 divider)
//!   into the first pair Q16/Q15; its collectors (R19/R23 and the 1st VCA BAL trim R14 to
//!   the chain's node nn) drive the second pair Q14/Q13; its collectors (R18/R21 and the
//!   2nd VCA BAL trim R12 to nm; the A-440's R40 and C8 on Q14's) drive the PNP output pair
//!   Q12/Q17 (R8/R28 to Q1, a current source); Q17's collector current into R29 is the
//!   output, through C2 and R77 to the load. Each pair is a tanh behind its series drop
//!   (RB, RE, R8) and its source resistances against its base currents, which is also how
//!   the trims set the pairs' balance.
//! - **Tails**: Q18 from the loudness contour (R59, R37, R43; its collector through R35
//!   under the first pair: it saturates near a 5 V contour, so the gain peaks there), Q21
//!   from EXT. LOUDNESS (J3's 33K to +10 V when the jack is empty), Q1 from the chain's
//!   first node nl through R9: full Gummel-Poon transistors, solved at the control rate.
//! - **The supply chain** (R10 with C4, R11, R13, R22, R36 with C15) carries the pairs'
//!   load currents: the first pair's tail lowers nl, which raises Q1's current and so the
//!   output's resting voltage (the circuit's thump), over C4's and C15's time constants.
//!
//! Signal couplings (C6, C8, C2) and the chain (C4, C15) are states, integrated by the
//! trapezoidal rule; the pairs are algebraic. Left out: the transistors' capacitances
//! (their nodes are 100 to 400 ohm) and CR1/CR2, which R29 keeps from conducting.

use crate::devices::{
    Bjt, PairWarm, TailPair, degenerated_warm_to, solve_junctions_limited, solve_tail_pair,
};
use crate::vcf::TIS97;
use std::cell::Cell;

/// 2N4058 (`Q2N4058`) as in `mm-devices.lib`: the output pair Q12/Q17 and Q1.
pub const Q2N4058: Bjt = Bjt {
    is: 5e-15,
    bf: 200.0,
    ise: 1e-14,
    ne: 1.5,
    vaf: 60.0,
    ikf: 0.1,
    br: 4.0,
    rb: 20.0,
    re: 0.5,
    rc: 1.0,
    cje: 6e-12,
    vje: 0.75,
    mje: 0.33,
    cjc: 5e-12,
    vjc: 0.75,
    mjc: 0.33,
    tf: 0.6e-9,
    xti: 3.0,
    xtb: 1.5,
    eg: 1.11,
    tnom: 25.0,
};

/// The VCAs' circuit constants (Folkman's July 1973 values for R2, R8 and R40: the
/// owner's default, history.md).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VcaCircuit {
    /// Input: R2, C6, R34.
    pub r2: f64,
    pub c6: f64,
    pub r34: f64,
    /// First pair's loads R19 = R23 and the 1st VCA BAL trim R14 (25 ohm) with its wiper
    /// position (0..1, from R19's side).
    pub r19: f64,
    pub r14: f64,
    pub r14_pos: f64,
    /// Second pair's loads R18 = R21 and the 2nd VCA BAL trim R12 (100 ohm).
    pub r18: f64,
    pub r12: f64,
    pub r12_pos: f64,
    /// Q18: R35 (collector), R59 (base, from the contour), R37 and R43 (emitter).
    pub r35: f64,
    pub r59: f64,
    pub r37: f64,
    pub r43: f64,
    /// Q21: R33 (collector), R51 (base to ground), R42 and R44 (emitter); J3's 33K and the
    /// voltage behind it.
    pub r33: f64,
    pub r51: f64,
    pub r42: f64,
    pub r44: f64,
    pub r_j3: f64,
    pub ext: f64,
    /// The A-440's feed R40 and C8 on Q14's collector (its oscillator off: 0 V).
    pub r40: f64,
    pub c8: f64,
    /// Output stage: R9 (Q1's emitter), R8 = R28, R30 and R29 (the pair's collector
    /// loads; R29's is the output), C2, R77 and the main output's external load.
    pub r9: f64,
    pub r8: f64,
    pub r30: f64,
    pub r29: f64,
    pub c2: f64,
    pub r77: f64,
    pub load: f64,
    /// The chain: R10 (with C4 across it) from +10 V to nl, R11 to nm, R13 to nn, R22 to
    /// bb, R36 (with C15) to ground.
    pub r10: f64,
    pub c4: f64,
    pub r11: f64,
    pub r13: f64,
    pub r22: f64,
    pub r36: f64,
    pub c15: f64,
    pub vp: f64,
    pub vn: f64,
    pub pair: Bjt,
    pub out: Bjt,
}

impl Default for VcaCircuit {
    fn default() -> Self {
        VcaCircuit {
            r2: 160e3,
            c6: 0.33e-6,
            r34: 820.0,
            r19: 100.0,
            r14: 25.0,
            r14_pos: 0.5,
            r18: 330.0,
            r12: 100.0,
            r12_pos: 0.5,
            r35: 1e3,
            r59: 68e3,
            r37: 6.8e3,
            r43: 270e3,
            r33: 470.0,
            r51: 33e3,
            r42: 3.3e3,
            r44: 150e3,
            r_j3: 33e3,
            ext: 10.0,
            r40: 10e3,
            c8: 0.1e-6,
            r9: 62.0,
            r8: 4.7,
            r30: 680.0,
            r29: 680.0,
            c2: 10e-6,
            r77: 1e3,
            load: 10e3,
            r10: 120.0,
            c4: 220e-6,
            r11: 150.0,
            r13: 120.0,
            r22: 120.0,
            r36: 510.0,
            c15: 220e-6,
            vp: 10.0,
            vn: -10.0,
            pair: TIS97,
            out: Q2N4058,
        }
    }
}

/// The most passes a bias solve takes ([`Vca::solve_bias`]).
const MAX_PASSES: usize = 30;

/// How far the contour may move from where the bias was last solved before it is solved
/// again at once, V ([`Vca::tick`]).
pub const REFRESH_V: f64 = 2e-3;

/// The same for EXT. LOUDNESS's voltage, V, for a VCA whose bias is solved at an interval
/// with the jack plugged (the voice solves it every sample then, and in Potato takes it
/// from its table: [`DriveTable`]).
pub const REFRESH_EXT_V: f64 = 2e-3;

/// Newton steps per sample at most for the tails that follow the contour, from the last
/// sample's solution: they stop once converged, in a step or two while the contour moves
/// slowly. (Two steps and no more left a fast attack's tails unconverged: 34 to 42 mV off
/// the circuit's thump.)
const FAST_STEPS: usize = 60;

/// The loudness contour's level with SUSTAIN at 10 (the attack's peak, where CR3 conducts:
/// about 5 V, service manual 2.10); the level the 1st VCA balance is set at.
pub const CONTOUR_FULL: f64 = 5.0;

impl VcaCircuit {
    /// The circuit after the factory's VCA balance procedure (service manual 5.24.1 and
    /// 5.25; Folkman 1973): with the first VCA off, the 2nd VCA BAL trim R12 nulls what a
    /// signal on EXT. LOUDNESS leaks to the output; then with the contour at
    /// [`CONTOUR_FULL`], the 1st VCA BAL trim R14. The leak is measured as the output's
    /// resting voltage for 1 V less behind J3.
    pub fn calibrated(mut self) -> VcaCircuit {
        let leak = |c: VcaCircuit, cont: f64| {
            let a = Vca::new(c, 48e3, cont).rest_output(cont);
            let b = Vca::new(
                VcaCircuit {
                    ext: c.ext - 1.0,
                    ..c
                },
                48e3,
                cont,
            )
            .rest_output(cont);
            a - b
        };
        let null = |c: &mut VcaCircuit, cont: f64, set: fn(&mut VcaCircuit, f64)| {
            let (mut lo, mut hi) = (0.0, 1.0);
            set(c, lo);
            let f_lo = leak(*c, cont);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                set(c, mid);
                if (leak(*c, cont) > 0.0) == (f_lo > 0.0) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            set(c, 0.5 * (lo + hi));
        };
        null(&mut self, 0.0, |c, x| c.r12_pos = x);
        null(&mut self, CONTOUR_FULL, |c, x| c.r14_pos = x);
        self
    }
}

/// A pair's tail split: per unit of its tanh, and at rest.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct PairBias {
    /// Tail current, A; each side's collector current per half-tail (alpha).
    tail: f64,
    alpha: f64,
    /// Each base's resting current and its change per unit of the tanh.
    ib0: f64,
    b: f64,
    /// Effective thermal voltage (high injection), and the thermal voltage.
    vt: f64,
    vt0: f64,
    /// The base charge's Early factor at rest (q1 of Gummel-Poon at the resting
    /// base-collector voltage).
    q1: f64,
}

impl PairBias {
    fn lerp(&self, o: &PairBias, k: f64) -> PairBias {
        let l = |a: f64, b: f64| a + (b - a) * k;
        PairBias {
            tail: l(self.tail, o.tail),
            alpha: l(self.alpha, o.alpha),
            ib0: l(self.ib0, o.ib0),
            b: l(self.b, o.b),
            vt: l(self.vt, o.vt),
            vt0: l(self.vt0, o.vt0),
            q1: l(self.q1, o.q1),
        }
    }
}

fn pair_bias(q: &Bjt, celsius: f64, tail: f64, vbc: f64) -> PairBias {
    let m = q.at(celsius);
    let half = 0.5 * tail.max(1e-15);
    let ib0 = m.base_current(half, vbc, q.vaf);
    let slope = m.base_slope(half, vbc, q.vaf);
    PairBias {
        tail,
        alpha: half / (half + ib0),
        ib0,
        b: slope * half,
        vt: m.vt / m.gm_factor(half, vbc, q.vaf),
        vt0: m.vt,
        q1: 1.0 / (1.0 - vbc / q.vaf),
    }
}

/// The bias network's solution for one control-rate sample.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VcaBias {
    /// Tails: Q18 (first pair), Q21 (second pair), Q1 (output pair), A.
    pub i_a: f64,
    pub i_b: f64,
    pub i_c: f64,
    /// The chain's nodes nm and nn, V.
    pub nm: f64,
    pub nn: f64,
    /// Currents the pairs draw from the chain's nodes (nm, nn, bb) and Q1's base current
    /// (which flows into nl), A.
    pub draw_nm: f64,
    pub draw_nn: f64,
    pub draw_bb: f64,
    pub ib1: f64,
}

impl VcaBias {
    fn lerp(&self, o: &VcaBias, k: f64) -> VcaBias {
        let l = |a: f64, b: f64| a + (b - a) * k;
        VcaBias {
            i_a: l(self.i_a, o.i_a),
            i_b: l(self.i_b, o.i_b),
            i_c: l(self.i_c, o.i_c),
            nm: l(self.nm, o.nm),
            nn: l(self.nn, o.nn),
            draw_nm: l(self.draw_nm, o.draw_nm),
            draw_nn: l(self.draw_nn, o.draw_nn),
            draw_bb: l(self.draw_bb, o.draw_bb),
            ib1: l(self.ib1, o.ib1),
        }
    }
}

/// The signal path's result for one sample.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Path {
    o17: f64,
    v_c14: f64,
    ib16: f64,
    /// The pairs' inputs: Q16's base less bb, Q16's collector less Q15's, Q14's less Q13's.
    stages: [f64; 3],
    /// The output pair's split: the next sample's start in Potato.
    out: f64,
}

/// What the signal path takes from the bias each sample: the tails, the chain's nodes nm
/// and bb, and each pair's split ([`Vca::control`], [`Vca::signal`]). The bias reads nothing
/// of the signal path, so it can be worked out ahead, on another thread (performance).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VcaDrive {
    i_a: f64,
    i_b: f64,
    i_c: f64,
    nm: f64,
    bb: f64,
    pairs: [PairDrive; 3],
}

/// A pair's part of [`VcaDrive`] (see [`PairBias`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct PairDrive {
    alpha: f64,
    ib0: f64,
    b: f64,
    vt: f64,
    vt0: f64,
}

impl VcaDrive {
    /// Its values, as [`VcaDrive::to_array`] gives them.
    pub const LEN: usize = 20;

    /// Its values in a fixed order (for handing it between threads).
    pub fn to_array(&self) -> [f64; VcaDrive::LEN] {
        let mut x = [0.0; VcaDrive::LEN];
        x[..5].copy_from_slice(&[self.i_a, self.i_b, self.i_c, self.nm, self.bb]);
        for (k, p) in self.pairs.iter().enumerate() {
            x[5 + 5 * k..10 + 5 * k].copy_from_slice(&[p.alpha, p.ib0, p.b, p.vt, p.vt0]);
        }
        x
    }

    /// From [`VcaDrive::to_array`]'s values.
    pub fn from_array(x: &[f64; VcaDrive::LEN]) -> VcaDrive {
        VcaDrive {
            i_a: x[0],
            i_b: x[1],
            i_c: x[2],
            nm: x[3],
            bb: x[4],
            pairs: std::array::from_fn(|k| PairDrive {
                alpha: x[5 + 5 * k],
                ib0: x[6 + 5 * k],
                b: x[7 + 5 * k],
                vt: x[8 + 5 * k],
                vt0: x[9 + 5 * k],
            }),
        }
    }
}

/// Potato's bias: the VCA's drive with its chain settled, for each loudness contour
/// with EXT. LOUDNESS empty and for each contour and jack voltage with it plugged, worked
/// out once a process for a circuit (by the full solve) and shared by every VCA built on it;
/// interpolated each sample in place of the solve. Each entry: the drive ([`VcaDrive`]'s
/// values) and the draws the chain steps on (Q1's base current, nm's, nn's and bb's draws);
/// the chain's settled states (s4, bb); and the drive's and draws' slopes against each of
/// them. The chain moves slowly (tens of milliseconds) and the tails follow it (Q1's base is
/// on its node nl): the settled drive is taken to the chain as it is, to first order.
#[derive(Debug)]
pub struct DriveTable {
    empty: Vec<[f64; ENTRY]>,
    /// The jack's voltage by rows, the contour along each.
    plugged: Vec<[f64; ENTRY]>,
}

/// The drive and the draws.
const DRAWN: usize = VcaDrive::LEN + 4;
/// The drive and the draws, the settled chain, the slopes against s4 and bb.
const ENTRY: usize = 3 * DRAWN + 2;
/// The table's axes: from, to, points. The contour in 0.05 V steps with the jack empty and
/// 0.1 V steps with it plugged; the jack in 0.1 V steps.
const CONT_EMPTY: (f64, f64, usize) = (-1.0, 9.0, 201);
const CONT_PLUGGED: (f64, f64, usize) = (-1.0, 9.0, 101);
const EXT_PLUGGED: (f64, f64, usize) = (-1.0, 11.0, 121);

/// Where `x` falls on an axis: the point below it and how far towards the next.
fn place(x: f64, (from, to, n): (f64, f64, usize)) -> (usize, f64) {
    let f = ((x - from) / (to - from) * (n - 1) as f64).clamp(0.0, (n - 1) as f64);
    let i = (f as usize).min(n - 2);
    (i, f - i as f64)
}

fn lerp_entry(a: &[f64; ENTRY], b: &[f64; ENTRY], t: f64) -> [f64; ENTRY] {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}

impl DriveTable {
    /// The table for a VCA's circuit (EXT. LOUDNESS's normal contact as it has it), made
    /// once a process and shared.
    fn for_circuit(circuit: VcaCircuit, celsius: f64) -> std::sync::Arc<DriveTable> {
        type Made = Vec<(VcaCircuit, u64, std::sync::Arc<DriveTable>)>;
        static MADE: std::sync::Mutex<Made> = std::sync::Mutex::new(Vec::new());
        let mut made = MADE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, _, t)) = made
            .iter()
            .find(|(c, k, _)| *c == circuit && *k == celsius.to_bits())
        {
            return t.clone();
        }
        let t = std::sync::Arc::new(DriveTable::make(circuit, celsius));
        made.push((circuit, celsius.to_bits(), t.clone()));
        t
    }

    fn make(circuit: VcaCircuit, celsius: f64) -> DriveTable {
        let mut v = Vca::new(circuit, 48e3, CONT_EMPTY.0);
        v.celsius = celsius;
        // Settled at each point from the last one's solution.
        // The drive and draws at the chain as it is.
        let drawn = |v: &mut Vca, cont: f64| -> [f64; DRAWN] {
            v.solve_bias(cont);
            let mut e = [0.0; DRAWN];
            e[..VcaDrive::LEN].copy_from_slice(&v.drive().to_array());
            let b = v.bias;
            e[VcaDrive::LEN..].copy_from_slice(&[b.ib1, b.draw_nm, b.draw_nn, b.draw_bb]);
            e
        };
        let settled = |v: &mut Vca, cont: f64| -> [f64; ENTRY] {
            for _ in 0..60 {
                v.solve_bias(cont);
                let (s4, bb) = v.chain_equilibrium();
                let moved = (s4 - v.s4).abs() + (bb - v.bb).abs();
                (v.s4, v.bb) = (s4, bb);
                if moved < 1e-9 {
                    break;
                }
            }
            let base = drawn(v, cont);
            // The slopes against the chain's states, by differences (on copies: the sweep's
            // warm starts left as they were).
            const D: f64 = 1e-4;
            let slope = |v: &Vca, s4: f64, bb: f64| -> [f64; DRAWN] {
                let mut w = v.clone();
                (w.s4, w.bb) = (v.s4 + s4, v.bb + bb);
                let e = drawn(&mut w, cont);
                std::array::from_fn(|k| (e[k] - base[k]) / D)
            };
            let (d_s4, d_bb) = (slope(v, D, 0.0), slope(v, 0.0, D));
            let mut e = [0.0; ENTRY];
            e[..DRAWN].copy_from_slice(&base);
            e[DRAWN] = v.s4;
            e[DRAWN + 1] = v.bb;
            e[DRAWN + 2..2 * DRAWN + 2].copy_from_slice(&d_s4);
            e[2 * DRAWN + 2..].copy_from_slice(&d_bb);
            e
        };
        let at = |(from, to, n): (f64, f64, usize), i: usize| {
            from + (to - from) * i as f64 / (n - 1) as f64
        };
        let empty = (0..CONT_EMPTY.2)
            .map(|i| settled(&mut v, at(CONT_EMPTY, i)))
            .collect();
        v.circuit.r_j3 = 0.0;
        let mut plugged = Vec::with_capacity(EXT_PLUGGED.2 * CONT_PLUGGED.2);
        for j in 0..EXT_PLUGGED.2 {
            v.circuit.ext = at(EXT_PLUGGED, j);
            for i in 0..CONT_PLUGGED.2 {
                plugged.push(settled(&mut v, at(CONT_PLUGGED, i)));
            }
        }
        DriveTable { empty, plugged }
    }

    /// The entry for the contour `cont`, EXT. LOUDNESS at `ext` if plugged.
    fn lookup(&self, cont: f64, ext: Option<f64>) -> [f64; ENTRY] {
        match ext {
            None => {
                let (i, t) = place(cont, CONT_EMPTY);
                lerp_entry(&self.empty[i], &self.empty[i + 1], t)
            }
            Some(x) => {
                let (i, t) = place(cont, CONT_PLUGGED);
                let (j, u) = place(x, EXT_PLUGGED);
                let row = |j: usize| {
                    let r = &self.plugged[j * CONT_PLUGGED.2..];
                    lerp_entry(&r[i], &r[i + 1], t)
                };
                lerp_entry(&row(j), &row(j + 1), u)
            }
        }
    }
}

/// The VCAs and output stage in real time.
#[derive(Debug, Clone)]
pub struct Vca {
    pub circuit: VcaCircuit,
    pub celsius: f64,
    h: f64,
    /// States: C6's voltage (from rest), C8's (absolute), C2's (absolute), R10's drop (C4)
    /// and the node bb (C15).
    q6: f64,
    q8: f64,
    q2: f64,
    s4: f64,
    bb: f64,
    /// The A-440's output (V) at C8's far side (0 V while it is off).
    pub a440: f64,
    bias: VcaBias,
    /// Warm starts for Q18, Q21 and Q1.
    x18: [f64; 2],
    x21: [f64; 2],
    x1: [f64; 2],
    /// The three pairs' base-emitter junctions at the last solve (the first, second and
    /// output pair's): where their solves start ([`solve_tail_pair`]).
    xp: [f64; 3],
    /// The pairs' base-collector voltages at rest (from the bias solution).
    vbc: [f64; 3],
    pairs: [PairBias; 3],
    /// Q16's resting base current, which R34 carries at rest; bb at rest (C6's reference).
    ib16_rest: f64,
    bb_rest: f64,
    /// The whole bias network is solved every `control_every` samples and its slow parts
    /// (Q21, Q1, the pairs' base-current ratios, which follow the chain's states)
    /// interpolated between; Q18, which follows the contour, is solved every sample.
    pub control_every: usize,
    count: usize,
    from: (VcaBias, [PairBias; 3]),
    to: (VcaBias, [PairBias; 3]),
    /// The contour at the last full solve: once it has moved further than
    /// [`REFRESH_V`], the bias is solved again at once.
    solved_cont: f64,
    /// EXT. LOUDNESS's voltage and resistance at the last full solve.
    solved_ext: (f64, f64),
    /// The first and second pairs' emitter nodes' drops below their bases at the last
    /// full solve, and the half-tails they were taken at.
    e1_drop: (f64, f64),
    e2_drop: (f64, f64),
    /// The signal path's pairs' warm starts (performance): their last solutions and
    /// tanh anchors ([`PairWarm`]).
    warm: Cell<[PairWarm; 3]>,
    /// High Fidelity's and Potato's signal path: the pairs solved to 1e-9 (not 1e-15)
    /// and the output pair's Early effect by one Newton step from the last sample's split,
    /// which [`Vca::signal`] keeps here, not four passes from rest.
    pub plain_signal: bool,
    out_warm: Cell<Option<f64>>,
    /// Potato's bias ([`DriveTable`]; `None` until prepared) and whether it is in use.
    potato: Option<std::sync::Arc<DriveTable>>,
    pub potato_on: bool,
}

impl Vca {
    /// A VCA at `rate` Hz, at rest (its operating point solved) with the contour at
    /// `cont` V.
    pub fn new(circuit: VcaCircuit, rate: f64, cont: f64) -> Vca {
        let mut v = Vca {
            circuit,
            celsius: 25.0,
            h: 1.0 / rate,
            q6: 0.0,
            q8: 0.0,
            q2: 0.0,
            a440: 0.0,
            s4: 0.0,
            bb: 0.0,
            bias: VcaBias::default(),
            x18: [0.6, -1.0],
            x21: [0.6, -1.0],
            x1: [0.6, -1.0],
            xp: [0.6; 3],
            vbc: [-1.0, -1.0, -4.5],
            pairs: [PairBias::default(); 3],
            ib16_rest: 0.0,
            bb_rest: 0.0,
            control_every: ((rate / 3000.0).round() as usize).max(1),
            count: 0,
            from: (VcaBias::default(), [PairBias::default(); 3]),
            to: (VcaBias::default(), [PairBias::default(); 3]),
            solved_cont: f64::NAN,
            solved_ext: (f64::NAN, f64::NAN),
            e1_drop: (0.6, 1e-4),
            e2_drop: (0.6, 1e-4),
            warm: Cell::new([PairWarm::COLD; 3]),
            plain_signal: false,
            out_warm: Cell::new(None),
            potato: None,
            potato_on: false,
        };
        v.settle(cont);
        v
    }

    /// Puts every state at its equilibrium for the contour `cont` (no signal).
    pub fn settle(&mut self, cont: f64) {
        let c = self.circuit;
        // Start from the chain without loads.
        let total = c.r10 + c.r11 + c.r13 + c.r22 + c.r36;
        self.s4 = c.vp * c.r10 / total;
        self.bb = c.vp * c.r36 / total;
        for _ in 0..60 {
            self.solve_bias(cont);
            let (s4, bb) = self.chain_equilibrium();
            self.s4 = s4;
            self.bb = bb;
        }
        self.solve_bias(cont);
        let p = self.pairs[0];
        self.ib16_rest = p.ib0;
        self.bb_rest = self.bb;
        // C6 at rest: no current, R34 carries Q16's base current.
        self.q6 = c.r34 * p.ib0;
        // C8 and C2 at rest: no current through them.
        for _ in 0..8 {
            let path = self.path(
                &self.drive(),
                0.0,
                self.q6,
                self.q8 + self.a440,
                self.q2,
                None,
            );
            self.q8 = path.v_c14;
            self.q2 = path.o17;
        }
        self.count = 0;
        self.from = (self.bias, self.pairs);
        self.to = self.from;
        self.solved_cont = cont;
        self.solved_ext = (self.circuit.ext, self.circuit.r_j3);
    }

    /// The chain's node voltages nl and bb in equilibrium with the present draws.
    fn chain_equilibrium(&self) -> (f64, f64) {
        let c = self.circuit;
        let b = self.bias;
        // At rest J = s4 / R10 + ib1 flows from nl down R11 (s4: R10's drop);
        // bb = nl - (R11 + R13 + R22) J + (R13 + R22) draw_nm + R22 draw_nn, and
        // bb = R36 (J - draw_nm - draw_nn - draw_bb): linear in s4.
        let r_mid = c.r11 + c.r13 + c.r22;
        let k = c.r36;
        let lhs0 = c.vp - r_mid * b.ib1 + (c.r13 + c.r22) * b.draw_nm + c.r22 * b.draw_nn;
        let rhs0 = k * (b.ib1 - b.draw_nm - b.draw_nn - b.draw_bb);
        let s4 = (lhs0 - rhs0) / (1.0 + r_mid / c.r10 + k / c.r10);
        let j = s4 / c.r10 + b.ib1;
        let bb = k * (j - b.draw_nm - b.draw_nn - b.draw_bb);
        (s4, bb)
    }

    /// Prepares Potato's bias ([`DriveTable`]: the first VCA of a circuit in a process takes
    /// its making, a few tenths of a second; not on the audio thread).
    pub fn prepare_potato(&mut self) {
        self.potato = Some(DriveTable::for_circuit(self.circuit, self.celsius));
    }

    /// The tails and the chain's middle nodes for the contour `cont`, at the present chain
    /// states (control rate).
    pub fn solve_bias(&mut self, cont: f64) {
        let c = self.circuit;
        let (tis, pnp) = (c.pair, c.out);
        let (m_tis, m_pnp) = (tis.at(self.celsius), pnp.at(self.celsius));
        let nl = c.vp - self.s4;
        let bb = self.bb;
        let mut b = self.bias;
        let r_mid = c.r11 + c.r13 + c.r22;
        let ra2 = c.r19 + c.r14 * c.r14_pos;
        let rb2 = c.r19 + c.r14 * (1.0 - c.r14_pos);
        let ra3 = c.r18 + c.r12 * c.r12_pos;
        let rb3 = c.r18 + c.r12 * (1.0 - c.r12_pos);
        let mut vbc = self.vbc;
        // Passes over the tails until they agree to 1e-12 of their currents: each tail is
        // solved with its pair's drop at its own current (the strong coupling, which where
        // EXT. LOUDNESS overdrives Q21 made passes over a drop taken from the last pass swing
        // without settling); the pairs' base currents and the chain's nodes follow from
        // pass to pass.
        let mut last_move = f64::INFINITY;
        let mut passes = 0;
        while last_move > 1e-12 && passes < MAX_PASSES {
            passes += 1;
            let before = (b.i_a, b.i_b, b.i_c);
            let p1 = pair_bias(&tis, self.celsius, b.i_a, vbc[0]);
            let p2 = pair_bias(&tis, self.celsius, b.i_b, vbc[1]);
            let p3 = pair_bias(&pnp, self.celsius, b.i_c, vbc[2]);
            // The middle nodes from the chain's current, which its states set.
            b.draw_nn = p1.alpha * b.i_a + 2.0 * p2.ib0;
            b.draw_nm = p2.alpha * b.i_b - 2.0 * p3.ib0;
            b.draw_bb = 2.0 * p1.ib0;
            let j = (nl - bb + (c.r13 + c.r22) * b.draw_nm + c.r22 * b.draw_nn) / r_mid;
            b.nm = nl - c.r11 * j;
            b.nn = b.nm - c.r13 * (j - b.draw_nm);
            // Q18: its collector through R35 under the first pair's emitters (Q15's base
            // on bb), solved with the pair.
            // (An emitter to ground through R37 and to -10 V through R43: its Thevenin.)
            let r_e18 = 1.0 / (1.0 / c.r37 + 1.0 / c.r43);
            let tp = TailPair {
                tail: (&m_tis, &tis),
                pair: (&m_tis, &tis),
                pnp: false,
                vb0: cont,
                rb: c.r59,
                ve0: r_e18 * c.vn / c.r43,
                re: r_e18,
                mid: bb,
                pair_vbc: vbc[0],
                r_emitter: 0.0,
                r_collector: c.r35,
            };
            let (x, ic, _, e1) = solve_tail_pair(&tp, [self.x18[0], self.x18[1], self.xp[0]], 60);
            (self.x18, self.xp[0]) = ([x[0], x[1]], x[2]);
            b.i_a = ic.max(0.0);
            self.e1_drop = (bb - e1, 0.5 * b.i_a.max(1e-15));
            // Q21: base on J3's divider, collector through R33 under the second pair's
            // emitters (on the first pair's collectors), solved with the pair.
            let v_th = c.ext * c.r51 / (c.r_j3 + c.r51);
            let r_th = c.r_j3 * c.r51 / (c.r_j3 + c.r51);
            let v_c16 = b.nn - ra2 * (p1.alpha * 0.5 * b.i_a + p2.ib0);
            let v_c15 = b.nn - rb2 * (p1.alpha * 0.5 * b.i_a + p2.ib0);
            let mid2 = 0.5 * (v_c16 + v_c15);
            let r_e21 = 1.0 / (1.0 / c.r42 + 1.0 / c.r44);
            let tp = TailPair {
                vb0: v_th,
                rb: r_th,
                ve0: r_e21 * c.vn / c.r44,
                re: r_e21,
                mid: mid2,
                pair_vbc: vbc[1],
                r_collector: c.r33,
                ..tp
            };
            let (x, ic, _, e2) = solve_tail_pair(&tp, [self.x21[0], self.x21[1], self.xp[1]], 60);
            (self.x21, self.xp[1]) = ([x[0], x[1]], x[2]);
            b.i_b = ic.max(0.0);
            self.e2_drop = (mid2 - e2, 0.5 * b.i_b.max(1e-15));
            // Q1 (PNP): base on nl, emitter through R9 from +10 V, collector straight under
            // the output pair's emitters (R8 in each), solved with the pair.
            let v_c14 = b.nm - ra3 * (p2.alpha * 0.5 * b.i_b - p3.ib0);
            let v_c13 = b.nm - rb3 * (p2.alpha * 0.5 * b.i_b - p3.ib0);
            let tp = TailPair {
                tail: (&m_pnp, &pnp),
                pair: (&m_pnp, &pnp),
                pnp: true,
                vb0: nl,
                rb: 0.0,
                ve0: c.vp,
                re: c.r9,
                mid: 0.5 * (v_c14 + v_c13),
                pair_vbc: vbc[2],
                r_emitter: c.r8,
                r_collector: 0.0,
            };
            let (x, ic, ib, _) = solve_tail_pair(&tp, [self.x1[0], self.x1[1], self.xp[2]], 60);
            let half_c = 0.5 * b.i_c.max(1e-15);
            (self.x1, self.xp[2]) = ([x[0], x[1]], x[2]);
            b.i_c = ic.max(0.0);
            b.ib1 = ib;
            // The pairs' base-collector voltages (a PNP's collector-base) at rest: Q16 on
            // bb under R19; Q14 on Q16's collector under R18; Q17 on Q13's collector over
            // R29.
            vbc = [bb - v_c16, v_c16 - v_c14, c.r29 * p3.alpha * half_c - v_c13];
            let scale = b.i_a.abs() + b.i_b.abs() + b.i_c.abs() + 1e-15;
            last_move =
                ((b.i_a - before.0).abs() + (b.i_b - before.1).abs() + (b.i_c - before.2).abs())
                    / scale;
        }
        if last_move > 1e-12 {
            // (Its tails not settled to 1e-12 of their currents in `MAX_PASSES` passes.)
            crate::unconverged::note(crate::unconverged::Solver::VcaBias);
        }
        self.vbc = vbc;
        self.bias = b;
        self.pairs = [
            pair_bias(&tis, self.celsius, b.i_a, vbc[0]),
            pair_bias(&tis, self.celsius, b.i_b, vbc[1]),
            pair_bias(&pnp, self.celsius, b.i_c, vbc[2]),
        ];
    }

    /// The tails that follow the contour at once, every sample: Q18 (its collector under
    /// the first pair's emitters: bb less their drop, scaled from the last full solve) and
    /// Q21 (its collector under the second pair's emitters, which follow nn), with the
    /// chain's middle nodes; the pairs' base-current ratios are the last full solve's.
    fn fast_bias(&mut self, cont: f64) {
        let c = self.circuit;
        let tis = c.pair;
        let m = tis.at(self.celsius);
        let nl = c.vp - self.s4;
        let r_mid = c.r11 + c.r13 + c.r22;
        let ra2 = c.r19 + c.r14 * c.r14_pos;
        let rb2 = c.r19 + c.r14 * (1.0 - c.r14_pos);
        let scale = |p: PairBias, tail: f64| {
            let k = if p.tail > 1e-12 { tail / p.tail } else { 1.0 };
            PairBias {
                tail,
                ib0: p.ib0 * k,
                b: p.b * k,
                ..p
            }
        };
        // Q18.
        let (drop0, half0) = self.e1_drop;
        let half = 0.5 * self.bias.i_a.max(1e-15);
        let e1 = self.bb - (drop0 + m.vt * crate::ulp::log(half / half0));
        let q18 = |ic: f64, ib: f64| {
            let ie = ic + ib;
            let ve = (ie + c.vn / c.r43) / (1.0 / c.r37 + 1.0 / c.r43);
            (cont - c.r59 * ib, ve, e1 - c.r35 * ic)
        };
        let (x, ic, _) = solve_junctions_limited(&m, &tis, false, self.x18, q18, FAST_STEPS);
        self.x18 = x;
        let i_a = ic.max(0.0);
        self.pairs[0] = scale(self.pairs[0], i_a);
        let (p1, p3) = (self.pairs[0], self.pairs[2]);
        // The chain's middle nodes with the new draws.
        let b = &mut self.bias;
        b.i_a = i_a;
        b.draw_bb = 2.0 * p1.ib0;
        let nodes = |b: &mut VcaBias, p2: &PairBias| {
            b.draw_nn = p1.alpha * b.i_a + 2.0 * p2.ib0;
            b.draw_nm = p2.alpha * b.i_b - 2.0 * p3.ib0;
            let j = (nl - self.bb + (c.r13 + c.r22) * b.draw_nm + c.r22 * b.draw_nn) / r_mid;
            b.nm = nl - c.r11 * j;
            b.nn = b.nm - c.r13 * (j - b.draw_nm);
        };
        nodes(b, &self.pairs[1]);
        // Q21.
        let (drop2, half2) = self.e2_drop;
        let p2 = self.pairs[1];
        let v_c = b.nn - 0.5 * (ra2 + rb2) * (p1.alpha * 0.5 * i_a + p2.ib0);
        let e2 = v_c - (drop2 + m.vt * crate::ulp::log(0.5 * b.i_b.max(1e-15) / half2));
        let v_th = c.ext * c.r51 / (c.r_j3 + c.r51);
        let r_th = c.r_j3 * c.r51 / (c.r_j3 + c.r51);
        let q21 = |ic: f64, ib: f64| {
            let ie = ic + ib;
            let ve = (ie + c.vn / c.r44) / (1.0 / c.r42 + 1.0 / c.r44);
            (v_th - r_th * ib, ve, e2 - c.r33 * ic)
        };
        let (x, ic, _) = solve_junctions_limited(&m, &tis, false, self.x21, q21, FAST_STEPS);
        self.x21 = x;
        b.i_b = ic.max(0.0);
        self.pairs[1] = scale(p2, b.i_b);
        nodes(b, &self.pairs[1]);
    }

    /// The signal path for an input `v_in` (from rest) with C6 and C8 at `q6`, `q8`; or,
    /// with `w`, the first pair driven directly (Q16's base `w` above bb, no R34).
    fn path(&self, d: &VcaDrive, v_in: f64, q6: f64, q8: f64, q2: f64, w: Option<f64>) -> Path {
        let c = self.circuit;
        let b = d;
        let [p1, p2, p3] = d.pairs;
        let (tis, pnp) = (c.pair, c.out);
        // First pair: Q16's base through R34 (and R2 through C6), Q15's on bb.
        let k_in = c.r34 / (c.r2 + c.r34);
        let r_p = c.r2 * c.r34 / (c.r2 + c.r34);
        // C6's far side follows the input; its near side is bb's (which moves) plus w.
        let (a1, r_src) = match w {
            Some(w) => (w, 0.0),
            None => (
                k_in * (v_in - q6 - (d.bb - self.bb_rest)) - r_p * p1.ib0,
                r_p,
            ),
        };
        let b1 = r_src * p1.b + 2.0 * tis.rb * p1.b + tis.re * b.i_a;
        let mut warm = self.warm.get();
        let tol = if self.plain_signal { 1e-9 } else { 1e-15 };
        let (u1, _) = degenerated_warm_to(a1, b1, 2.0 * p1.vt, &mut warm[0], tol);
        let ic16 = p1.alpha * 0.5 * b.i_a * (1.0 + u1);
        let ic15 = p1.alpha * 0.5 * b.i_a * (1.0 - u1);
        let ib16 = p1.ib0 + p1.b * u1;
        // Second pair on the first's collectors (R19 + R14's part, R23 + the rest).
        let ra2 = c.r19 + c.r14 * c.r14_pos;
        let rb2 = c.r19 + c.r14 * (1.0 - c.r14_pos);
        let a2 = -ra2 * ic16 + rb2 * ic15 - (ra2 - rb2) * p2.ib0;
        let b2 = (ra2 + rb2) * p2.b + 2.0 * tis.rb * p2.b + tis.re * b.i_b;
        let (u2, _) = degenerated_warm_to(a2, b2, 2.0 * p2.vt, &mut warm[1], tol);
        let ic14 = p2.alpha * 0.5 * b.i_b * (1.0 + u2);
        let ic13 = p2.alpha * 0.5 * b.i_b * (1.0 - u2);
        // Output pair (PNP) on the second's collectors; Q14's also loaded by R40 and C8.
        let ra3 = c.r18 + c.r12 * c.r12_pos;
        let rb3 = c.r18 + c.r12 * (1.0 - c.r12_pos);
        let ra3p = ra3 * c.r40 / (ra3 + c.r40);
        let v14_open = (c.r40 * (b.nm - ra3 * ic14) + ra3 * q8) / (c.r40 + ra3);
        let a3 = v14_open - b.nm + rb3 * ic13 + (ra3p - rb3) * p3.ib0;
        let b3 = (ra3p + rb3) * p3.b + 2.0 * pnp.rb * p3.b + (pnp.re + c.r8) * b.i_c;
        // The output node: R29, and R77 and the load through C2. Each output transistor's
        // collector voltage sets its base charge (Early effect), which shifts the pair's
        // split: the emitter currents divide as e^(dVeb / Vt) qb12 / qb17.
        let r_out = c.r77 + c.load;
        let g_o = 1.0 / c.r29 + 1.0 / r_out;
        let v_c13 = b.nm - rb3 * (ic13 - p3.ib0);
        let v_c14_rest = v14_open + ra3p * p3.ib0;
        let half_c = 0.5 * b.i_c;
        let (mut o17, mut v_c12) = (c.r29 * p3.alpha * half_c, c.r30 * p3.alpha * half_c);
        let mut u3 = 0.0;
        if let (true, Some(u_w), None) = (self.plain_signal, self.out_warm.get(), w) {
            // Potato: the loop is a fixed point in the pair's split u = G(u) (the Early
            // effect shifts it by delta(u); about 0.02 of a change comes back a pass), so
            // Newton steps on it from the last sample's split, the slope worked out with
            // each: one, or two when the split moved far (a sawtooth's reset).
            let o17_at = |u: f64| (p3.alpha * half_c * (1.0 + u) + q2 / r_out) / g_o;
            let v_c12_at = |u: f64| c.r30 * p3.alpha * half_c * (1.0 - u);
            u3 = u_w;
            for _ in 0..2 {
                let (o, v) = (o17_at(u3), v_c12_at(u3));
                let qb17 = 1.0 / (1.0 - (o - v_c13) / pnp.vaf);
                let qb12 = 1.0 / (1.0 - (v - v_c14_rest) / pnp.vaf);
                let delta = p3.vt0 * crate::ulp::log(qb12 / qb17);
                let (u1, du_da) =
                    degenerated_warm_to(a3 + delta, b3, 2.0 * p3.vt, &mut warm[2], tol);
                let d_delta = p3.vt0
                    * (qb12 / pnp.vaf * (-c.r30 * p3.alpha * half_c)
                        - qb17 / pnp.vaf * (p3.alpha * half_c / g_o));
                let step = (u1 - u3) / (1.0 - du_da * d_delta);
                u3 += step;
                if step.abs() < 1e-3 {
                    break;
                }
            }
            o17 = o17_at(u3);
        } else {
            for _ in 0..4 {
                let qb17 = 1.0 / (1.0 - (o17 - v_c13) / pnp.vaf);
                let qb12 = 1.0 / (1.0 - (v_c12 - v_c14_rest) / pnp.vaf);
                let delta = p3.vt0 * crate::ulp::log(qb12 / qb17);
                u3 = degenerated_warm_to(a3 + delta, b3, 2.0 * p3.vt, &mut warm[2], tol).0;
                let ic17 = p3.alpha * half_c * (1.0 + u3);
                let ic12 = p3.alpha * half_c * (1.0 - u3);
                o17 = (ic17 + q2 / r_out) / g_o;
                v_c12 = c.r30 * ic12;
            }
        }
        self.warm.set(warm);
        let ib12 = p3.ib0 - p3.b * u3;
        let ib17 = p3.ib0 + p3.b * u3;
        let v_c14 = v14_open + ra3p * ib12;
        let w1 = a1 + r_src * (p1.ib0 - ib16);
        let ib14 = p2.ib0 + p2.b * u2;
        let ib13 = p2.ib0 - p2.b * u2;
        let d1 = -ra2 * (ic16 + ib14) + rb2 * (ic15 + ib13);
        let d2 = v_c14 - (b.nm - rb3 * (ic13 - ib17));
        Path {
            o17,
            v_c14,
            ib16,
            stages: [w1, d1, d2],
            out: u3,
        }
    }

    /// The output (Q17's collector) at rest for the contour `cont`: the operating point
    /// the calibration procedures watch.
    pub fn rest_output(&mut self, cont: f64) -> f64 {
        self.settle(cont);
        self.path(
            &self.drive(),
            0.0,
            self.q6,
            self.q8 + self.a440,
            self.q2,
            None,
        )
        .o17
    }

    /// The output (Q17's collector) with the first pair driven directly, at rest for the
    /// contour `cont`: the static transfer.
    pub fn static_output(&mut self, w: f64, cont: f64) -> f64 {
        self.settle(cont);
        // At DC: no current through C2.
        let mut o17 = self.q2;
        let d = self.drive();
        for _ in 0..8 {
            o17 = self
                .path(&d, 0.0, self.q6, self.q8 + self.a440, o17, Some(w))
                .o17;
        }
        o17
    }

    /// One sample: the filter's output `v_in` (from its rest) and the loudness contour
    /// `cont` (V) in; the main output's voltage (after C2 and R77, across the load) out.
    pub fn tick(&mut self, v_in: f64, cont: f64) -> f64 {
        let d = self.control(cont);
        self.signal(v_in, &d)
    }

    /// What the signal path takes from the bias at present.
    pub fn drive(&self) -> VcaDrive {
        let b = &self.bias;
        VcaDrive {
            i_a: b.i_a,
            i_b: b.i_b,
            i_c: b.i_c,
            nm: b.nm,
            bb: self.bb,
            pairs: self.pairs.map(|p| PairDrive {
                alpha: p.alpha,
                ib0: p.ib0,
                b: p.b,
                vt: p.vt,
                vt0: p.vt0,
            }),
        }
    }

    /// A sample's bias for the loudness contour `cont` (V), and the chain stepped over it:
    /// the first half of [`Vca::tick`]. It reads nothing [`Vca::signal`] changes, so a copy
    /// of the VCA can run it ahead of the one that runs the signal path.
    pub fn control(&mut self, cont: f64) -> VcaDrive {
        if self.potato_on
            && let Some(t) = &self.potato
        {
            // Potato: the settled drive from the table, the chain stepped on its draws and
            // its own bb kept (the thump at attacks). Back in another mode the bias is solved
            // again at once.
            let plugged = (self.circuit.r_j3 == 0.0).then_some(self.circuit.ext);
            let t = t.lookup(cont, plugged);
            // Taken from the settled chain to the chain as it is.
            let (ds4, dbb) = (self.s4 - t[DRAWN], self.bb - t[DRAWN + 1]);
            let e: [f64; DRAWN] =
                std::array::from_fn(|k| t[k] + t[DRAWN + 2 + k] * ds4 + t[2 * DRAWN + 2 + k] * dbb);
            let mut x = [0.0; VcaDrive::LEN];
            x.copy_from_slice(&e[..VcaDrive::LEN]);
            let mut d = VcaDrive::from_array(&x);
            d.bb = self.bb;
            let k = VcaDrive::LEN;
            (
                self.bias.ib1,
                self.bias.draw_nm,
                self.bias.draw_nn,
                self.bias.draw_bb,
            ) = (e[k], e[k + 1], e[k + 2], e[k + 3]);
            self.step_chain();
            self.solved_cont = f64::NAN;
            return d;
        }
        let every = self.control_every.max(1);
        // The slow parts follow the contour a block behind, interpolated: right while it
        // moves slowly, but a fast attack or release left them far from the tails that
        // follow it every sample (a spike of up to 2 V at the output against the circuit's
        // 118 mV thump). Once the contour has moved more than REFRESH_V since the last
        // solve, the bias is solved again at once and taken as it is.
        // (Likewise EXT. LOUDNESS, plugged or moving.)
        let moved = (cont - self.solved_cont).abs() > REFRESH_V
            || (self.circuit.ext - self.solved_ext.0).abs() > REFRESH_EXT_V
            || self.circuit.r_j3 != self.solved_ext.1;
        // Solved this sample and taken whole (the contour moved, or the bias is solved every
        // sample): the whole network's solution, tails included.
        let whole = moved || every == 1;
        if whole || self.count.is_multiple_of(every) {
            let before = self.to;
            let (x18, x21) = (self.x18, self.x21);
            let fast = self.bias;
            self.solve_bias(cont);
            self.to = (self.bias, self.pairs);
            if whole {
                self.from = self.to;
            } else {
                (self.x18, self.x21) = (x18, x21);
                self.from = before;
                self.bias = fast;
            }
            self.solved_cont = cont;
            self.solved_ext = (self.circuit.ext, self.circuit.r_j3);
        }
        self.count += 1;
        if !whole {
            let k = ((self.count - 1) % every + 1) as f64 / every as f64;
            // The slow parts interpolated; the tails that follow the contour kept and
            // resolved.
            let (i_a, i_b) = (self.bias.i_a, self.bias.i_b);
            self.bias = VcaBias {
                i_a,
                i_b,
                ..self.from.0.lerp(&self.to.0, k)
            };
            for i in 0..3 {
                self.pairs[i] = self.from.1[i].lerp(&self.to.1[i], k);
            }
            self.fast_bias(cont);
        }
        let d = self.drive();
        // The chain, with the present draws.
        self.step_chain();
        d
    }

    /// A sample of the signal path, the filter's output `v_in` (from its rest) in, on the
    /// bias `d` ([`Vca::control`]'s): the second half of [`Vca::tick`]. The main output's
    /// voltage out.
    pub fn signal(&mut self, v_in: f64, d: &VcaDrive) -> f64 {
        let c = self.circuit;
        let h = self.h;
        let path = self.path(d, v_in, self.q6, self.q8 + self.a440, self.q2, None);
        self.out_warm.set(self.plain_signal.then_some(path.out));
        // C6: dq6/dt = (v_in - (bb - bb at rest) - q6 + R34 ib16) / ((R2 + R34) C6).
        let a6 = 1.0 / ((c.r2 + c.r34) * c.c6);
        let drive6 = (v_in - (d.bb - self.bb_rest) + c.r34 * path.ib16) * a6;
        self.q6 = (self.q6 * (1.0 - 0.5 * h * a6) + h * drive6) / (1.0 + 0.5 * h * a6);
        // C8: dq8/dt = (v_c14 - q8 - a440) / (R40 C8), v_c14 linear in R40's far side
        // (q8 + a440).
        let ra3 = c.r18 + c.r12 * c.r12_pos;
        let g = ra3 / (c.r40 + ra3);
        let k8 = path.v_c14 - g * self.q8 - self.a440;
        let a8 = (1.0 - g) / (c.r40 * c.c8);
        self.q8 = (self.q8 * (1.0 - 0.5 * h * a8) + h * k8 / (c.r40 * c.c8)) / (1.0 + 0.5 * h * a8);
        // C2 and the load.
        let r_out = c.r77 + c.load;
        let a2 = 1.0 / (r_out * c.c2);
        let main = c.load * (path.o17 - self.q2) / r_out;
        // (C2's update below uses o17 with this step's q2: its time constant is 110 ms.)
        self.q2 = (self.q2 * (1.0 - 0.5 * h * a2) + h * path.o17 * a2) / (1.0 + 0.5 * h * a2);
        main
    }

    /// The chain's two states over one step: backward Euler on its linear network (its
    /// time constants are tens of milliseconds, the step microseconds).
    fn step_chain(&mut self) {
        let c = self.circuit;
        let b = self.bias;
        let h = self.h;
        // d s4/dt = (J - ib1 - s4 / R10) / C4 with J the current down R11:
        // J = (nl - bb + R13 dnm + R22 (dnm + dnn)) / (R11 + R13 + R22), nl = vp - s4;
        // d bb/dt = (J - dnm - dnn - dbb - bb / R36) / C15.
        let r_mid = c.r11 + c.r13 + c.r22;
        let j0 = (c.vp + c.r13 * b.draw_nm + c.r22 * (b.draw_nm + b.draw_nn)) / r_mid;
        // J = j0 - s4 / r_mid - bb / r_mid.
        let m = [
            [
                1.0 + h / c.c4 * (1.0 / r_mid + 1.0 / c.r10),
                h / c.c4 / r_mid,
            ],
            [
                h / c.c15 / r_mid,
                1.0 + h / c.c15 * (1.0 / r_mid + 1.0 / c.r36),
            ],
        ];
        let r = [
            self.s4 + h / c.c4 * (j0 - b.ib1),
            self.bb + h / c.c15 * (j0 - b.draw_nm - b.draw_nn - b.draw_bb),
        ];
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        self.s4 = (r[0] * m[1][1] - m[0][1] * r[1]) / det;
        self.bb = (m[0][0] * r[1] - m[1][0] * r[0]) / det;
    }

    /// The output (Q17's collector) and the pairs' inputs (see [`Path`]) for an input
    /// `v_in` at the present states, without advancing them (for tests).
    pub fn probe(&self, v_in: f64) -> (f64, [f64; 3]) {
        let p = self.path(
            &self.drive(),
            v_in,
            self.q6,
            self.q8 + self.a440,
            self.q2,
            None,
        );
        (p.o17, p.stages)
    }

    /// The chain's nodes nl, nm, nn, bb (V), for tests.
    pub fn chain(&self) -> [f64; 4] {
        [
            self.circuit.vp - self.s4,
            self.bias.nm,
            self.bias.nn,
            self.bb,
        ]
    }

    pub fn bias(&self) -> VcaBias {
        self.bias
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// The full solve swept slowly up EXT. LOUDNESS and down again: the tails at the same
    /// voltages each way (a difference: more than one solution, the solve's warm start
    /// choosing).
    #[test]
    #[ignore]
    fn the_bias_under_a_slow_sweep_up_and_down() {
        let mut c = VcaCircuit::default().calibrated();
        c.r_j3 = 0.0;
        c.ext = 0.0;
        let cont = 4.0;
        let mut v = Vca::new(c, 48e3, cont);
        v.control_every = 1;
        let steps = 48_000;
        let mut up = Vec::new();
        for i in 0..=steps {
            v.circuit.ext = 8.0 * i as f64 / steps as f64;
            v.tick(0.0, cont);
            up.push((v.circuit.ext, v.bias.i_a, v.bias.i_b, v.bias.i_c, v.bb));
        }
        let mut worst = (0.0f64, 0.0);
        for i in (0..=steps).rev() {
            v.circuit.ext = 8.0 * i as f64 / steps as f64;
            v.tick(0.0, cont);
            let u = up[i];
            let d = ((v.bias.i_b - u.2).abs() / u.2.abs().max(1e-9))
                .max((v.bias.i_c - u.3).abs() / u.3.abs());
            if d > worst.0 {
                worst = (d, u.0);
            }
            if i % 4800 == 0 {
                println!(
                    "ext {:.2}: i_b {:.4e} up, {:.4e} down; i_c {:.4e} up, {:.4e} down; bb {:.4} up, {:.4} down",
                    u.0, u.2, v.bias.i_b, u.3, v.bias.i_c, u.4, v.bb
                );
            }
        }
        println!(
            "worst relative difference {:.3e} at {:.3} V",
            worst.0, worst.1
        );
    }

    /// Potato's signal path (`plain_signal`) against the full one on the same bias: sines
    /// and a sawtooth's steps at quiet and loud contours, within 1 uV of the output.
    #[test]
    fn plain_signal_follows_the_full_one() {
        let rate = 48e3;
        let c = VcaCircuit::default().calibrated();
        let mut worst = 0.0f64;
        for cont in [1.0, 4.0, 8.0] {
            for (hz, amp, saw) in [
                (220.0, 0.5, false),
                (5000.0, 2.0, false),
                (110.0, 3.0, true),
            ] {
                let mut full = Vca::new(c, rate, cont);
                let mut plain = full.clone();
                plain.plain_signal = true;
                let (mut dy, mut peak) = (0.0f64, 0.0f64);
                for i in 0..(rate as usize / 2) {
                    let ph = (i as f64 * hz / rate).fract();
                    let x = if saw {
                        amp * (2.0 * ph - 1.0)
                    } else {
                        amp * (2.0 * std::f64::consts::PI * ph).sin()
                    };
                    let (a, b) = (full.tick(x, cont), plain.tick(x, cont));
                    dy = dy.max((a - b).abs());
                    peak = peak.max(a.abs());
                }
                eprintln!(
                    "contour {cont} V, {hz} Hz at {amp} V: peak {peak:.3} V, within {dy:.2e} V"
                );
                worst = worst.max(dy);
            }
        }
        assert!(worst < 1e-6, "{worst:e}");
    }

    /// Potato's table against the full solve with EXT. LOUDNESS held at 4 V, then sweeping
    /// (the worst case's 5 Hz between 0 and 8 V), the contour held: the output and the
    /// chain's states.
    #[test]
    #[ignore]
    fn drive_table_under_a_sweep() {
        let rate = 48e3;
        let mut c = VcaCircuit::default().calibrated();
        c.r_j3 = 0.0;
        c.ext = 4.0;
        for (sweep, cont) in [(false, 2.0), (true, 2.0), (true, 5.0)] {
            let mut full = Vca::new(c, rate, cont);
            full.control_every = 1;
            let mut pot = full.clone();
            pot.prepare_potato();
            pot.potato_on = true;
            let (mut dy, mut ds4, mut dbb, mut peak) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            for i in 0..(rate as usize) {
                let t = i as f64 / rate;
                let ext = if sweep {
                    4.0 + 4.0 * (2.0 * std::f64::consts::PI * 5.0 * t).sin()
                } else {
                    4.0
                };
                full.circuit.ext = ext;
                pot.circuit.ext = ext;
                let x = 0.5 * (2.0 * std::f64::consts::PI * 220.0 * t).sin();
                let (a, b) = (full.tick(x, cont), pot.tick(x, cont));
                if i > 4800 {
                    dy = dy.max((a - b).abs());
                    peak = peak.max(a.abs());
                    ds4 = ds4.max((full.s4 - pot.s4).abs());
                    dbb = dbb.max((full.bb - pot.bb).abs());
                }
            }
            println!(
                "sweep {sweep}, cont {cont}: output peak {peak:.4}, difference {dy:.4e}; chain s4 {ds4:.3e} V, bb {dbb:.3e} V"
            );
        }
    }

    /// Potato's table against the full solve, settled, at points on and between its grid.
    #[test]
    #[ignore]
    fn drive_table_matches_the_solve() {
        let c = VcaCircuit::default().calibrated();
        let t = DriveTable::for_circuit(c, 25.0);
        for ext in [None, Some(2.0), Some(5.05), Some(6.5), Some(8.0)] {
            for cont in [0.0, 2.02, 4.0, 6.0] {
                let mut cc = c;
                if let Some(x) = ext {
                    cc.ext = x;
                    cc.r_j3 = 0.0;
                }
                let mut v = Vca::new(cc, 48e3, cont);
                v.settle(cont);
                let d = v.drive().to_array();
                let e = t.lookup(cont, ext);
                let worst = (0..VcaDrive::LEN)
                    .map(|k| (d[k] - e[k]).abs() / d[k].abs().max(1e-12))
                    .fold(0.0f64, f64::max);
                println!(
                    "ext {ext:?} cont {cont}: i_a {:.4e}/{:.4e} i_b {:.4e}/{:.4e} i_c {:.4e}/{:.4e}; worst relative {worst:.2e}",
                    d[0], e[0], d[1], e[1], d[2], e[2]
                );
            }
        }
    }

    /// The VCA's time a sample with EXT. LOUDNESS plugged (the bias solved every sample),
    /// and of that the bias solve's.
    #[test]
    #[ignore]
    fn vca_parts() {
        let mut v = Vca::new(VcaCircuit::default().calibrated(), 48e3, 0.0);
        v.circuit.ext = 5.0;
        v.circuit.r_j3 = 0.0;
        v.control_every = 1;
        let reps = 200_000;
        let cont = |i: usize| 4.0 + 3.0 * (i as f64 * 0.001).sin();
        let t0 = std::time::Instant::now();
        for i in 0..reps {
            v.circuit.ext = 5.0 + 2.0 * (i as f64 * 0.003).sin();
            std::hint::black_box(v.tick(0.3 * (i as f64 * 0.05).sin(), cont(i)));
        }
        let tick = t0.elapsed().as_secs_f64() / reps as f64 * 1e6;
        let t0 = std::time::Instant::now();
        for i in 0..reps {
            v.circuit.ext = 5.0 + 2.0 * (i as f64 * 0.003).sin();
            v.solve_bias(cont(i));
            std::hint::black_box(&v.bias);
        }
        let bias = t0.elapsed().as_secs_f64() / reps as f64 * 1e6;
        println!("vca: {tick:.2} us a sample, the bias solve {bias:.2} us");
    }
}
