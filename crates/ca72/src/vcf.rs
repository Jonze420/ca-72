//! Board 4's filter (circuit No. 6) in real time, derived from its circuit
//! (docs/circuit/board4.md). Figure 9-11's reference designators.
//!
//! - **Control**: the control node is a passive sum of the front panel's resistors against
//!   the SCALE network (R49, R70) and Q26's base current; Q26 (PNP) and Q28 (NPN) make the
//!   ladder's current, exponential in the node's voltage ([`FilterExpo`]).
//! - **Ladder**: from the transistors' exponential law and each node's current balance, the
//!   differential voltage x_k across stage k's capacitor obeys
//!   2C dx_1/dt = -E1 (tanh(D / 2Vt) + tanh(x_1 / 2Vt)) (D: the input pair's base
//!   difference) and 2C dx_k/dt = Ek (tanh(x_(k-1) / 2Vt) - tanh(x_k / 2Vt)), where Ek is
//!   stage k's emitter current: what the stage below passes on after its base currents.
//!   Each pair's tanh sits behind its series drop (RE, RB against the base currents; for
//!   the input pair also R54 and R73/R76 against its base currents), its Vt is raised by
//!   high injection (IKF), and each stage's C carries half its nodes' junction and
//!   diffusion capacitances ([`VcfCircuit::bias`]).
//! - **Input**: the mixer's bus drives C27 into R54 and the bias chain's bottom (b5: R67,
//!   the chain above it, C24), which the emphasis current also returns to.
//! - **Output**: C5/C1 couple the top capacitor's voltage (a 15 Hz high-pass) into the
//!   followers Q8/Q6 and the pair Q7/Q5: the gain recovery amplifier, tabulated from its
//!   DC transfer ([`crate::tables::OUT_DI`]; its operating point is offset, so it clips
//!   asymmetrically, and Q8 saturates on the positive side). The followers' base currents
//!   ([`crate::tables::OUT_DIB`]) and R31/R38 load the ladder's top stage. R7's top (nt)
//!   is held by R6 and C9, not an AC ground below 100 Hz.
//! - **Emphasis**: R7/R3's junction, through C10, the EMPHASIS rheostat R14, R73 and R76,
//!   drives Q30's base.
//!
//! The whole loop (nine states: [`STATES`]) is solved without delay: trapezoidal
//! integration, Newton's method, at `oversample` times the output rate, with the ladder's
//! corner prewarped ([`Vcf::prewarp_hz`]). Left out: the output stage's transistor
//! capacitances (docs/circuit/assumptions.md A11).

use crate::devices::{Bjt, JunctionCapacitance, PairWarm, degenerated_warm, junction_capacitance};
use crate::expo::Input;
use crate::prof::Part;
use crate::resample::{Decimator, Interpolator};
use crate::tables::{OUT_DI, OUT_DI_MIN, OUT_DI_STEP, OUT_DIB, OUT_DIB_MIN, OUT_DIB_STEP};

/// TIS97 (`QTIS97`), TIS92 (`QTIS92`) and TIS93 (`QTIS93`) as in `mm-devices.lib`.
pub const TIS97: Bjt = Bjt {
    is: 5e-15,
    bf: 420.0,
    ise: 1e-14,
    ne: 1.5,
    vaf: 100.0,
    ikf: 0.1,
    br: 4.0,
    rb: 50.0,
    re: 0.5,
    rc: 1.0,
    cje: 5e-12,
    vje: 0.75,
    mje: 0.33,
    cjc: 3e-12,
    vjc: 0.75,
    mjc: 0.33,
    tf: 0.4e-9,
    xti: 3.0,
    xtb: 1.5,
    eg: 1.11,
    tnom: 25.0,
};
pub const TIS92: Bjt = Bjt {
    is: 1e-14,
    bf: 200.0,
    ise: 2e-14,
    ne: 1.5,
    vaf: 80.0,
    ikf: 0.3,
    br: 4.0,
    rb: 10.0,
    re: 0.3,
    rc: 0.5,
    cje: 15e-12,
    vje: 0.75,
    mje: 0.33,
    cjc: 8e-12,
    vjc: 0.75,
    mjc: 0.33,
    tf: 0.5e-9,
    xti: 3.0,
    xtb: 1.5,
    eg: 1.11,
    tnom: 25.0,
};
pub const TIS93: Bjt = Bjt {
    tf: 0.6e-9,
    ..TIS92
};

/// The filter's exponential converter and control node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterExpo {
    /// R39 (RANGE, 10K) wiper position from its +10 V end, 0..1; R46 68K.
    pub r39: f64,
    pub r46: f64,
    /// R49 (SCALE, 500 ohm rheostat) and R70 (1.8K) to ground.
    pub r49: f64,
    pub r70: f64,
    /// R74 (47K) from the AMOUNT OF CONTOUR pot's wiper.
    pub r74: f64,
    /// Q26's collector load R45 (330) to the negative rail; R60 (680) from the ladder's
    /// tail to Q28's collector.
    pub r45: f64,
    pub r60: f64,
    pub vp: f64,
    pub vn: f64,
    pub q26: Bjt,
    pub q28: Bjt,
    /// Where Q28's collector sits: the input pair (the ladder's transistors) under the
    /// bias chain (R4, R16, R32, R41, R52 from +10 V, R67 to ground; each stage's bases on
    /// its node), Q29's base through R54.
    pub ladder: Bjt,
    pub chain: [f64; 5],
    pub r67: f64,
    pub r54: f64,
}

impl Default for FilterExpo {
    fn default() -> Self {
        FilterExpo {
            r39: 0.5,
            r46: 68e3,
            r49: 250.0,
            r70: 1.8e3,
            r74: 47e3,
            r45: 330.0,
            r60: 680.0,
            vp: 10.0,
            vn: -10.0,
            q26: TIS93,
            q28: TIS92,
            ladder: TIS97,
            chain: [220.0, 150.0, 150.0, 150.0, 150.0],
            r67: 200.0,
            r54: 470.0,
        }
    }
}

/// The converter's operating point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpoPoint {
    /// The ladder's current, Q28's collector, A.
    pub i0: f64,
    /// The control node (Q26's base) and Q26's emitter (Q28's base), V.
    pub v_ctl: f64,
    pub v_e26: f64,
    /// Q28's collector (R60's lower end), V.
    pub v_tail: f64,
}

/// A root of an increasing function between `lo` and `hi` (f(lo) < 0 < f(hi)): secant
/// steps kept inside the bracket, bisection when they stall.
fn root(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64, tol: f64) -> f64 {
    let (mut flo, mut fhi) = (f(lo), f(hi));
    let mut x = 0.5 * (lo + hi);
    for k in 0..100 {
        let secant = lo - flo * (hi - lo) / (fhi - flo);
        x = if k % 3 == 2 || !(secant > lo && secant < hi) {
            0.5 * (lo + hi)
        } else {
            secant
        };
        let fx = f(x);
        if fx < 0.0 {
            (lo, flo) = (x, fx);
        } else {
            (hi, fhi) = (x, fx);
        }
        if hi - lo < tol || fx == 0.0 {
            return x;
        }
    }
    crate::unconverged::note(crate::unconverged::Solver::FilterNode);
    x
}

impl FilterExpo {
    /// The ladder's current (Q28's collector), A, for the control node's inputs (the front
    /// panel's resistors, R74 among them) at a temperature in C.
    pub fn current(&self, inputs: &[Input], celsius: f64) -> f64 {
        self.solve(inputs, celsius).i0
    }

    /// The input pair's emitters (the ladder's tail node) for a tail current: the bias
    /// chain's bottom, less R54's drop and Q29's base-emitter voltage at half the current.
    fn tail_node(&self, i0: f64, celsius: f64) -> f64 {
        const VBC: f64 = -0.9;
        let m = self.ladder.at(celsius);
        let q = &self.ladder;
        let half = 0.5 * i0.max(1e-15);
        let ib = m.base_current(half, VBC, q.vaf);
        // Every pair draws about 2 ib from its node (the input pair through R54 and R76 from
        // the bottom); a current drawn at a node lowers the bottom by that current times the
        // resistance above the node, times R67 over the whole chain.
        let total = self.chain.iter().sum::<f64>() + self.r67;
        let mut up = 0.0;
        let mut drawn = 0.0;
        for r in self.chain {
            up += r;
            drawn += up * 2.0 * ib;
        }
        let v_b5 = (self.vp - drawn) * self.r67 / total;
        let vbe = m.junction_voltage(half, VBC, q.vaf) + q.rb * ib + q.re * (half + ib);
        v_b5 - self.r54 * ib - vbe
    }

    /// Q28's collector and base currents and its collector voltage for its base voltage,
    /// its collector held by R60 to the ladder's tail node, in every region (saturation
    /// included): Newton's method on the two internal junction voltages, with steps
    /// limited as SPICE limits them.
    fn q28(&self, v_b: f64, celsius: f64) -> (f64, f64, f64) {
        let m = self.q28.at(celsius);
        let q = &self.q28;
        // Residuals: the external base voltage, and the collector's voltage from inside the
        // transistor against R60's.
        let res = |x: [f64; 2]| {
            let (ic, ib) = m.currents(x[0], x[1], q.vaf);
            let v_e = q.re * (ic + ib);
            let v_bi = v_e + x[0];
            let v_c = v_bi - x[1] + q.rc * ic;
            let v_tail = self.tail_node(ic, celsius) - self.r60 * ic;
            ([v_bi + q.rb * ib - v_b, v_c - v_tail], (ic, ib, v_tail))
        };
        let mut x = [(v_b - 0.02).min(0.75), -0.5];
        for _ in 0..100 {
            let (r, _) = res(x);
            let h = 1e-7;
            let (r0, _) = res([x[0] + h, x[1]]);
            let (r1, _) = res([x[0], x[1] + h]);
            let j = [
                [(r0[0] - r[0]) / h, (r1[0] - r[0]) / h],
                [(r0[1] - r[1]) / h, (r1[1] - r[1]) / h],
            ];
            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
            let dx = [
                (r[0] * j[1][1] - r[1] * j[0][1]) / det,
                (j[0][0] * r[1] - j[1][0] * r[0]) / det,
            ];
            // Junction steps at most 2 Vt up, freely down.
            let lim = |d: f64| d.clamp(-0.2, 2.0 * m.vt);
            let step = [lim(-dx[0]), lim(-dx[1])];
            x = [x[0] + step[0], x[1] + step[1]];
            if step[0].abs() < 1e-13 && step[1].abs() < 1e-13 {
                return res(x).1;
            }
        }
        crate::unconverged::note(crate::unconverged::Solver::FilterNode);
        res(x).1
    }

    /// The operating point for a control node voltage (Q26's base), and Q26's base
    /// current, which flows into the node.
    pub fn at_node(&self, v_ctl: f64, celsius: f64) -> (ExpoPoint, f64) {
        let n = self.q26.at(celsius);
        let q = &self.q26;
        let v_open = self.vp * (1.0 - self.r39);
        let r_th = 10e3 * self.r39 * (1.0 - self.r39) + self.r46;
        // For Q28's base voltage v: Q26's emitter current is R46's less Q28's base current;
        // Q26 (PNP, its collector on R45) then needs its emitter-base voltage. The residual:
        // v less that voltage less the node's.
        let side = |v: f64| {
            let (i0, ib28, v_tail) = self.q28(v, celsius);
            let ie26 = ((v_open - v) / r_th - ib28).max(1e-15);
            let vcb = self.vn + self.r45 * ie26 - v_ctl;
            let veb = root(
                |x| {
                    let (c, b) = n.currents(x, vcb, q.vaf);
                    c + b - ie26
                },
                -0.5,
                1.2,
                1e-12,
            );
            let (_, ib26) = n.currents(veb, vcb, q.vaf);
            let v_eb = veb + q.rb * ib26 + q.re * ie26;
            (
                v - v_eb - v_ctl,
                ExpoPoint {
                    i0,
                    v_ctl,
                    v_e26: v,
                    v_tail,
                },
                ib26,
            )
        };
        let v = root(|v| side(v).0, -1.5, 1.5, 1e-12);
        let (_, point, ib26) = side(v);
        (point, ib26)
    }

    /// The operating point for the control node's inputs: the node's voltage is where its
    /// resistors' currents, R49/R70's and Q26's base current balance.
    pub fn solve(&self, inputs: &[Input], celsius: f64) -> ExpoPoint {
        let (i_in, g_node) = self.node(inputs);
        let v = root(
            |v| v * g_node - i_in - self.at_node(v, celsius).1,
            -1.5,
            1.5,
            1e-12,
        );
        self.at_node(v, celsius).0
    }

    /// The control node's inputs as a Norton source: their current into the node at 0 V
    /// and the node's total conductance (R49 and R70 included).
    pub fn node(&self, inputs: &[Input]) -> (f64, f64) {
        let g_in: f64 = inputs.iter().map(|i| 1.0 / i.r).sum();
        let i_in: f64 = inputs.iter().map(|i| i.v / i.r).sum();
        (i_in, g_in + 1.0 / (self.r49 + self.r70))
    }
}

/// The converter tabulated against the control node's voltage for real time: ln I0 and
/// Q26's base current from [`FilterExpo::at_node`], built for one temperature and trim
/// setting.
#[derive(Debug, Clone)]
pub struct ExpoTable {
    pub expo: FilterExpo,
    pub celsius: f64,
    ln_i0: Vec<f64>,
    ib26: Vec<f64>,
}

/// The ladder's bias tables made so far ([`Vcf::prepare`]): one a process for each circuit
/// (EMPHASIS aside, which the bias does not read) and temperature.
static BIAS_TABLES: crate::shared::Shared<(VcfCircuit, u64), Vec<LadderBias>> =
    crate::shared::Shared::new();

/// The control node's tables made so far ([`ExpoTable::shared`]).
static EXPO_TABLES: crate::shared::Shared<(FilterExpo, u64), ExpoTable> =
    crate::shared::Shared::new();

/// The table's range and step on the control node, V.
pub const EXPO_V_MIN: f64 = -0.8;
pub const EXPO_V_STEP: f64 = 0.001;
pub const EXPO_POINTS: usize = 1601;

impl ExpoTable {
    /// [`ExpoTable::new`], made once a process for each circuit and temperature and shared.
    pub fn shared(expo: FilterExpo, celsius: f64) -> std::sync::Arc<ExpoTable> {
        EXPO_TABLES.get((expo, celsius.to_bits()), || ExpoTable::new(expo, celsius))
    }

    pub fn new(expo: FilterExpo, celsius: f64) -> ExpoTable {
        let (ln_i0, ib26) = (0..EXPO_POINTS)
            .map(|k| {
                let (p, ib) = expo.at_node(EXPO_V_MIN + EXPO_V_STEP * k as f64, celsius);
                (crate::ulp::log(p.i0), ib)
            })
            .unzip();
        ExpoTable {
            expo,
            celsius,
            ln_i0,
            ib26,
        }
    }

    /// The ladder's current for the control node's inputs as a Norton source
    /// ([`FilterExpo::node`]).
    pub fn current(&self, i_in: f64, g_node: f64) -> f64 {
        self.current_and_node(i_in, g_node).0
    }

    /// [`ExpoTable::current`] and the control node's voltage, V.
    pub fn current_and_node(&self, i_in: f64, g_node: f64) -> (f64, f64) {
        // The node's voltage: Q26's base current changes it by microvolts; two passes.
        let mut v = i_in / g_node;
        for _ in 0..2 {
            v = (i_in + table_hermite(&self.ib26, EXPO_V_MIN, EXPO_V_STEP, v).0) / g_node;
        }
        (
            crate::ulp::exp(table_hermite(&self.ln_i0, EXPO_V_MIN, EXPO_V_STEP, v).0),
            v,
        )
    }
}

/// The filter's circuit constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VcfCircuit {
    /// The ladder's capacitors C3, C7, C11, C16 (.068 uF).
    pub c: f64,
    /// The ladder's transistors (TIS97), for the current each stage loses to its bases, and
    /// their base-collector voltage (the bias chain drops about 1.5 V per stage).
    pub ladder: Bjt,
    pub ladder_vbc: f64,
    /// C27 (10 uF) and R54 (470) at the input.
    pub c27: f64,
    pub r54: f64,
    /// The bias chain's bottom (b5): R67 (200) to ground, the chain above it to +10 V
    /// (R52, R41, R32, R16, R4: 820), C24 (220 uF) to ground.
    pub r67: f64,
    pub r_chain: f64,
    pub c24: f64,
    /// The output coupling: C5/C1 (.22 uF) into R31/R38 (47K).
    pub c5: f64,
    pub r31: f64,
    /// The gain recovery amplifier's loads R3 (1K) and R7 (180), whose top (nt) R6 (47) ties
    /// to +10 V and C9 (100 uF) to ground; the amplifier's transfer is tabulated.
    pub r3: f64,
    pub r7: f64,
    pub r6: f64,
    pub c9: f64,
    /// The emphasis path: C10 (10 uF), R14 (EMPHASIS rheostat, ohms in circuit), R73 (REGEN
    /// CAL, 1K, wiper position from R76) and R76 (330).
    pub c10: f64,
    pub r14: f64,
    pub r73: f64,
    pub r73_pos: f64,
    pub r76: f64,
}

impl Default for VcfCircuit {
    fn default() -> Self {
        VcfCircuit {
            c: 0.068e-6,
            ladder: TIS97,
            ladder_vbc: -0.9,
            c27: 10e-6,
            r54: 470.0,
            r67: 200.0,
            r_chain: 820.0,
            c24: 220e-6,
            c5: 0.22e-6,
            r31: 47e3,
            r3: 1000.0,
            r7: 180.0,
            r6: 47.0,
            c9: 100e-6,
            c10: 10e-6,
            r14: 50e3,
            r73: 1000.0,
            r73_pos: 0.5,
            r76: 330.0,
        }
    }
}

/// The loop's states, in volts: the ladder's capacitors x1..x4 (Q29's side less Q30's),
/// then these.
pub const STATES: usize = 9;
/// The followers' bases, Q8's less Q6's (C5/C1 against R31/R38).
pub const W: usize = 4;
/// C10's voltage, R7/R3's junction (nu) less pin 7.
pub const Q10: usize = 5;
/// R7's top (nt: R6, C9), from rest.
pub const NT: usize = 6;
/// The bias chain's bottom (b5: R67, C24), from rest.
pub const VB: usize = 7;
/// C27's voltage, the bus less Q29's base.
pub const Q27: usize = 8;

/// What drives the loop over one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drive {
    /// The mixer bus as a Norton source: the current its switched-on channels push into
    /// it, and their total conductance.
    pub i_n: f64,
    pub g_n: f64,
    /// The ladder's bias at its current ([`VcfCircuit::bias`]).
    pub bias: LadderBias,
    /// The ladder's time scale: 1 as drawn, or the prewarping factor.
    pub p: f64,
    /// Potato: each pair's tanh taken at once with its series drop folded into its thermal
    /// voltage, not solved behind it ([`plain_pair`]).
    pub plain: bool,
}

/// The ladder's currents and its transistors' losses at one tail current.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LadderBias {
    /// The emitter currents of the four stages (the first: the input pair's collectors).
    pub e: [f64; 4],
    /// The input pair's base current change per unit of its tanh (Q29's; Q30's opposite).
    pub b_in: f64,
    /// The input pair's internal drop per unit of its tanh: RB against the base currents,
    /// RE against the emitter currents.
    pub v_in: f64,
    /// Each stage's internal drop per unit of its tanh (RE, and RB against the base
    /// currents, times the stage's current).
    pub v_stage: [f64; 4],
    /// The pairs' effective thermal voltages: Vt over the transconductance's high-injection
    /// factor at the pair's current (the input pair, then the stages).
    pub vt_in: f64,
    pub vt_stage: [f64; 4],
    /// Each stage's effective capacitance: C and half of what each of its nodes carries
    /// to the (differentially quiet) bases: the pair's base-emitter capacitance (junction
    /// and diffusion) and the collector-base capacitance of the pair below.
    pub c_stage: [f64; 4],
}

impl LadderBias {
    fn lerp(&self, o: &LadderBias, t: f64) -> LadderBias {
        let l = |a: f64, b: f64| a + (b - a) * t;
        let l4 = |a: [f64; 4], b: [f64; 4]| std::array::from_fn(|k| l(a[k], b[k]));
        LadderBias {
            e: l4(self.e, o.e),
            b_in: l(self.b_in, o.b_in),
            v_in: l(self.v_in, o.v_in),
            v_stage: l4(self.v_stage, o.v_stage),
            vt_in: l(self.vt_in, o.vt_in),
            vt_stage: l4(self.vt_stage, o.vt_stage),
            c_stage: l4(self.c_stage, o.c_stage),
        }
    }
}

/// Potato's ladder bias: [`VcfCircuit::bias`] on 1024 points of the tail current's
/// logarithm from 1e-9 to 1e-2 A, linear between (outside, the bias worked out).
const BIAS_POINTS: usize = 1024;
const BIAS_LN: (f64, f64) = (-20.723_265_836_946_41, -4.605_170_185_988_091);

impl VcfCircuit {
    /// The ladder's bias for the tail current `i0` at a temperature: each stage passes on
    /// its emitter current less its base currents (Gummel-Poon at the stage's current, the
    /// pair balanced); the pairs' small-signal base currents and series resistances.
    pub fn bias(&self, i0: f64, celsius: f64) -> LadderBias {
        let m = self.ladder.at(celsius);
        let (vbc, vaf) = (self.ladder_vbc, self.ladder.vaf);
        let mut e = [0.0; 4];
        let mut v_stage = [0.0; 4];
        let mut ie = i0.max(0.0);
        for stage in e.iter_mut() {
            let half = 0.5 * ie;
            let mut ic = half * m.bf / (m.bf + 1.0);
            for _ in 0..3 {
                ic = half - m.base_current(ic, vbc, vaf);
            }
            ie = 2.0 * ic.max(0.0);
            *stage = ie;
        }
        let mut vt_stage = [0.0; 4];
        let mut c_stage = [0.0; 4];
        let mut slopes = [0.0; 4];
        let q = &self.ladder;
        let c_mu = junction_capacitance(q.cjc, q.vjc, q.mjc, vbc);
        let c_je = JunctionCapacitance::new(q.cje, q.vje, q.mje);
        for k in 0..4 {
            let ic = 0.5 * e[k];
            let slope = m.base_slope(ic, vbc, vaf);
            slopes[k] = slope;
            v_stage[k] = (q.re + q.rb * slope / (1.0 + slope)) * e[k];
            let factor = m.gm_factor(ic, vbc, vaf);
            vt_stage[k] = m.vt / factor;
            let vbe = m.vt * crate::ulp::log(ic.max(1e-30) / m.is);
            let c_pi = c_je.at(vbe) + q.tf * ic * factor / m.vt;
            c_stage[k] = self.c + 0.5 * (c_pi + c_mu);
        }
        // The input pair is the first stage's (its slope and factor as above).
        let b_in = slopes[0] * 0.5 * e[0];
        LadderBias {
            e,
            b_in,
            v_in: 2.0 * self.ladder.rb * b_in + self.ladder.re * (e[0] + 2.0 * b_in),
            v_stage,
            vt_in: vt_stage[0],
            vt_stage,
            c_stage,
        }
    }

    /// The loop's derivatives and its output (Q7's collector, from rest) at state `y`, and,
    /// if asked, the derivatives' Jacobian.
    ///
    /// From the circuit: the bus current i reaches Q29's base through C27 and R54 into b5;
    /// the emphasis current i_f leaves nu through C10, R14, R73 and R76 into b5; the input
    /// pair sees D = R54 i - (R73a + R76) i_f. The ladder's top stage is loaded by C5/C1 and
    /// the followers' base currents; the amplifier's current (table) and i_f leave nt
    /// through R7.
    pub fn eval(
        &self,
        y: &[f64; STATES],
        d: &Drive,
        jac: Option<&mut [[f64; STATES]; STATES]>,
    ) -> ([f64; STATES], f64) {
        self.eval_from(y, d, jac, &mut [PairWarm::COLD; 5])
    }

    /// [`VcfCircuit::eval`] with the five pairs' warm starts (the input pair's, then the
    /// ladder's), which it updates.
    pub fn eval_from(
        &self,
        y: &[f64; STATES],
        d: &Drive,
        jac: Option<&mut [[f64; STATES]; STATES]>,
        zs: &mut [PairWarm; 5],
    ) -> ([f64; STATES], f64) {
        self.eval_full(y, d, jac.map(|j| (j, None)), zs)
    }

    /// [`VcfCircuit::eval_from`] with, when the Jacobian is asked for, the output's
    /// gradient too.
    fn eval_full(
        &self,
        y: &[f64; STATES],
        d: &Drive,
        jac: Option<JacobianOut<'_>>,
        zs: &mut [PairWarm; 5],
    ) -> ([f64; STATES], f64) {
        self.eval_sparse(y, d, jac, zs, true)
    }

    /// [`VcfCircuit::eval_full`]; without `zero`, the Jacobian's and the gradient's entries
    /// that are always zero are left as they are (zero already: [`Vcf::step`]'s, kept from
    /// one step to the next; they were once cleared at each evaluation).
    fn eval_sparse(
        &self,
        y: &[f64; STATES],
        d: &Drive,
        jac: Option<JacobianOut<'_>>,
        zs: &mut [PairWarm; 5],
        zero: bool,
    ) -> ([f64; STATES], f64) {
        let r73b = self.r73 * (1.0 - self.r73_pos);
        let r_g = self.r73 * self.r73_pos + self.r76;
        let r_tot = self.r7 + self.r14 + r73b + r_g;
        let r_b5 = self.r67 * self.r_chain / (self.r67 + self.r_chain);
        let k_in = 1.0 / (1.0 + d.g_n * self.r54);
        let bias = &d.bias;
        let e = bias.e;
        let (di, ddi) = table_hermite(&OUT_DI, OUT_DI_MIN, OUT_DI_STEP, y[W]);
        let (dib, ddib) = table_hermite(&OUT_DIB, OUT_DIB_MIN, OUT_DIB_STEP, y[W]);
        // Without the input pair's base currents: the bus current and the emphasis current.
        let i_in0 = (d.i_n - d.g_n * (y[Q27] + y[VB])) * k_in;
        let i_f0 = (y[NT] - self.r7 * di - y[Q10] - y[VB]) / r_tot;
        // The pair's base currents (Q29's +b u, Q30's -b u) drop across R54 and R73/R76
        // and, through them, change the bus and emphasis currents: D = A - B u.
        let a = self.r54 * i_in0 - r_g * i_f0;
        let b = bias.b_in * (self.r54 * k_in + r_g * (1.0 - r_g / r_tot)) + bias.v_in;
        let pair = |x: f64, b: f64, two_vt: f64, z: &mut PairWarm| {
            if d.plain {
                plain_pair(x, b, two_vt)
            } else {
                degenerated_warm(x, b, two_vt, z)
            }
        };
        let (ud, dud) = pair(a, b, 2.0 * bias.vt_in, &mut zs[0]);
        let c_i = d.g_n * self.r54 * bias.b_in * k_in;
        let c_f = -r_g * bias.b_in / r_tot;
        let i_in = i_in0 + c_i * ud;
        let i_f = i_f0 + c_f * ud;
        let mut u = [0.0; 4];
        let mut du = [0.0; 4];
        for s in 0..4 {
            (u[s], du[s]) = pair(
                y[s],
                bias.v_stage[s],
                2.0 * bias.vt_stage[s],
                &mut zs[s + 1],
            );
        }
        let k = [
            d.p / (2.0 * bias.c_stage[0]),
            d.p / (2.0 * bias.c_stage[1]),
            d.p / (2.0 * bias.c_stage[2]),
            d.p / (2.0 * bias.c_stage[3]),
        ];
        let load = y[W] / self.r31 + dib;
        let f3 = k[3] * (e[3] * (u[2] - u[3]) - load);
        let f = [
            -k[0] * e[0] * (ud + u[0]),
            k[1] * e[1] * (u[0] - u[1]),
            k[2] * e[2] * (u[1] - u[2]),
            f3,
            f3 - load / self.c5,
            i_f / self.c10,
            (-y[NT] / self.r6 - di - i_f) / self.c9,
            (i_in + i_f - y[VB] / r_b5) / self.c24,
            i_in / self.c27,
        ];
        let out = y[NT] - self.r7 * (di + i_f) - self.r3 * di;
        if let Some((j, gout)) = jac {
            if zero {
                *j = [[0.0; STATES]; STATES];
            }
            let gi = -d.g_n * k_in;
            let mut di_in = [0.0; STATES];
            di_in[Q27] = gi;
            di_in[VB] = gi;
            let mut di_f = [0.0; STATES];
            di_f[NT] = 1.0 / r_tot;
            di_f[W] = -self.r7 * ddi / r_tot;
            di_f[Q10] = -1.0 / r_tot;
            di_f[VB] = -1.0 / r_tot;
            let mut d_ud = [0.0; STATES];
            for n in [W, Q10, NT, VB, Q27] {
                d_ud[n] = dud * (self.r54 * di_in[n] - r_g * di_f[n]);
            }
            for n in [W, Q10, NT, VB, Q27] {
                di_in[n] += c_i * d_ud[n];
                di_f[n] += c_f * d_ud[n];
                j[0][n] = -k[0] * e[0] * d_ud[n];
            }
            j[0][0] = -k[0] * e[0] * du[0];
            j[1][0] = k[1] * e[1] * du[0];
            j[1][1] = -k[1] * e[1] * du[1];
            j[2][1] = k[2] * e[2] * du[1];
            j[2][2] = -k[2] * e[2] * du[2];
            j[3][2] = k[3] * e[3] * du[2];
            j[3][3] = -k[3] * e[3] * du[3];
            let load_w = 1.0 / self.r31 + ddib;
            j[3][W] = -k[3] * load_w;
            j[W][2] = j[3][2];
            j[W][3] = j[3][3];
            j[W][W] = j[3][W] - load_w / self.c5;
            for n in [W, Q10, NT, VB, Q27] {
                j[Q10][n] = di_f[n] / self.c10;
                let d_di = if n == W { ddi } else { 0.0 };
                let d_nt = if n == NT { 1.0 / self.r6 } else { 0.0 };
                j[NT][n] = (-d_nt - d_di - di_f[n]) / self.c9;
                let d_vb = if n == VB { 1.0 / r_b5 } else { 0.0 };
                j[VB][n] = (di_in[n] + di_f[n] - d_vb) / self.c24;
                j[Q27][n] = di_in[n] / self.c27;
            }
            if let Some(g) = gout {
                if zero {
                    *g = [0.0; STATES];
                }
                for n in [W, Q10, NT, VB, Q27] {
                    g[n] = -self.r7 * di_f[n];
                }
                g[NT] += 1.0;
                g[W] -= (self.r7 + self.r3) * ddi;
            }
        }
        (f, out)
    }
}

/// A pair's tanh and its slope with its series drop `b` folded into its thermal voltage,
/// tanh(x / (2Vt + b)): the drop's own curvature left out (Potato). The drops are a
/// small part of 2Vt. Its tanh is Potato's ([`crate::fast::tanh`]).
fn plain_pair(x: f64, b: f64, two_vt: f64) -> (f64, f64) {
    let g = 1.0 / (two_vt + b);
    let u = crate::fast::tanh(x * g);
    (u, (1.0 - u * u) * g)
}

/// Solves (I - hh J) x = r for the loop's Jacobian `j` by its structure
/// (performance): stages 1 to 3 and W follow stage 0 along the ladder, which leaves stage 0
/// and the input side (Q10, NT, VB, Q27) as a 5 by 5 system. The same solution as the
/// dense elimination to rounding. With `RECIP` (Potato) each pivot's reciprocal is taken
/// once and multiplied by, not divided by each time (performance).
fn solve_loop<const RECIP: bool>(
    j: &[[f64; STATES]; STATES],
    hh: f64,
    r: &[f64; STATES],
) -> [f64; STATES] {
    let a = |i: usize, k: usize| if i == k { 1.0 } else { 0.0 } - hh * j[i][k];
    // x1 = p1 + q1 x0, x2 = p2 + q2 x0, x3 = p3 + q3 x0 + s3 xW.
    let (a11, a22, a33) = (a(1, 1), a(2, 2), a(3, 3));
    let (p1, q1, p2, q2, p3, q3, s3);
    let a21 = a(2, 1);
    let a32 = a(3, 2);
    if RECIP {
        let (i11, i22, i33) = (1.0 / a11, 1.0 / a22, 1.0 / a33);
        (p1, q1) = (r[1] * i11, -a(1, 0) * i11);
        (p2, q2) = ((r[2] - a21 * p1) * i22, -a21 * q1 * i22);
        (p3, q3, s3) = ((r[3] - a32 * p2) * i33, -a32 * q2 * i33, -a(3, W) * i33);
    } else {
        (p1, q1) = (r[1] / a11, -a(1, 0) / a11);
        (p2, q2) = ((r[2] - a21 * p1) / a22, -a21 * q1 / a22);
        (p3, q3, s3) = ((r[3] - a32 * p2) / a33, -a32 * q2 / a33, -a(3, W) / a33);
    }
    // W's row: xW = pW + qW x0.
    let (aw2, aw3) = (a(W, 2), a(W, 3));
    let dw = a(W, W) + aw3 * s3;
    let (pw, qw) = if RECIP {
        let iw = 1.0 / dw;
        (
            (r[W] - aw2 * p2 - aw3 * p3) * iw,
            -(aw2 * q2 + aw3 * q3) * iw,
        )
    } else {
        (
            (r[W] - aw2 * p2 - aw3 * p3) / dw,
            -(aw2 * q2 + aw3 * q3) / dw,
        )
    };
    // Stage 0 and the input side.
    const IN: [usize; 4] = [Q10, NT, VB, Q27];
    let mut m = [[0.0; 5]; 5];
    let mut b = [0.0; 5];
    let a0w = a(0, W);
    m[0][0] = a(0, 0) + a0w * qw;
    b[0] = r[0] - a0w * pw;
    for (c, &n) in IN.iter().enumerate() {
        m[0][c + 1] = a(0, n);
    }
    for (row, &k) in IN.iter().enumerate() {
        let akw = a(k, W);
        m[row + 1][0] = akw * qw;
        b[row + 1] = r[k] - akw * pw;
        for (c, &n) in IN.iter().enumerate() {
            m[row + 1][c + 1] = a(k, n);
        }
    }
    let u = solve::<5, RECIP>(m, b);
    let x0 = u[0];
    let xw = pw + qw * x0;
    let mut x = [0.0; STATES];
    x[0] = x0;
    x[1] = p1 + q1 * x0;
    x[2] = p2 + q2 * x0;
    x[3] = p3 + q3 * x0 + s3 * xw;
    x[W] = xw;
    for (c, &n) in IN.iter().enumerate() {
        x[n] = u[c + 1];
    }
    x
}

/// Where [`VcfCircuit::eval_full`] puts the Jacobian and, if asked, the output's gradient.
type JacobianOut<'a> = (
    &'a mut [[f64; STATES]; STATES],
    Option<&'a mut [f64; STATES]>,
);

/// The default prewarping limit, Hz ([`Vcf::prewarp_hz`]).
pub const PREWARP_HZ: f64 = 16e3;

/// The filter in real time.
#[derive(Debug, Clone)]
pub struct Vcf {
    pub circuit: VcfCircuit,
    pub expo: FilterExpo,
    pub celsius: f64,
    /// Prewarping: the ladder's time constants are scaled so that the trapezoidal rule
    /// puts its corner (I1 / 4 C Vt, where the resonance lies) at the circuit's frequency,
    /// for corners up to this frequency in Hz; above it, the audio band is kept closer by
    /// prewarping at this frequency instead. 0 turns prewarping off
    /// (`docs/circuit/numerics.md`).
    pub prewarp_hz: f64,
    oversample: usize,
    h: f64,
    y: [f64; STATES],
    /// The derivatives at the last step's end (the trapezoidal rule's left side).
    f_prev: Option<[f64; STATES]>,
    /// The ladder's bias for the last (i0, celsius).
    bias: (f64, f64, LadderBias),
    up_n: Interpolator,
    up_g: Interpolator,
    down: Decimator,
    /// Another oversampling and its resamplers ([`Vcf::prepare`], [`Vcf::set_oversample`]).
    other: usize,
    other_rs: (Interpolator, Interpolator, Decimator),
    /// Potato: its ladder bias table (made by [`Vcf::prepare`] for the ladder and
    /// temperature then) and Newton's tolerance ([`Vcf::set_quality`]).
    bias_table: std::sync::Arc<Vec<LadderBias>>,
    bias_key: [u64; 2],
    potato: bool,
    /// The loop's elimination by its pivots' reciprocals (every mode but No Compromises).
    recip: bool,
    tol: f64,
    /// Newton iterations taken in the last output sample (for tests and benchmarks).
    pub last_iterations: usize,
    /// The pairs' last solutions: warm starts for the next evaluation.
    zs: [PairWarm; 5],
    /// The prewarping factor for the present bias, and the limit it was taken at.
    warp: Option<(f64, f64)>,
    /// The loop's Jacobian and the output's gradient, kept from step to step: the entries
    /// that are always zero stay zero, and the rest are written at each evaluation.
    jac: [[f64; STATES]; STATES],
    gout: [f64; STATES],
    /// The bus's conductance as last given, how many samples in a row it has been the same,
    /// and its resampler's outputs then: once the resampler's whole line holds it, the same
    /// outputs again (decisions.md R11).
    g_held: (f64, usize, [f64; 8]),
}

/// The loop Jacobian's entries that are not always zero ([`VcfCircuit::eval_full`]), row by
/// row in column order.
const JAC_ROWS: [&[usize]; STATES] = [
    &[0, W, Q10, NT, VB, Q27],
    &[0, 1],
    &[1, 2],
    &[2, 3, W],
    &[2, 3, W],
    &[W, Q10, NT, VB, Q27],
    &[W, Q10, NT, VB, Q27],
    &[W, Q10, NT, VB, Q27],
    &[W, Q10, NT, VB, Q27],
];

/// Samples the bus's conductance must hold before its resampler's outputs are taken as
/// held: at least its longest halfband's taps (79), at the sample rate.
const G_HOLD: usize = 80;

impl Vcf {
    pub fn new(sample_rate: f64, oversample: usize) -> Vcf {
        let up_n = Interpolator::new(oversample);
        let os = up_n.factor();
        Vcf {
            circuit: VcfCircuit::default(),
            expo: FilterExpo::default(),
            celsius: 25.0,
            prewarp_hz: PREWARP_HZ,
            oversample: os,
            h: 1.0 / (sample_rate * os as f64),
            y: [0.0; STATES],
            f_prev: None,
            bias: (f64::NAN, f64::NAN, VcfCircuit::default().bias(0.0, 25.0)),
            up_n,
            up_g: Interpolator::new(os),
            down: Decimator::new(os),
            other: os,
            other_rs: (
                Interpolator::new(os),
                Interpolator::new(os),
                Decimator::new(os),
            ),
            bias_table: std::sync::Arc::new(Vec::new()),
            bias_key: [0; 2],
            potato: false,
            recip: false,
            tol: 1e-10,
            last_iterations: 0,
            zs: [PairWarm::COLD; 5],
            warp: None,
            jac: [[0.0; STATES]; STATES],
            gout: [0.0; STATES],
            g_held: (f64::NAN, 0, [0.0; 8]),
        }
    }

    /// Prepares a second oversampling factor (1, 2, 4 or 8) to switch to while it plays
    /// ([`Vcf::set_oversample`]; not on the audio thread: its resamplers are made here).
    /// They are Potato's: sparse halfbands ([`crate::resample::Halfband::sparse`]).
    pub fn prepare(&mut self, oversample: usize) {
        let up = Interpolator::sparse(oversample);
        self.other = up.factor();
        self.other_rs = (
            up,
            Interpolator::sparse(oversample),
            Decimator::sparse(oversample),
        );
        let (a, b) = BIAS_LN;
        let (circuit, celsius) = (self.circuit, self.celsius);
        let key = VcfCircuit {
            r14: 0.0,
            ..circuit
        };
        self.bias_table = BIAS_TABLES.get((key, celsius.to_bits()), || {
            (0..BIAS_POINTS)
                .map(|k| {
                    let ln = a + (b - a) * k as f64 / (BIAS_POINTS - 1) as f64;
                    circuit.bias(libm::exp(ln), celsius)
                })
                .collect()
        });
        self.bias_key = [self.celsius.to_bits(), self.circuit.c.to_bits()];
    }

    /// The quality mode, from the next sample (switchable while it plays): in Potato the
    /// ladder's bias from its table, the pairs' tanh taken at once ([`plain_pair`]) and each
    /// step at most two Newton iterations to 1e-4 V (from twenty to 1e-10: one alone
    /// lets a self-oscillating filter stray).
    pub fn set_quality(&mut self, q: crate::voice::Quality) {
        self.recip = q != crate::voice::Quality::NoCompromises;
        let potato = q == crate::voice::Quality::Potato;
        if potato != self.potato {
            self.potato = potato;
            self.tol = if potato { 1e-4 } else { 1e-10 };
            // (The bias taken again at the next sample.)
            self.bias.0 = f64::NAN;
        }
    }

    /// The ladder's bias for `i0` from Potato's table, if it has it.
    fn table_bias(&self, i0: f64) -> Option<LadderBias> {
        if self.bias_table.is_empty()
            || self.bias_key != [self.celsius.to_bits(), self.circuit.c.to_bits()]
            || i0 <= 0.0
        {
            return None;
        }
        let (a, b) = BIAS_LN;
        let f = (libm::log(i0) - a) / (b - a) * (BIAS_POINTS - 1) as f64;
        if !(0.0..(BIAS_POINTS - 1) as f64).contains(&f) {
            return None;
        }
        let i = f as usize;
        Some(self.bias_table[i].lerp(&self.bias_table[i + 1], f - i as f64))
    }

    /// Runs at this oversampling from the next sample, if it is this one's or the prepared
    /// one's (otherwise nothing changes): the loop's states carry on; the resamplers start
    /// again from silence (Potato).
    pub fn set_oversample(&mut self, oversample: usize) {
        if oversample == self.oversample || oversample != self.other {
            return;
        }
        let rate_h = self.h * self.oversample as f64;
        std::mem::swap(&mut self.up_n, &mut self.other_rs.0);
        std::mem::swap(&mut self.up_g, &mut self.other_rs.1);
        std::mem::swap(&mut self.down, &mut self.other_rs.2);
        self.other = self.oversample;
        self.oversample = oversample;
        self.h = rate_h / oversample as f64;
        self.up_n.reset();
        self.up_g.reset();
        self.down.reset();
        self.f_prev = None;
        self.warp = None;
        self.g_held = (f64::NAN, 0, [0.0; 8]);
    }

    pub fn reset(&mut self) {
        self.zs = [PairWarm::COLD; 5];
        self.y = [0.0; STATES];
        self.f_prev = None;
        self.up_n.reset();
        self.up_g.reset();
        self.down.reset();
        self.g_held = (f64::NAN, 0, [0.0; 8]);
    }

    /// Delay through the resamplers, in output samples.
    pub fn latency(&self) -> f64 {
        self.up_n.delay() + self.down.delay()
    }

    /// The loop's state (see [`STATES`]).
    pub fn state(&self) -> &[f64; STATES] {
        &self.y
    }

    /// The output's change from its resting voltage (Q7's collector), in volts, for one
    /// output sample of the mixer bus (the Norton current `i_bus` its switched-on channels
    /// push into it, and their total conductance `g_bus`) and the ladder current `i0`.
    pub fn tick(&mut self, i_bus: f64, g_bus: f64, i0: f64) -> f64 {
        let mut laps = crate::prof::Laps::start();
        let mut ns = [0.0f64; 8];
        let mut gs = [0.0f64; 8];
        self.up_n.push(i_bus, &mut ns);
        let (g, n, held) = self.g_held;
        if g_bus.to_bits() == g.to_bits() && n >= G_HOLD {
            gs = held;
        } else {
            self.up_g.push(g_bus, &mut gs);
            let n = if g_bus.to_bits() == g.to_bits() {
                n + 1
            } else {
                0
            };
            self.g_held = (g_bus, n, gs);
        }
        laps.lap(Part::FilterResample);
        let mut out = 0.0;
        let mut iters = 0;
        for k in 0..self.oversample {
            let v = self.step(ns[k], gs[k].max(0.0), i0, &mut iters, &mut laps);
            if let Some(o) = self.down.push(v) {
                out = o;
            }
            laps.lap(Part::FilterResample);
        }
        self.last_iterations = iters;
        out
    }

    fn drive(&mut self, i_n: f64, g_n: f64, i0: f64) -> Drive {
        if self.bias.0 != i0 || self.bias.1 != self.celsius {
            let from_table = if self.potato {
                self.table_bias(i0)
            } else {
                None
            };
            let bias = from_table.unwrap_or_else(|| self.circuit.bias(i0, self.celsius));
            self.bias = (i0, self.celsius, bias);
            self.warp = None;
        }
        let bias = self.bias.2;
        // The prewarping factor follows the bias (and the rate and limit).
        if let Some((hz, p)) = self.warp
            && hz == self.prewarp_hz
        {
            return Drive {
                i_n,
                g_n,
                bias,
                p,
                plain: self.potato,
            };
        }
        let p = if self.prewarp_hz > 0.0 {
            let corner = bias.e[0] / (4.0 * bias.c_stage[0] * bias.vt_stage[0]);
            let theta = (corner.min(2.0 * core::f64::consts::PI * self.prewarp_hz) * self.h / 2.0)
                .clamp(1e-9, 1.4);
            crate::ulp::tan(theta) / theta
        } else {
            1.0
        };
        self.warp = Some((self.prewarp_hz, p));
        Drive {
            i_n,
            g_n,
            bias,
            p,
            plain: self.potato,
        }
    }

    /// One trapezoidal step of the whole loop, solved by Newton's method.
    fn step(
        &mut self,
        i_n: f64,
        g_n: f64,
        i0: f64,
        iters: &mut usize,
        laps: &mut crate::prof::Laps,
    ) -> f64 {
        let d = self.drive(i_n, g_n, i0);
        laps.lap(Part::FilterBias);
        let c = self.circuit;
        let h = self.h;
        let y0 = self.y;
        let f_old = match self.f_prev {
            Some(f) => f,
            None => c.eval_from(&y0, &d, None, &mut self.zs).0,
        };
        let mut y = y0;
        for i in 0..STATES {
            y[i] = y0[i] + h * f_old[i];
        }
        let mut done = None;
        // (Potato: at most two iterations, the last's solution then taken to first order as
        // a converged one's, not evaluated again.)
        let iterations = if self.potato { 2 } else { 20 };
        for it in 0..iterations {
            *iters += 1;
            let (fy, out) = c.eval_sparse(
                &y,
                &d,
                Some((&mut self.jac, Some(&mut self.gout))),
                &mut self.zs,
                false,
            );
            laps.lap(Part::FilterEval);
            let mut r = [0.0; STATES];
            for i in 0..STATES {
                r[i] = y[i] - y0[i] - 0.5 * h * (f_old[i] + fy[i]);
            }
            let dy = if self.recip {
                solve_loop::<true>(&self.jac, 0.5 * h, &r)
            } else {
                solve_loop::<false>(&self.jac, 0.5 * h, &r)
            };
            let mut big = 0.0f64;
            for i in 0..STATES {
                y[i] -= dy[i];
                big = big.max(dy[i].abs());
            }
            laps.lap(Part::FilterSolve);
            if big < self.tol || (self.potato && it + 1 == iterations) {
                done = Some((fy, out, dy));
                break;
            }
        }
        if done.is_none() && !self.potato {
            crate::unconverged::note(crate::unconverged::Solver::FilterLoop);
        }
        self.y = y;
        // The derivatives and output at the solution from the last iteration's, to first
        // order: the step was under 1e-10, so what that leaves is far below rounding
        // (performance: one evaluation of the loop fewer a step).
        let (f_new, out) = match done {
            Some((fy, out, dy)) => {
                // (Each row's terms in the order the full product took them, its zero entries
                // left out: they take nothing away.)
                let mut f = fy;
                for (i, cols) in JAC_ROWS.iter().enumerate() {
                    for &k in *cols {
                        f[i] -= self.jac[i][k] * dy[k];
                    }
                }
                let mut o = out;
                for (g, d) in self.gout[W..].iter().zip(&dy[W..]) {
                    o -= g * d;
                }
                (f, o)
            }
            None => c.eval_from(&y, &d, None, &mut self.zs),
        };
        self.f_prev = Some(f_new);
        laps.lap(Part::FilterSolve);
        out
    }
}

/// A table's value and slope at x by cubic Hermite interpolation (Catmull-Rom slopes);
/// beyond its ends, continued along the end slope.
pub fn table_hermite(t: &[f64], min: f64, step: f64, x: f64) -> (f64, f64) {
    let n = t.len();
    let u = (x - min) / step;
    if u <= 0.0 {
        let s = (t[1] - t[0]) / step;
        return (t[0] + s * (x - min), s);
    }
    if u >= (n - 1) as f64 {
        let s = (t[n - 1] - t[n - 2]) / step;
        return (t[n - 1] + s * (x - min - step * (n - 1) as f64), s);
    }
    let i = (u.floor() as usize).min(n - 2);
    let f = u - i as f64;
    let m = |k: usize| {
        let a = if k == 0 { t[0] } else { t[k - 1] };
        let b = if k + 1 >= n { t[n - 1] } else { t[k + 1] };
        let span = if k == 0 || k + 1 >= n { 1.0 } else { 2.0 };
        (b - a) / span
    };
    let (p0, p1, m0, m1) = (t[i], t[i + 1], m(i), m(i + 1));
    let (f2, f3) = (f * f, f * f * f);
    let v = (2.0 * f3 - 3.0 * f2 + 1.0) * p0
        + (f3 - 2.0 * f2 + f) * m0
        + (-2.0 * f3 + 3.0 * f2) * p1
        + (f3 - f2) * m1;
    let d = (6.0 * f2 - 6.0 * f) * p0
        + (3.0 * f2 - 4.0 * f + 1.0) * m0
        + (-6.0 * f2 + 6.0 * f) * p1
        + (3.0 * f2 - 2.0 * f) * m1;
    (v, d / step)
}

/// Solves m x = r by Gaussian elimination with partial pivoting (with `RECIP`, each
/// pivot's reciprocal multiplied by).
fn solve<const N: usize, const RECIP: bool>(mut m: [[f64; N]; N], mut r: [f64; N]) -> [f64; N] {
    let mut inv = [0.0; N];
    for col in 0..N {
        let mut p = col;
        for i in col + 1..N {
            if m[i][col].abs() > m[p][col].abs() {
                p = i;
            }
        }
        m.swap(col, p);
        r.swap(col, p);
        let d = m[col][col];
        if d.abs() < 1e-300 {
            continue;
        }
        inv[col] = 1.0 / d;
        let pivot = m[col];
        for i in col + 1..N {
            let f = if RECIP {
                m[i][col] * inv[col]
            } else {
                m[i][col] / d
            };
            if f != 0.0 {
                for (a, p) in m[i][col..].iter_mut().zip(&pivot[col..]) {
                    *a -= f * p;
                }
                r[i] -= f * r[col];
            }
        }
    }
    let mut x = [0.0; N];
    for i in (0..N).rev() {
        let mut s = r[i];
        for k in i + 1..N {
            s -= m[i][k] * x[k];
        }
        x[i] = if m[i][i].abs() < 1e-300 {
            0.0
        } else if RECIP {
            s * inv[i]
        } else {
            s / m[i][i]
        };
    }
    x
}
