//! A small nodal solver for the board-level circuits that are quiet most of the time and
//! move only after a key or a switch (the keyboard circuit): the circuit's nodes solved
//! by Newton's method with ngspice's device equations, capacitances as charges, backward
//! Euler steps. Dense and allocation-free after construction; tens of nodes at most.
//!
//! Conventions: node 0 is ground; nodes `1..held` are held at voltages the caller sets
//! (supplies, a key's contact); the rest are solved. Every solved node has GMIN to ground
//! and every junction GMIN across it, as SPICE adds them.

use crate::devices::BjtAt;

pub type Node = usize;
pub const GND: Node = 0;

/// SPICE's GMIN, S.
pub const GMIN: f64 = 1e-12;

/// The most solved nodes a circuit may have.
pub const MAX_SOLVED: usize = 64;

/// An N-channel JFET: ngspice's level 1 (Shichman-Hodges, B = 1), at 25 C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Jfet {
    pub vto: f64,
    pub beta: f64,
    pub lambda: f64,
    pub rd: f64,
    pub rs: f64,
    /// Gate junctions: zero-bias capacitances, built-in potential, grading (ngspice's
    /// level 1 fixes M at 0.5), forward-bias coefficient, saturation current, emission.
    pub cgs: f64,
    pub cgd: f64,
    pub pb: f64,
    pub fc: f64,
    pub is: f64,
    pub n: f64,
}

/// 2N4303 as `J2N4303` in `mm-devices.lib`.
pub const J2N4303: Jfet = Jfet {
    vto: -3.5,
    beta: 5.1e-4,
    lambda: 0.005,
    rd: 10.0,
    rs: 10.0,
    cgs: 5e-12,
    cgd: 3e-12,
    pb: 0.6,
    fc: 0.5,
    is: 1e-14,
    n: 1.0,
};

impl Jfet {
    /// Drain current and its slopes (d/dvgs, d/dvds) at the intrinsic terminals, vds >= 0.
    fn intrinsic(&self, vgs: f64, vds: f64) -> (f64, f64, f64) {
        let vgst = vgs - self.vto;
        if vgst <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let l = 1.0 + self.lambda * vds;
        if vds < vgst {
            let core = vds * (2.0 * vgst - vds);
            (
                self.beta * core * l,
                self.beta * 2.0 * vds * l,
                self.beta * (2.0 * vgst - 2.0 * vds) * l + self.beta * core * self.lambda,
            )
        } else {
            (
                self.beta * vgst * vgst * l,
                self.beta * 2.0 * vgst * l,
                self.beta * vgst * vgst * self.lambda,
            )
        }
    }

    /// Drain-to-source current through the channel and its series resistances, with its
    /// slopes against the external vgs and vds (vds >= 0; the source resistance first):
    /// Newton on the current.
    fn forward(&self, vgs: f64, vds: f64, r_s: f64, r_d: f64) -> (f64, f64, f64) {
        let (i0, g0, d0) = self.intrinsic(vgs, vds);
        if r_s + r_d == 0.0 {
            return (i0, g0, d0);
        }
        let mut i = i0.min(vds / (r_s + r_d));
        for _ in 0..50 {
            let (f, fg, fd) = self.intrinsic(vgs - r_s * i, (vds - (r_s + r_d) * i).max(0.0));
            let g = i - f;
            let dg = 1.0 + r_s * fg + (r_s + r_d) * fd;
            let step = g / dg;
            i -= step;
            if step.abs() <= 1e-15 + 1e-12 * i.abs() {
                break;
            }
        }
        let (_, fg, fd) = self.intrinsic(vgs - r_s * i, (vds - (r_s + r_d) * i).max(0.0));
        let dg = 1.0 + r_s * fg + (r_s + r_d) * fd;
        (i, fg / dg, fd / dg)
    }

    /// The channel current from drain to source and its slopes against the gate, drain and
    /// source voltages, either way round (the channel is symmetrical).
    pub fn channel(&self, vg: f64, vd: f64, vs: f64) -> (f64, [f64; 3]) {
        if vd >= vs {
            let (i, g, d) = self.forward(vg - vs, vd - vs, self.rs, self.rd);
            (i, [g, d, -g - d])
        } else {
            // The drain acts as the source.
            let (i, g, d) = self.forward(vg - vd, vs - vd, self.rd, self.rs);
            (-i, [-g, g + d, -d])
        }
    }
}

/// A depletion (junction) capacitance with SPICE's charge: zero-bias capacitance,
/// built-in potential, grading, forward-bias coefficient; plus a transit time's diffusion
/// charge on the junction's current.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Depletion {
    pub cj0: f64,
    pub vj: f64,
    pub m: f64,
    pub fc: f64,
}

impl Depletion {
    /// [`Depletion::charge`] with `a^-m` from the reference `r` (an earlier `a` and its
    /// power, exact) while `a` is within 1/256 of it relatively: the binomial series in
    /// the relative difference to degree 6 (the next term under 1e-17); further away it is
    /// evaluated exactly and becomes the reference. Between Newton's iterates and a
    /// circuit's substeps a junction's voltage moves far less than that
    /// (performance).
    /// Its constants `k` are [`Depletion::consts`]'.
    #[inline]
    pub fn charge_near(&self, v: f64, r: &mut [f64; 2], k: &DepletionConsts) -> (f64, f64) {
        let (cj0, vj, m, fc) = (self.cj0, self.vj, self.m, self.fc);
        if v < fc * vj {
            let a = 1.0 - v / vj;
            let e = (a - r[0]) / r[0];
            let s = if e.abs() < 1.0 / 256.0 {
                let [c1, c2, c3, c4, c5, c6] = k.series;
                r[1] * (1.0 + e * (c1 + e * (c2 + e * (c3 + e * (c4 + e * (c5 + e * c6))))))
            } else {
                let s = crate::ulp::exp(-m * crate::ulp::log(a));
                *r = [a, s];
                s
            };
            (cj0 * vj * (1.0 - a * s) / (1.0 - m), cj0 * s)
        } else {
            self.beyond(v, &k.beyond)
        }
    }

    /// The constants of [`Depletion::charge_near`], worked out once per part, not per load
    /// (performance): the same operations on the same values, so the same bits.
    pub fn consts(&self) -> DepletionConsts {
        let m = self.m;
        // (1 + e)^-m = sum of C(-m, k) e^k.
        let (c1, c2) = (-m, -m * (-m - 1.0) / 2.0);
        let c3 = c2 * (-m - 2.0) / 3.0;
        let c4 = c3 * (-m - 3.0) / 4.0;
        let c5 = c4 * (-m - 4.0) / 5.0;
        let c6 = c5 * (-m - 5.0) / 6.0;
        DepletionConsts {
            series: [c1, c2, c3, c4, c5, c6],
            beyond: self.beyond_consts(),
        }
    }

    /// SPICE's F1, F2 and F3 for the linear capacitance beyond FC*VJ.
    fn beyond_consts(&self) -> [f64; 3] {
        let (vj, m, fc) = (self.vj, self.m, self.fc);
        let f1 = vj * (1.0 - crate::ulp::pow(1.0 - fc, 1.0 - m)) / (1.0 - m);
        let f2 = crate::ulp::pow(1.0 - fc, 1.0 + m);
        let f3 = 1.0 - fc * (1.0 + m);
        [f1, f2, f3]
    }

    /// The charge and capacitance beyond FC*VJ (`v` there) with [`Depletion::beyond_consts`].
    #[inline]
    fn beyond(&self, v: f64, &[f1, f2, f3]: &[f64; 3]) -> (f64, f64) {
        let (cj0, vj, m, fc) = (self.cj0, self.vj, self.m, self.fc);
        let x = v - fc * vj;
        let q = cj0 * (f1 + (f3 * x + m / (2.0 * vj) * (v * v - (fc * vj) * (fc * vj))) / f2);
        (q, cj0 / f2 * (f3 + m * v / vj))
    }

    /// Charge and capacitance at a junction voltage.
    pub fn charge(&self, v: f64) -> (f64, f64) {
        let (cj0, vj, m, fc) = (self.cj0, self.vj, self.m, self.fc);
        if v < fc * vj {
            let a = 1.0 - v / vj;
            // a^-m as exp(-m ln a): within an ulp or two of libm's pow, in half its time
            // (performance).
            let s = crate::ulp::exp(-m * crate::ulp::log(a));
            (cj0 * vj * (1.0 - a * s) / (1.0 - m), cj0 * s)
        } else {
            // Linear capacitance beyond FC*VJ (SPICE's extrapolation).
            self.beyond(v, &self.beyond_consts())
        }
    }
}

/// A depletion charge's constants ([`Depletion::consts`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DepletionConsts {
    /// The binomial series' coefficients.
    pub series: [f64; 6],
    /// SPICE's F1, F2 and F3 for the linear capacitance beyond FC*VJ.
    pub beyond: [f64; 3],
}

/// A circuit's parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    Resistor {
        a: Node,
        b: Node,
        r: f64,
    },
    Capacitor {
        a: Node,
        b: Node,
        c: f64,
    },
    /// A pn junction (anode, cathode): SPICE's diode current `is (exp(v/nvt) - 1)`, its
    /// depletion charge and the transit time's diffusion charge.
    Diode {
        a: Node,
        k: Node,
        is: f64,
        nvt: f64,
        cap: Depletion,
        tt: f64,
    },
    /// A depletion capacitance alone (anode, cathode).
    Junction {
        a: Node,
        k: Node,
        cap: Depletion,
    },
    /// A junction's diffusion charge alone, `tt is (exp(v/vt) - 1)` (anode, cathode): a
    /// bipolar transistor's forward transit time on its base-emitter junction.
    Diffusion {
        a: Node,
        k: Node,
        is: f64,
        vt: f64,
        tt: f64,
    },
    /// A bipolar transistor: Gummel-Poon currents (`BjtAt::currents`) at its terminals
    /// (series resistances left out), with its forward Early voltage.
    Bjt {
        c: Node,
        b: Node,
        e: Node,
        m: BjtAt,
        vaf: f64,
        pnp: bool,
    },
    /// An N-channel JFET's channel (its gate junctions are separate `Diode`s).
    Jfet {
        d: Node,
        g: Node,
        s: Node,
        m: Jfet,
    },
}

/// The solver failed to converge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoConvergence {
    pub iterations: usize,
    pub residual: f64,
}

/// A circuit and its solution.
#[derive(Debug, Clone)]
pub struct Circuit {
    pub parts: Vec<Part>,
    /// Each resistor's conductance (0 for other parts), kept with its resistance.
    g: Vec<f64>,
    /// Each part's references for its junction charges' powers in the load
    /// ([`Depletion::charge_near`]).
    near: Vec<[[f64; 2]; 3]>,
    /// The parts whose junctions are limited (diodes, transistors, JFETs), and those that
    /// hold a charge (capacitors, junctions, diffusion charges, diodes): the only ones a
    /// solve's start and a step's acceptance change (performance; the others' limiting
    /// voltages, charges and charge currents stay 0).
    limiting: Vec<u32>,
    charged: Vec<u32>,
    /// Each part's depletion charge's constants ([`Depletion::consts`]; zero for parts
    /// without one).
    consts: Vec<DepletionConsts>,
    nodes: usize,
    held: usize,
    /// Node voltages (the held ones as set), and a copy to restore after a failed solve.
    v: Vec<f64>,
    saved: Vec<f64>,
    /// Each part's stored charge at the last accepted point, and (for the theta method)
    /// its current there.
    q: Vec<f64>,
    i_prev: Vec<f64>,
    /// The integration: 1 is backward Euler (the default: L-stable, for stiff circuits such
    /// as the keyboard's), 0.5 the trapezoidal rule (second order, for circuits whose
    /// response near the step's frequency matters), between them the theta method.
    theta: f64,
    /// Each part's junction voltages at the last Newton iterate, for SPICE's limiting.
    lim: Vec<[f64; 2]>,
    /// Each junction part's critical voltage for the limiting, taken when it was added
    /// (performance: it was a log and a square root an iteration).
    vcrit: Vec<f64>,
    jac: Vec<f64>,
    /// The elimination's recorded structure ([`Circuit::linear_solve`]).
    lu: LuProgram,
    f: Vec<f64>,
    dx: Vec<f64>,
    /// Newton's tolerance: a solve has converged when every node's last correction is
    /// within the first plus the second times its voltage, V (1e-9 and 1e-9 unless a
    /// quality mode sets it).
    pub tolerance: (f64, f64),
    /// With a limit (Potato's keyboard), a step starts from its extrapolation only if that
    /// moves no node more than the limit, V: after a jump (a contact changed) the quadratic
    /// extrapolation overshoots, and Newton took 50 to 80 limited iterations from there.
    pub predict_limit: Option<f64>,
    /// A step's most Newton iterations before it fails (100; fewer for Potato's keyboard,
    /// whose failed steps are halved: two halves converge in fewer than one slow step).
    pub step_iterations: usize,
    /// The last solve's Newton iterations.
    pub iterations: usize,
    /// Counts since construction: steps and DC solves, and their Newton iterations (for
    /// profiles).
    pub solves: u64,
    pub total_iterations: u64,
    /// Linear solves whose elimination was worked out again from a column (a pivot moved,
    /// or none recorded yet), and that column summed over them.
    pub lu_recorded: u64,
    pub lu_recorded_from: u64,
    /// Solves by their Newton iterations: 1, 2, 3, 4, 5 to 8, 9 to 16, 17 to 32, more
    /// (and failed).
    pub iteration_counts: [u64; 8],
    /// The time its solves took, ns, and of that its loads' and linear solves' (counted
    /// with the `profile` feature only).
    pub nanos: u64,
    pub nanos_load: u64,
    pub nanos_lu: u64,
    /// The voltages at the two accepted points before the last, and the two steps that
    /// led from them (0: none), for the predictor ([`Circuit::step`]).
    prev: Vec<f64>,
    prev2: Vec<f64>,
    prev_h: f64,
    prev2_h: f64,
    /// Each charge's value and capacitance at the last load, and the part's voltage there
    /// (for taking the accepted point's charges without evaluating them again).
    qc: Vec<[f64; 3]>,
    qc_ok: bool,
    /// Each part's stamps' positions in the residual and the Jacobian (up to three stamps
    /// a part, a transistor's), and the Jacobian's pattern for a DC solve and for a step
    /// (`rows_in`, `cols_in` as the load leaves them): worked out once, from the parts
    /// (performance; the same additions in the same order).
    slots: Vec<[Slot; 3]>,
    pattern: [([u64; MAX_SOLVED], [u64; MAX_SOLVED]); 2],
    slots_ready: bool,
    /// Tests only: load just the parts of this kind (see [`Circuit::census`]'s order).
    #[cfg(test)]
    only: Option<usize>,
}

/// A stamp's nodes: the one its current leaves, the one it enters, its slopes' nodes and
/// how many slopes it has.
type StampNodes = (Node, Node, [Node; 3], usize);

/// Where one stamp adds: the rows of its two nodes (`NONE` for a held node) and, for each
/// of up to three slopes, its entries in those rows.
#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    fa: u32,
    fb: u32,
    ja: [u32; 3],
    jb: [u32; 3],
}

const NONE: u32 = u32::MAX;

/// The most additions a flattened elimination holds (a circuit's needs a few hundred).
const FLAT_OPS: usize = 4096;

/// An elimination's structure, recorded for replaying ([`Circuit::linear_solve`]): each
/// column's pivot row, the rows its pivot search visits, the pivot row's columns from the
/// diagonal on and the rows it eliminates (flat, by column), and each column's pattern
/// before it (each column's rows and each row's columns, a bit each).
#[derive(Debug)]
struct LuProgram {
    ready: bool,
    pattern: usize,
    n: usize,
    piv: [u8; MAX_SOLVED],
    cand: Vec<u8>,
    cols: Vec<u8>,
    rows: Vec<u8>,
    cand_off: [u16; MAX_SOLVED + 1],
    cols_off: [u16; MAX_SOLVED + 1],
    rows_off: [u16; MAX_SOLVED + 1],
    snap_rows: Vec<[u64; MAX_SOLVED]>,
    snap_cols: Vec<[u64; MAX_SOLVED]>,
    /// The recorded elimination flattened for replaying (`flat_ok` while it fits its room):
    /// each column's rows below the pivot as (the entry its factor comes from, the row, its
    /// additions' range), each addition as (the entry added to, the pivot row's entry), and
    /// the back substitution's terms as (the entry, the unknown).
    flat_ok: bool,
    flat_tgt: Vec<[u16; 4]>,
    flat_tgt_off: [u16; MAX_SOLVED + 1],
    flat_ops: Vec<[u16; 2]>,
    flat_back: Vec<[u16; 2]>,
    flat_back_off: [u16; MAX_SOLVED + 1],
}

/// A copy keeps the lists' room, not only their contents: recording again on the audio
/// thread must not allocate (a voice is a copy of a prototype).
impl Clone for LuProgram {
    fn clone(&self) -> LuProgram {
        let with_room = |v: &Vec<u8>| {
            let mut c = Vec::with_capacity(self.n * self.n);
            c.extend_from_slice(v);
            c
        };
        LuProgram {
            ready: self.ready,
            pattern: self.pattern,
            n: self.n,
            piv: self.piv,
            cand: with_room(&self.cand),
            cols: with_room(&self.cols),
            rows: with_room(&self.rows),
            cand_off: self.cand_off,
            cols_off: self.cols_off,
            rows_off: self.rows_off,
            snap_rows: self.snap_rows.clone(),
            snap_cols: self.snap_cols.clone(),
            flat_ok: self.flat_ok,
            flat_tgt: {
                let mut c = Vec::with_capacity(self.flat_tgt.capacity());
                c.extend_from_slice(&self.flat_tgt);
                c
            },
            flat_tgt_off: self.flat_tgt_off,
            flat_ops: {
                let mut c = Vec::with_capacity(self.flat_ops.capacity());
                c.extend_from_slice(&self.flat_ops);
                c
            },
            flat_back: {
                let mut c = Vec::with_capacity(self.flat_back.capacity());
                c.extend_from_slice(&self.flat_back);
                c
            },
            flat_back_off: self.flat_back_off,
        }
    }
}

impl LuProgram {
    /// Room for a circuit of `n` solved nodes, none recorded.
    fn new(n: usize) -> LuProgram {
        LuProgram {
            ready: false,
            pattern: 0,
            n,
            piv: [0; MAX_SOLVED],
            cand: Vec::with_capacity(n * n),
            cols: Vec::with_capacity(n * n),
            rows: Vec::with_capacity(n * n),
            cand_off: [0; MAX_SOLVED + 1],
            cols_off: [0; MAX_SOLVED + 1],
            rows_off: [0; MAX_SOLVED + 1],
            snap_rows: vec![[0; MAX_SOLVED]; n],
            snap_cols: vec![[0; MAX_SOLVED]; n],
            flat_ok: false,
            flat_tgt: Vec::with_capacity(n * n),
            flat_tgt_off: [0; MAX_SOLVED + 1],
            flat_ops: Vec::with_capacity(FLAT_OPS),
            flat_back: Vec::with_capacity(n * n),
            flat_back_off: [0; MAX_SOLVED + 1],
        }
    }

    /// Flattens the recorded elimination of `n` columns ([`LuProgram`]'s `flat_*`); leaves
    /// `flat_ok` false if it does not fit the room made for it.
    fn flatten(&mut self, n: usize) {
        self.flat_ok = false;
        self.flat_tgt.clear();
        self.flat_ops.clear();
        self.flat_back.clear();
        for col in 0..n {
            let cols = &self.cols[self.cols_off[col] as usize..self.cols_off[col + 1] as usize];
            let rows = &self.rows[self.rows_off[col] as usize..self.rows_off[col + 1] as usize];
            for &r in rows {
                let r = r as usize;
                let start = self.flat_ops.len();
                for &c in cols.iter().filter(|&&c| c as usize > col) {
                    let c = c as usize;
                    if self.flat_ops.len() == self.flat_ops.capacity() {
                        return;
                    }
                    self.flat_ops
                        .push([(r * n + c) as u16, (col * n + c) as u16]);
                }
                self.flat_tgt.push([
                    (r * n + col) as u16,
                    r as u16,
                    start as u16,
                    self.flat_ops.len() as u16,
                ]);
            }
            self.flat_tgt_off[col + 1] = self.flat_tgt.len() as u16;
            for &c in cols.iter().filter(|&&c| c as usize > col) {
                self.flat_back
                    .push([(col * n + c as usize) as u16, c as u16]);
            }
            self.flat_back_off[col + 1] = self.flat_back.len() as u16;
        }
        self.flat_ok = true;
    }
}

/// SPICE's pn junction limiting (pnjlim): a junction's voltage change per Newton step
/// held to a logarithmic one above its critical voltage.
pub(crate) fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > 2.0 * vt {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 {
                vold + vt * crate::ulp::log(arg)
            } else {
                vcrit
            }
        } else {
            vt * crate::ulp::log(vnew / vt)
        }
    } else {
        vnew
    }
}

/// A diode's depletion and diffusion charge and their capacitance at a junction voltage.
fn diode_charge(vd: f64, is: f64, nvt: f64, cap: &Depletion, tt: f64) -> (f64, f64) {
    let e = crate::ulp::exp((vd / nvt).min(80.0));
    diode_charge_e(vd, is, nvt, (cap, &cap.consts()), tt, e, &mut [f64::NAN; 2])
}

/// [`diode_charge`] with `exp(vd / nvt)` (held at 80) given: the diode's current has it
/// already (performance), and the depletion charge's constants with it. Without a
/// transit time there is no diffusion charge.
fn diode_charge_e(
    vd: f64,
    is: f64,
    nvt: f64,
    (cap, k): (&Depletion, &DepletionConsts),
    tt: f64,
    e: f64,
    r: &mut [f64; 2],
) -> (f64, f64) {
    let (qj, cj) = cap.charge_near(vd, r, k);
    if tt == 0.0 {
        return (qj, cj);
    }
    (qj + tt * is * (e - 1.0), cj + tt * is * e / nvt)
}

/// A diffusion charge and its capacitance, the exponential held linear above 1 V (a
/// junction's voltage between Newton's iterates can be anything; no junction here works
/// above 1 V).
pub(crate) fn diffusion_charge(v: f64, is: f64, vt: f64, tt: f64) -> (f64, f64) {
    let v1 = v.min(1.0);
    let e = crate::ulp::exp(v1 / vt);
    let (q1, c1) = (tt * is * (e - 1.0), tt * is * e / vt);
    (q1 + c1 * (v - v1), c1)
}

pub(crate) fn vcrit(vt: f64, is: f64) -> f64 {
    vt * crate::ulp::log(vt / (core::f64::consts::SQRT_2 * is))
}

/// SPICE's FET limiting (fetlim): a gate voltage's change per Newton step held to steps
/// that do not jump across the threshold `vto` region at once.
fn fetlim(mut vnew: f64, vold: f64, vto: f64) -> f64 {
    let vtsthi = (2.0 * (vold - vto)).abs() + 2.0;
    let vtstlo = vtsthi / 2.0 + 2.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;
    if vold >= vto {
        if vold >= vtox {
            if delv <= 0.0 {
                if vnew >= vtox {
                    if -delv > vtstlo {
                        vnew = vold - vtstlo;
                    }
                } else {
                    vnew = vnew.max(vto + 2.0);
                }
            } else if delv >= vtsthi {
                vnew = vold + vtsthi;
            }
        } else if delv <= 0.0 {
            vnew = vnew.max(vto - 0.5);
        } else {
            vnew = vnew.min(vto + 4.0);
        }
    } else if delv <= 0.0 {
        if -delv > vtsthi {
            vnew = vold - vtsthi;
        }
    } else {
        let vtemp = vto + 0.5;
        if vnew <= vtemp {
            if delv > vtstlo {
                vnew = vold + vtstlo;
            }
        } else {
            vnew = vtemp;
        }
    }
    vnew
}

impl Circuit {
    /// A circuit of `nodes` nodes (ground included) whose first `held` (ground included)
    /// are held.
    pub fn new(nodes: usize, held: usize) -> Circuit {
        assert!(held >= 1 && held <= nodes);
        let n = nodes - held;
        Circuit {
            parts: Vec::new(),
            g: Vec::new(),
            near: Vec::new(),
            limiting: Vec::new(),
            charged: Vec::new(),
            consts: Vec::new(),
            nodes,
            held,
            v: vec![0.0; nodes],
            saved: vec![0.0; nodes],
            q: Vec::new(),
            i_prev: Vec::new(),
            theta: 1.0,
            lim: Vec::new(),
            vcrit: Vec::new(),
            jac: vec![0.0; n * n],
            lu: LuProgram::new(n),
            f: vec![0.0; n],
            dx: vec![0.0; n],
            tolerance: (1e-9, 1e-9),
            predict_limit: None,
            step_iterations: 100,
            iterations: 0,
            solves: 0,
            total_iterations: 0,
            lu_recorded: 0,
            lu_recorded_from: 0,
            iteration_counts: [0; 8],
            nanos: 0,
            nanos_load: 0,
            nanos_lu: 0,
            prev: vec![0.0; nodes],
            prev2: vec![0.0; nodes],
            prev_h: 0.0,
            prev2_h: 0.0,
            qc: Vec::new(),
            qc_ok: false,
            slots: Vec::new(),
            pattern: [([0; MAX_SOLVED], [0; MAX_SOLVED]); 2],
            slots_ready: false,
            #[cfg(test)]
            only: None,
        }
    }

    /// Adds a solved node (a device's internal node); returns it.
    pub fn node(&mut self) -> Node {
        self.nodes += 1;
        self.v.push(0.0);
        self.saved.push(0.0);
        self.prev.push(0.0);
        self.prev2.push(0.0);
        self.slots_ready = false;
        let n = self.nodes - self.held;
        assert!(
            n <= MAX_SOLVED,
            "a circuit of more than {MAX_SOLVED} solved nodes"
        );
        self.jac = vec![0.0; n * n];
        self.lu = LuProgram::new(n);
        self.f.push(0.0);
        self.dx.push(0.0);
        self.nodes - 1
    }

    /// Adds a bipolar transistor with its base, emitter and collector resistances (as
    /// ngspice's model has them) on internal nodes, at a temperature in C.
    pub fn add_bjt(
        &mut self,
        c: Node,
        b: Node,
        e: Node,
        q: &crate::devices::Bjt,
        celsius: f64,
        pnp: bool,
    ) {
        let inner = |outer: Node, r: f64, circuit: &mut Circuit| -> Node {
            if r > 0.0 {
                let n = circuit.node();
                circuit.v[n] = circuit.v[outer];
                circuit.add(Part::Resistor { a: outer, b: n, r });
                n
            } else {
                outer
            }
        };
        let ci = inner(c, q.rc, self);
        let bi = inner(b, q.rb, self);
        let ei = inner(e, q.re, self);
        let m = q.at(celsius);
        self.add(Part::Bjt {
            c: ci,
            b: bi,
            e: ei,
            m,
            vaf: q.vaf,
            pnp,
        });
        // Its charges (ngspice's, at the internal nodes): the junctions' depletion
        // capacitance and the forward transit time's diffusion charge (on the forward
        // current alone: the base's modulation of it is left out).
        let (anode_e, cathode_e) = if pnp { (ei, bi) } else { (bi, ei) };
        let (anode_c, cathode_c) = if pnp { (ci, bi) } else { (bi, ci) };
        if q.cje > 0.0 {
            let cap = Depletion {
                cj0: q.cje,
                vj: q.vje,
                m: q.mje,
                fc: 0.5,
            };
            self.add(Part::Junction {
                a: anode_e,
                k: cathode_e,
                cap,
            });
        }
        if q.cjc > 0.0 {
            let cap = Depletion {
                cj0: q.cjc,
                vj: q.vjc,
                m: q.mjc,
                fc: 0.5,
            };
            self.add(Part::Junction {
                a: anode_c,
                k: cathode_c,
                cap,
            });
        }
        if q.tf > 0.0 {
            self.add(Part::Diffusion {
                a: anode_e,
                k: cathode_e,
                is: m.is,
                vt: m.vt,
                tt: q.tf,
            });
        }
    }

    /// Adds an N-channel JFET as ngspice's model has it: the drain and source resistances
    /// on internal nodes, the channel between them, and the gate's junctions (current and
    /// depletion charge, M = 0.5) from the gate to the internal drain and source, at 25 C.
    pub fn add_jfet(&mut self, d: Node, g: Node, s: Node, j: &Jfet) {
        let mut inner = |outer: Node, r: f64| -> Node {
            if r > 0.0 {
                let n = self.node();
                self.v[n] = self.v[outer];
                self.add(Part::Resistor { a: outer, b: n, r });
                n
            } else {
                outer
            }
        };
        let di = inner(d, j.rd);
        let si = inner(s, j.rs);
        self.add(Part::Jfet {
            d: di,
            g,
            s: si,
            m: Jfet {
                rd: 0.0,
                rs: 0.0,
                ..*j
            },
        });
        let nvt = j.n * crate::devices::vt(25.0);
        let cap = |cj0| Depletion {
            cj0,
            vj: j.pb,
            m: 0.5,
            fc: j.fc,
        };
        self.add(Part::Diode {
            a: g,
            k: si,
            is: j.is,
            nvt,
            cap: cap(j.cgs),
            tt: 0.0,
        });
        self.add(Part::Diode {
            a: g,
            k: di,
            is: j.is,
            nvt,
            cap: cap(j.cgd),
            tt: 0.0,
        });
    }

    /// Adds a part; returns its index (for changing a resistor later).
    pub fn add(&mut self, p: Part) -> usize {
        let idx = self.parts.len() as u32;
        if matches!(p, Part::Diode { .. } | Part::Bjt { .. } | Part::Jfet { .. }) {
            self.limiting.push(idx);
        }
        if matches!(
            p,
            Part::Capacitor { .. }
                | Part::Junction { .. }
                | Part::Diffusion { .. }
                | Part::Diode { .. }
        ) {
            self.charged.push(idx);
        }
        self.parts.push(p);
        self.g.push(match p {
            Part::Resistor { r, .. } => 1.0 / r,
            _ => 0.0,
        });
        self.qc.push([0.0; 3]);
        self.near.push([[f64::NAN; 2]; 3]);
        self.consts.push(match p {
            Part::Junction { cap, .. } | Part::Diode { cap, .. } => cap.consts(),
            _ => DepletionConsts::default(),
        });
        self.slots.push([Slot::default(); 3]);
        self.slots_ready = false;
        self.q.push(0.0);
        self.i_prev.push(0.0);
        self.lim.push([0.0; 2]);
        self.vcrit.push(match p {
            Part::Diode { is, nvt, .. } => vcrit(nvt, is),
            Part::Bjt { m, .. } => vcrit(m.vt, m.is),
            _ => 0.0,
        });
        self.parts.len() - 1
    }

    /// A node's voltage.
    pub fn v(&self, n: Node) -> f64 {
        self.v[n]
    }

    /// Sets a node's voltage: a held node's value, or a solved node's starting guess (the
    /// predictor then starts again).
    pub fn set(&mut self, n: Node, v: f64) {
        self.v[n] = v;
        if n >= self.held {
            self.prev_h = 0.0;
        }
    }

    /// The integration method for the steps that follow (see [`Circuit`]'s `theta`): 1
    /// backward Euler, 0.5 trapezoidal.
    pub fn set_theta(&mut self, theta: f64) {
        assert!((0.5..=1.0).contains(&theta));
        self.theta = theta;
    }

    /// A charge's current over a step of `h` from its last accepted point to `q`, and the
    /// slope's divisor (theta h).
    fn charge_current(&self, idx: usize, q: f64, h: f64) -> (f64, f64) {
        if self.theta == 1.0 {
            return ((q - self.q[idx]) / h, h);
        }
        let th = self.theta * h;
        (
            (q - self.q[idx]) / th - (1.0 - self.theta) / self.theta * self.i_prev[idx],
            th,
        )
    }

    /// Changes a resistor's value.
    pub fn set_resistance(&mut self, part: usize, r: f64) {
        if let Part::Resistor { r: x, .. } = &mut self.parts[part] {
            *x = r;
            self.g[part] = 1.0 / r;
        }
    }

    fn free(&self, n: Node) -> Option<usize> {
        (n >= self.held).then(|| n - self.held)
    }

    /// Each part's stamps: the nodes a stamp's current leaves and enters and the nodes of its
    /// slopes, in the order its load gives them (a slope against a held node adds nothing).
    fn stamp_nodes(p: &Part) -> [Option<StampNodes>; 3] {
        match *p {
            Part::Resistor { a, b, .. }
            | Part::Capacitor { a, b, .. }
            | Part::Junction { a, k: b, .. }
            | Part::Diffusion { a, k: b, .. }
            | Part::Diode { a, k: b, .. } => [Some((a, b, [a, b, GND], 2)), None, None],
            Part::Bjt { c, b, e, .. } => [
                Some((c, GND, [b, e, c], 3)),
                Some((b, GND, [b, e, c], 3)),
                Some((e, GND, [b, e, c], 3)),
            ],
            Part::Jfet { d, g, s, .. } => [Some((d, s, [g, d, s], 3)), None, None],
        }
    }

    /// Works out each stamp's positions and the Jacobian's patterns (see `slots`).
    fn build_slots(&mut self) {
        let n = self.nodes - self.held;
        let row = |x: Node| -> u32 {
            if x >= self.held {
                (x - self.held) as u32
            } else {
                NONE
            }
        };
        let mut pattern = [([0u64; MAX_SOLVED], [0u64; MAX_SOLVED]); 2];
        for (rows_in, cols_in) in &mut pattern {
            for k in 0..n {
                rows_in[k] = 1 << k;
                cols_in[k] = 1 << k;
            }
        }
        for idx in 0..self.parts.len() {
            let p = self.parts[idx];
            // Charges stamp only in a step, not at DC.
            let in_dc = !matches!(
                p,
                Part::Capacitor { .. } | Part::Junction { .. } | Part::Diffusion { .. }
            );
            for (s, stamp) in Self::stamp_nodes(&p).iter().enumerate() {
                let Some((a, b, on, count)) = *stamp else {
                    continue;
                };
                let mut slot = Slot {
                    fa: row(a),
                    fb: row(b),
                    ja: [NONE; 3],
                    jb: [NONE; 3],
                };
                for (j, &m) in on.iter().take(count).enumerate() {
                    let cm = row(m);
                    if cm == NONE {
                        continue;
                    }
                    for (r, entry) in [(slot.fa, &mut slot.ja[j]), (slot.fb, &mut slot.jb[j])] {
                        if r == NONE {
                            continue;
                        }
                        *entry = r * n as u32 + cm;
                        for (dc, (rows_in, cols_in)) in pattern.iter_mut().enumerate() {
                            if dc == 1 || in_dc {
                                rows_in[cm as usize] |= 1 << r;
                                cols_in[r as usize] |= 1 << cm;
                            }
                        }
                    }
                }
                self.slots[idx][s] = slot;
            }
        }
        // (Index 0 is the DC pattern, 1 a step's.)
        self.pattern = pattern;
        self.slots_ready = true;
    }

    /// Adds a current `i` leaving part `idx`'s stamp `s`'s first node (entering its second)
    /// with its slopes `g` against its slope nodes, at positions worked out beforehand.
    #[inline]
    fn stamp_at(&mut self, idx: usize, s: usize, i: f64, g: &[f64]) {
        let slot = self.slots[idx][s];
        if slot.fa != NONE {
            self.f[slot.fa as usize] += i;
            for (&at, &x) in slot.ja.iter().zip(g) {
                if at != NONE {
                    self.jac[at as usize] += x;
                }
            }
        }
        if slot.fb != NONE {
            self.f[slot.fb as usize] -= i;
            for (&at, &x) in slot.jb.iter().zip(g) {
                if at != NONE {
                    self.jac[at as usize] -= x;
                }
            }
        }
    }

    /// A part's charge at the present voltages (0 for parts that store none).
    fn charge(&self, p: &Part) -> (f64, f64) {
        match *p {
            Part::Capacitor { a, b, c } => (c * (self.v[a] - self.v[b]), c),
            Part::Junction { a, k, cap } => cap.charge(self.v[a] - self.v[k]),
            Part::Diode {
                a,
                k,
                is,
                nvt,
                cap,
                tt,
            } => diode_charge(self.v[a] - self.v[k], is, nvt, &cap, tt),
            Part::Diffusion { a, k, is, vt, tt } => {
                diffusion_charge(self.v[a] - self.v[k], is, vt, tt)
            }
            _ => (0.0, 0.0),
        }
    }

    /// Loads the residual and Jacobian at the present voltages; `h` is the step (None for
    /// DC, where charges carry no current). Returns whether any junction was limited.
    fn load(&mut self, h: Option<f64>) -> bool {
        if !self.slots_ready {
            self.build_slots();
        }
        self.qc_ok = h.is_some();
        self.jac.iter_mut().for_each(|x| *x = 0.0);
        self.f.iter_mut().for_each(|x| *x = 0.0);
        let n = self.nodes - self.held;
        for k in 0..n {
            let node = k + self.held;
            self.f[k] += GMIN * self.v[node];
            self.jac[k * n + k] += GMIN;
        }
        let mut limited = false;
        // The parts taken out for the loop, not copied part by part (a part is as large as
        // its largest kind, a transistor's model; performance).
        let parts = std::mem::take(&mut self.parts);
        for (idx, p) in parts.iter().enumerate() {
            #[cfg(test)]
            if let Some(k) = self.only
                && k != Self::kind(p)
            {
                continue;
            }
            match *p {
                Part::Resistor { a, b, .. } => {
                    let g = self.g[idx];
                    let i = g * (self.v[a] - self.v[b]);
                    self.stamp_at(idx, 0, i, &[g, -g]);
                }
                Part::Capacitor { a, b, .. }
                | Part::Junction { a, k: b, .. }
                | Part::Diffusion { a, k: b, .. } => {
                    if let Some(h) = h {
                        let v = self.v[a] - self.v[b];
                        let (q, c) = match *p {
                            Part::Junction { ref cap, .. } => {
                                cap.charge_near(v, &mut self.near[idx][0], &self.consts[idx])
                            }
                            _ => self.charge(p),
                        };
                        self.qc[idx] = [q, c, v];
                        let (i, th) = self.charge_current(idx, q, h);
                        self.stamp_at(idx, 0, i, &[c / th, -c / th]);
                    }
                }
                Part::Diode {
                    a,
                    k,
                    is,
                    nvt,
                    ref cap,
                    tt,
                } => {
                    let vd0 = self.v[a] - self.v[k];
                    let vd = pnjlim(vd0, self.lim[idx][0], nvt, self.vcrit[idx]);
                    limited |= vd != vd0;
                    self.lim[idx][0] = vd;
                    let e = crate::ulp::exp((vd / nvt).min(80.0));
                    let g = is * e / nvt + GMIN;
                    // Linearised about the limited voltage.
                    let i = is * (e - 1.0) + GMIN * vd + g * (vd0 - vd);
                    let mut gs = g;
                    let mut ii = i;
                    if let Some(h) = h {
                        // The charge at the limited voltage too (at the unlimited one the
                        // diffusion charge can be astronomical), linearised likewise.
                        let (q, c) = diode_charge_e(
                            vd,
                            is,
                            nvt,
                            (cap, &self.consts[idx]),
                            tt,
                            e,
                            &mut self.near[idx][1],
                        );
                        self.qc[idx] = [q, c, vd];
                        let (iq, th) = self.charge_current(idx, q, h);
                        ii += iq + c / th * (vd0 - vd);
                        gs += c / th;
                    }
                    self.stamp_at(idx, 0, ii, &[gs, -gs]);
                }
                Part::Bjt {
                    c,
                    b,
                    e,
                    ref m,
                    vaf,
                    pnp,
                } => {
                    let pol = if pnp { -1.0 } else { 1.0 };
                    let x1_0 = pol * (self.v[b] - self.v[e]);
                    let x2_0 = pol * (self.v[b] - self.v[c]);
                    let vc = self.vcrit[idx];
                    let x1 = pnjlim(x1_0, self.lim[idx][0], m.vt, vc);
                    let x2 = pnjlim(x2_0, self.lim[idx][1], m.vt, vc);
                    limited |= x1 != x1_0 || x2 != x2_0;
                    self.lim[idx] = [x1, x2];
                    let d = m.currents_d(x1, x2, vaf);
                    // Linearised about the limited junction voltages, plus GMIN across
                    // each junction.
                    let (dx1, dx2) = (x1_0 - x1, x2_0 - x2);
                    let ic = d.ic + d.dic[0] * dx1 + d.dic[1] * dx2 - GMIN * x2_0;
                    let ib = d.ib + d.dib[0] * dx1 + d.dib[1] * dx2 + GMIN * (x1_0 + x2_0);
                    let (c1, c2) = (d.dic[0], d.dic[1] - GMIN);
                    let (b1, b2) = (d.dib[0] + GMIN, d.dib[1] + GMIN);
                    // Currents leaving the nodes into the device: pol * ic at the
                    // collector, pol * ib at the base; slopes (polarity cancels).
                    self.stamp_at(idx, 0, pol * ic, &[c1 + c2, -c1, -c2]);
                    self.stamp_at(idx, 1, pol * ib, &[b1 + b2, -b1, -b2]);
                    self.stamp_at(
                        idx,
                        2,
                        -pol * (ic + ib),
                        &[-(c1 + c2 + b1 + b2), c1 + b1, c2 + b2],
                    );
                }
                Part::Jfet { d, g, s, ref m } => {
                    // Limit the gate's voltages to source and drain (fetlim), evaluate
                    // there, and linearise to the present voltages.
                    let vg = self.v[g];
                    let (vgs0, vgd0) = (vg - self.v[s], vg - self.v[d]);
                    let vgs = fetlim(vgs0, self.lim[idx][0], m.vto);
                    let vgd = fetlim(vgd0, self.lim[idx][1], m.vto);
                    limited |= vgs != vgs0 || vgd != vgd0;
                    self.lim[idx] = [vgs, vgd];
                    let (vd_l, vs_l) = (vg - vgd, vg - vgs);
                    let (i, sl) = m.channel(vg, vd_l, vs_l);
                    let i = i + sl[1] * (self.v[d] - vd_l) + sl[2] * (self.v[s] - vs_l);
                    self.stamp_at(idx, 0, i, &[sl[0], sl[1], sl[2]]);
                }
            }
        }
        self.parts = parts;
        limited
    }

    /// Solves `jac dx = -f` in place: Gaussian elimination with partial pivoting over the
    /// load's `pattern` (0 DC, 1 a step).
    ///
    /// The elimination's structure (which rows each pivot row reaches and which of its
    /// columns) follows from the pattern and the pivots alone, so it is recorded and
    /// replayed while each column's pivot, searched for as always, is the one recorded; from
    /// the first that is not, it is worked out again from that column's recorded pattern
    /// (performance). The additions are the same as an elimination that tracks
    /// nonzeros by value would make, in the same order; the extra ones the structure admits
    /// add exact zeros.
    fn linear_solve(&mut self, pattern: usize) {
        let n = self.nodes - self.held;
        for k in 0..n {
            self.dx[k] = -self.f[k];
        }
        let below = |mask: u64, col: usize| mask & (u64::MAX << col) & !(1u64 << col);
        // While the recording holds, its flattened form, up to the first column whose pivot
        // moves (performance: the same additions less the bookkeeping, and less those
        // that would only zero the entries below a pivot, which are never read again).
        let from =
            if self.lu.ready && self.lu.pattern == pattern && self.lu.n == n && self.lu.flat_ok {
                self.replay_flat(n)
            } else {
                0
            };
        if from == n {
            self.back_flat(n);
            return;
        }
        self.lu_recorded += 1;
        self.lu_recorded_from += from as u64;
        let lu = &mut self.lu;
        let mut valid = lu.ready && lu.pattern == pattern && lu.n == n;
        if !valid {
            // Recorded afresh from the first column.
            lu.cand.clear();
            lu.cols.clear();
            lu.rows.clear();
        }
        let (mut rows_in, mut cols_in) = self.pattern[pattern];
        for col in from..n {
            let mut piv = col;
            if valid {
                for &r in &lu.cand[lu.cand_off[col] as usize..lu.cand_off[col + 1] as usize] {
                    let r = r as usize;
                    if self.jac[r * n + col].abs() > self.jac[piv * n + col].abs() {
                        piv = r;
                    }
                }
                if piv != lu.piv[col] as usize {
                    // Worked out again from here, from this column's pattern.
                    valid = false;
                    rows_in = lu.snap_rows[col];
                    cols_in = lu.snap_cols[col];
                    lu.cand.truncate(lu.cand_off[col] as usize);
                    lu.cols.truncate(lu.cols_off[col] as usize);
                    lu.rows.truncate(lu.rows_off[col] as usize);
                }
            }
            if !valid {
                lu.snap_rows[col] = rows_in;
                lu.snap_cols[col] = cols_in;
                piv = col;
                let mut cand = below(rows_in[col], col);
                while cand != 0 {
                    let r = cand.trailing_zeros() as usize;
                    cand &= cand - 1;
                    lu.cand.push(r as u8);
                    if self.jac[r * n + col].abs() > self.jac[piv * n + col].abs() {
                        piv = r;
                    }
                }
                lu.piv[col] = piv as u8;
                if piv != col {
                    for m in rows_in.iter_mut().take(n).skip(col) {
                        if (*m >> col) & 1 != (*m >> piv) & 1 {
                            *m ^= (1 << col) | (1 << piv);
                        }
                    }
                    cols_in.swap(col, piv);
                }
                let fill = cols_in[col] & (u64::MAX << col);
                let mut cols = fill;
                while cols != 0 {
                    lu.cols.push(cols.trailing_zeros() as u8);
                    cols &= cols - 1;
                }
                let mut rows = below(rows_in[col], col);
                while rows != 0 {
                    let r = rows.trailing_zeros() as usize;
                    rows &= rows - 1;
                    lu.rows.push(r as u8);
                    cols_in[r] |= fill;
                    let mut c = fill;
                    while c != 0 {
                        rows_in[c.trailing_zeros() as usize] |= 1 << r;
                        c &= c - 1;
                    }
                }
                lu.cand_off[col + 1] = lu.cand.len() as u16;
                lu.cols_off[col + 1] = lu.cols.len() as u16;
                lu.rows_off[col + 1] = lu.rows.len() as u16;
            }
            if piv != col {
                for c in col..n {
                    self.jac.swap(col * n + c, piv * n + c);
                }
                self.dx.swap(col, piv);
            }
            let d = self.jac[col * n + col];
            let cols = &lu.cols[lu.cols_off[col] as usize..lu.cols_off[col + 1] as usize];
            let (top, rest) = self.jac.split_at_mut((col + 1) * n);
            let pivot = &top[col * n..];
            for &r in &lu.rows[lu.rows_off[col] as usize..lu.rows_off[col + 1] as usize] {
                let r = r as usize;
                let row = &mut rest[(r - col - 1) * n..(r - col) * n];
                let factor = row[col] / d;
                for &c in cols {
                    let c = c as usize;
                    row[c] -= factor * pivot[c];
                }
                self.dx[r] -= factor * self.dx[col];
            }
        }
        if !valid {
            lu.ready = true;
            lu.pattern = pattern;
            lu.n = n;
            lu.flatten(n);
        }
        for col in (0..n).rev() {
            let mut s = self.dx[col];
            for &c in &lu.cols[lu.cols_off[col] as usize..lu.cols_off[col + 1] as usize] {
                let c = c as usize;
                if c > col {
                    s -= self.jac[col * n + c] * self.dx[c];
                }
            }
            self.dx[col] = s / self.jac[col * n + col];
        }
    }

    /// The flattened elimination's columns while each one's pivot, searched for as always,
    /// is the one recorded; returns the first whose is not (`n` when none), untouched.
    fn replay_flat(&mut self, n: usize) -> usize {
        let lu = &self.lu;
        for col in 0..n {
            let mut piv = col;
            for &r in &lu.cand[lu.cand_off[col] as usize..lu.cand_off[col + 1] as usize] {
                let r = r as usize;
                if self.jac[r * n + col].abs() > self.jac[piv * n + col].abs() {
                    piv = r;
                }
            }
            if piv != lu.piv[col] as usize {
                return col;
            }
            if piv != col {
                for c in col..n {
                    self.jac.swap(col * n + c, piv * n + c);
                }
                self.dx.swap(col, piv);
            }
            let d = self.jac[col * n + col];
            let dx_col = self.dx[col];
            for t in &lu.flat_tgt[lu.flat_tgt_off[col] as usize..lu.flat_tgt_off[col + 1] as usize]
            {
                let factor = self.jac[t[0] as usize] / d;
                for op in &lu.flat_ops[t[2] as usize..t[3] as usize] {
                    let pivot = self.jac[op[1] as usize];
                    self.jac[op[0] as usize] -= factor * pivot;
                }
                self.dx[t[1] as usize] -= factor * dx_col;
            }
        }
        n
    }

    /// The flattened back substitution.
    fn back_flat(&mut self, n: usize) {
        let lu = &self.lu;
        for col in (0..n).rev() {
            let mut s = self.dx[col];
            for b in
                &lu.flat_back[lu.flat_back_off[col] as usize..lu.flat_back_off[col + 1] as usize]
            {
                s -= self.jac[b[0] as usize] * self.dx[b[1] as usize];
            }
            self.dx[col] = s / self.jac[col * n + col];
        }
    }

    /// Newton's method at a step `h` (None: DC) from the present voltages.
    fn newton(&mut self, h: Option<f64>, max_iter: usize) -> Result<usize, NoConvergence> {
        let n = self.nodes - self.held;
        // Start the limiting from the present junction voltages, no further forward than
        // their critical voltages (so a wild guess is limited from the first step).
        for &idx in &self.limiting {
            let idx = idx as usize;
            self.lim[idx] = match self.parts[idx] {
                Part::Diode { a, k, .. } => [(self.v[a] - self.v[k]).min(self.vcrit[idx]), 0.0],
                Part::Bjt { c, b, e, pnp, .. } => {
                    let pol = if pnp { -1.0 } else { 1.0 };
                    let vc = self.vcrit[idx];
                    [
                        (pol * (self.v[b] - self.v[e])).min(vc),
                        (pol * (self.v[b] - self.v[c])).min(vc),
                    ]
                }
                Part::Jfet { d, g, s, .. } => [self.v[g] - self.v[s], self.v[g] - self.v[d]],
                _ => [0.0; 2],
            };
        }
        self.solves += 1;
        let tol = self.tolerance;
        let mut loaded = false;
        for it in 1..=max_iter {
            loaded = true;
            self.total_iterations += 1;
            let mut laps = crate::prof::Laps::start();
            #[cfg(feature = "profile")]
            let t0 = std::time::Instant::now();
            let limited = self.load(h);
            laps.lap(crate::prof::Part::SolverLoad);
            #[cfg(feature = "profile")]
            let t1 = std::time::Instant::now();
            self.linear_solve(usize::from(h.is_some()));
            laps.lap(crate::prof::Part::SolverSolve);
            #[cfg(feature = "profile")]
            {
                self.nanos_load += (t1 - t0).as_nanos() as u64;
                self.nanos_lu += t1.elapsed().as_nanos() as u64;
            }
            // Full Newton steps: the junctions' limiting keeps them in range, as in SPICE
            // (damping every node instead held linear nodes back and let junctions creep
            // far forward in small, unlimited steps).
            if self.dx.iter().any(|x| !x.is_finite()) {
                break;
            }
            for k in 0..n {
                self.v[k + self.held] += self.dx[k];
            }
            let small =
                (0..n).all(|k| self.dx[k].abs() <= tol.0 + tol.1 * self.v[k + self.held].abs());
            if small && !limited {
                self.iterations = it;
                self.iteration_counts[match it {
                    0..=4 => it.max(1) - 1,
                    5..=8 => 4,
                    9..=16 => 5,
                    17..=32 => 6,
                    _ => 7,
                }] += 1;
                return Ok(it);
            }
        }
        self.iteration_counts[7] += 1;
        self.iterations = max_iter;
        // The last load's residual (worked out only on failure: the elimination leaves the
        // residual as it was).
        let residual = if loaded {
            self.f.iter().fold(0.0f64, |a, x| a.max(x.abs()))
        } else {
            f64::INFINITY
        };
        Err(NoConvergence {
            iterations: max_iter,
            residual,
        })
    }

    /// The DC operating point from the present voltages (charges carry no current), and
    /// the charges there taken as the state. On failure the voltages are restored.
    pub fn dc(&mut self) -> Result<usize, NoConvergence> {
        self.prev_h = 0.0;
        self.saved.copy_from_slice(&self.v);
        match self.newton(None, 200) {
            Ok(it) => {
                self.accept(None);
                Ok(it)
            }
            Err(e) => {
                self.v.copy_from_slice(&self.saved);
                Err(e)
            }
        }
    }

    /// One backward Euler step of `h` s from the last accepted point. On failure the
    /// voltages are restored, so the step can be retried shorter.
    pub fn step(&mut self, h: f64) -> Result<usize, NoConvergence> {
        #[cfg(feature = "profile")]
        let t0 = std::time::Instant::now();
        self.saved.copy_from_slice(&self.v);
        // Newton starts from the solved nodes extrapolated from the last three accepted
        // points (quadratically) when the last two steps were as long as this one
        // (performance): the keyboard's solves take 1.1 iterations instead of 2, the
        // preamplifier's 2.4 instead of 2.9, to the same tolerance. (A failed step is
        // retried from the unextrapolated voltages.)
        if self.prev_h == h && self.prev2_h == h {
            let smooth = self.predict_limit.is_none_or(|lim| {
                (self.held..self.nodes).all(|k| {
                    let d = 2.0 * self.v[k] - 3.0 * self.prev[k] + self.prev2[k];
                    d.abs() <= lim
                })
            });
            if smooth {
                for k in self.held..self.nodes {
                    self.v[k] = 3.0 * self.v[k] - 3.0 * self.prev[k] + self.prev2[k];
                }
            }
        }
        let r = match self.newton(Some(h), self.step_iterations) {
            Ok(it) => {
                // The last accepted point becomes the one before (swapped, not copied:
                // `saved` is set again before it is next read).
                std::mem::swap(&mut self.prev, &mut self.prev2);
                std::mem::swap(&mut self.prev, &mut self.saved);
                self.prev2_h = self.prev_h;
                self.prev_h = h;
                self.accept(Some(h));
                Ok(it)
            }
            Err(e) => {
                self.v.copy_from_slice(&self.saved);
                Err(e)
            }
        };
        #[cfg(feature = "profile")]
        {
            self.nanos += t0.elapsed().as_nanos() as u64;
        }
        r
    }

    fn accept(&mut self, h: Option<f64>) {
        // After a step the charges are the last load's, moved by their capacitance over the
        // last Newton update (under its 1e-9 V tolerance: the error is of the order of C
        // times its square), not evaluated again (performance).
        let cached = h.is_some() && self.qc_ok;
        for &idx in &self.charged {
            let idx = idx as usize;
            // Borrowed, not copied (a part is as large as a transistor's model).
            let p = &self.parts[idx];
            let q = match *p {
                Part::Capacitor { a, b, c } => c * (self.v[a] - self.v[b]),
                Part::Junction { a, k, .. }
                | Part::Diffusion { a, k, .. }
                | Part::Diode { a, k, .. }
                    if cached =>
                {
                    let [q, c, at] = self.qc[idx];
                    q + c * (self.v[a] - self.v[k] - at)
                }
                Part::Junction { .. } | Part::Diffusion { .. } | Part::Diode { .. } => {
                    self.charge(p).0
                }
                _ => 0.0,
            };
            if self.theta != 1.0 {
                self.i_prev[idx] = match h {
                    Some(h) => self.charge_current(idx, q, h).0,
                    None => 0.0,
                };
            }
            self.q[idx] = q;
        }
    }

    /// The residual and Jacobian at the present voltages, for checking the Jacobian by
    /// finite differences (tests): (f, jac) over the solved nodes.
    pub fn residual_and_jacobian(&mut self, h: Option<f64>) -> (Vec<f64>, Vec<f64>) {
        for idx in 0..self.parts.len() {
            self.lim[idx] = match self.parts[idx] {
                Part::Diode { a, k, .. } => [self.v[a] - self.v[k], 0.0],
                Part::Bjt { c, b, e, pnp, .. } => {
                    let pol = if pnp { -1.0 } else { 1.0 };
                    [pol * (self.v[b] - self.v[e]), pol * (self.v[b] - self.v[c])]
                }
                Part::Jfet { d, g, s, .. } => [self.v[g] - self.v[s], self.v[g] - self.v[d]],
                _ => [0.0; 2],
            };
        }
        self.load(h);
        (self.f.clone(), self.jac.clone())
    }

    /// The small-signal model at the present operating point (after [`Circuit::dc`]):
    /// conductance and capacitance matrices over the solved nodes (row-major, n x n) and
    /// the conductance column of a held node `input`, such that G x + C dx/dt = -g_in u
    /// for the solved nodes' and the input's deviations.
    pub fn small_signal(&mut self, input: Node) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = self.nodes - self.held;
        let (f0, g) = self.residual_and_jacobian(None);
        let saved = self.v[input];
        let d = 1e-6;
        self.v[input] = saved + d;
        let (f1, _) = self.residual_and_jacobian(None);
        self.v[input] = saved;
        let g_in: Vec<f64> = f1.iter().zip(&f0).map(|(a, b)| (a - b) / d).collect();
        let mut c = vec![0.0; n * n];
        for idx in 0..self.parts.len() {
            let p = self.parts[idx];
            let (a, b) = match p {
                Part::Capacitor { a, b, .. } => (a, b),
                Part::Junction { a, k, .. }
                | Part::Diode { a, k, .. }
                | Part::Diffusion { a, k, .. } => (a, k),
                _ => continue,
            };
            let (_, cap) = self.charge(&p);
            let fa = self.free(a);
            let fb = self.free(b);
            if let Some(i) = fa {
                c[i * n + i] += cap;
            }
            if let Some(j) = fb {
                c[j * n + j] += cap;
            }
            if let (Some(i), Some(j)) = (fa, fb) {
                c[i * n + j] -= cap;
                c[j * n + i] -= cap;
            }
        }
        (g, c, g_in)
    }

    /// A solved node's index among the solved nodes (for [`Circuit::small_signal`]'s rows).
    pub fn solved_index(&self, n: Node) -> Option<usize> {
        self.free(n)
    }

    /// The number of nodes and of held nodes.
    pub fn size(&self) -> (usize, usize) {
        (self.nodes, self.held)
    }

    /// The solved nodes and the parts (for profiles).
    pub fn extent(&self) -> (usize, usize) {
        (self.nodes - self.held, self.parts.len())
    }

    /// The solved nodes only resistors and linear capacitors touch (for profiles).
    pub fn linear_nodes(&self) -> usize {
        let mut nonlinear = vec![false; self.nodes];
        for p in &self.parts {
            if !matches!(p, Part::Resistor { .. } | Part::Capacitor { .. })
                && let Some((a, b, on, n)) = Self::stamp_nodes(p)[0]
            {
                for &x in [a, b].iter().chain(&on[..n]) {
                    nonlinear[x] = true;
                }
            }
            if let Part::Bjt { c, b, e, .. } = *p {
                for x in [c, b, e] {
                    nonlinear[x] = true;
                }
            }
        }
        (self.held..self.nodes).filter(|&x| !nonlinear[x]).count()
    }

    /// The solved nodes' connected components (parts joining solved nodes; held nodes join
    /// nothing): their sizes, largest first (for profiles).
    pub fn components(&self) -> Vec<usize> {
        let n = self.nodes;
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            p[x] = r;
            r
        }
        for p in &self.parts {
            for stamp in Self::stamp_nodes(p).iter().flatten() {
                let (a, b, on, k) = *stamp;
                let nodes: Vec<usize> = [a, b]
                    .iter()
                    .chain(&on[..k])
                    .copied()
                    .filter(|&x| x >= self.held)
                    .collect();
                for w in nodes.windows(2) {
                    let (ra, rb) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
                    parent[ra] = rb;
                }
            }
        }
        let mut sizes = std::collections::BTreeMap::new();
        for x in self.held..n {
            *sizes.entry(find(&mut parent, x)).or_insert(0usize) += 1;
        }
        let mut v: Vec<usize> = sizes.into_values().collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v
    }

    /// A part's kind's place in [`Circuit::census`]'s order.
    #[cfg(test)]
    fn kind(p: &Part) -> usize {
        match p {
            Part::Resistor { .. } => 0,
            Part::Capacitor { .. } => 1,
            Part::Diode { .. } => 2,
            Part::Junction { .. } => 3,
            Part::Diffusion { .. } => 4,
            Part::Bjt { .. } => 5,
            Part::Jfet { .. } => 6,
        }
    }

    /// How many parts of each kind, by name (for profiles).
    pub fn census(&self) -> Vec<(&'static str, usize)> {
        let names = [
            "resistor",
            "capacitor",
            "diode",
            "junction",
            "diffusion",
            "bjt",
            "jfet",
        ];
        let mut counts = [0usize; 7];
        for p in &self.parts {
            counts[match p {
                Part::Resistor { .. } => 0,
                Part::Capacitor { .. } => 1,
                Part::Diode { .. } => 2,
                Part::Junction { .. } => 3,
                Part::Diffusion { .. } => 4,
                Part::Bjt { .. } => 5,
                Part::Jfet { .. } => 6,
            }] += 1;
        }
        names
            .into_iter()
            .zip(counts)
            .filter(|(_, c)| *c > 0)
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Times a circuit's load and linear solve apart, at its present point (a diagnostic for
    /// the solver's performance; `cargo test --release -p ca72 -- --ignored
    /// solver_parts --nocapture`).
    pub(crate) fn time_parts(c: &mut Circuit, h: f64, name: &str) {
        let n = c.nodes - c.held;
        let reps = 100_000;
        c.load(Some(h));
        c.linear_solve(1);
        let t0 = std::time::Instant::now();
        for _ in 0..reps {
            std::hint::black_box(c.load(Some(h)));
        }
        let load = t0.elapsed().as_secs_f64() / reps as f64 * 1e9;
        let (jac, f) = (c.jac.clone(), c.f.clone());
        let t0 = std::time::Instant::now();
        for _ in 0..reps {
            c.jac.copy_from_slice(&jac);
            c.f.copy_from_slice(&f);
            c.linear_solve(1);
        }
        let solve = t0.elapsed().as_secs_f64() / reps as f64 * 1e9;
        let t0 = std::time::Instant::now();
        for _ in 0..reps {
            c.jac.copy_from_slice(&jac);
            c.f.copy_from_slice(&f);
            std::hint::black_box(&c.jac);
        }
        let copy = t0.elapsed().as_secs_f64() / reps as f64 * 1e9;
        let lu = &c.lu;
        let flops: usize = (0..n)
            .map(|col| {
                (lu.rows_off[col + 1] - lu.rows_off[col]) as usize
                    * (lu.cols_off[col + 1] - lu.cols_off[col]) as usize
            })
            .sum();
        println!(
            "{name}: {n} solved nodes, {} parts; load {load:.0} ns; linear solve {:.0} ns ({flops} multiply-adds in the elimination); a matrix copy {copy:.0} ns",
            c.parts.len(),
            solve - copy
        );
        // The load by kind: no parts (the zeroing and GMIN), then each kind alone.
        let kinds = [
            "none",
            "resistor",
            "capacitor",
            "diode",
            "junction",
            "diffusion",
            "bjt",
            "jfet",
        ];
        let mut line = String::new();
        for (k, name) in kinds.iter().enumerate() {
            c.only = Some(if k == 0 { usize::MAX } else { k - 1 });
            let t0 = std::time::Instant::now();
            for _ in 0..reps {
                std::hint::black_box(c.load(Some(h)));
            }
            let t = t0.elapsed().as_secs_f64() / reps as f64 * 1e9;
            line.push_str(&format!("{name} {t:.0} ns, "));
        }
        c.only = None;
        println!("  the load with only: {line}");
        println!("  parts: {:?}", c.census());
        let ms: Vec<f64> = c
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::Junction { cap, .. } | Part::Diode { cap, .. } => Some(cap.m),
                _ => None,
            })
            .collect();
        println!("  grading exponents: {ms:?}");
    }

    /// The elimination before it was recorded (partial pivoting over the nonzeros by value):
    /// the reference the replayed one must match to the bit.
    fn reference(
        jac: &mut [f64],
        dx: &mut [f64],
        n: usize,
        pattern: ([u64; MAX_SOLVED], [u64; MAX_SOLVED]),
    ) {
        let (mut rows_in, mut cols_in) = pattern;
        let mut nz = vec![0usize; n * n];
        let mut nz_len = vec![0usize; n];
        for col in 0..n {
            let below = |mask: u64| mask & (u64::MAX << col) & !(1u64 << col);
            let mut piv = col;
            let mut rows = below(rows_in[col]);
            while rows != 0 {
                let r = rows.trailing_zeros() as usize;
                rows &= rows - 1;
                if jac[r * n + col].abs() > jac[piv * n + col].abs() {
                    piv = r;
                }
            }
            if piv != col {
                for (c, m) in rows_in.iter_mut().enumerate().take(n).skip(col) {
                    jac.swap(col * n + c, piv * n + c);
                    if (*m >> col) & 1 != (*m >> piv) & 1 {
                        *m ^= (1 << col) | (1 << piv);
                    }
                }
                cols_in.swap(col, piv);
                dx.swap(col, piv);
            }
            let d = jac[col * n + col];
            let mut count = 0;
            let mut cols = cols_in[col] & (u64::MAX << col);
            while cols != 0 {
                let c = cols.trailing_zeros() as usize;
                cols &= cols - 1;
                if jac[col * n + c] != 0.0 {
                    nz[col * n + count] = c;
                    count += 1;
                }
            }
            let fill = cols_in[col] & (u64::MAX << col);
            nz_len[col] = count;
            let mut rows = below(rows_in[col]);
            while rows != 0 {
                let r = rows.trailing_zeros() as usize;
                rows &= rows - 1;
                let factor = jac[r * n + col] / d;
                if factor != 0.0 {
                    for k in 0..count {
                        let c = nz[col * n + k];
                        jac[r * n + c] -= factor * jac[col * n + c];
                        rows_in[c] |= 1 << r;
                    }
                    cols_in[r] |= fill;
                    dx[r] -= factor * dx[col];
                }
            }
        }
        for col in (0..n).rev() {
            let mut s = dx[col];
            for k in 0..nz_len[col] {
                let c = nz[col * n + k];
                if c > col {
                    s -= jac[col * n + c] * dx[c];
                }
            }
            dx[col] = s / jac[col * n + col];
        }
    }

    /// The recorded and replayed elimination gives the reference's solution to the bit over
    /// random sparse systems whose values change between solves, often enough to move the
    /// pivots (some entries at times exactly zero), and records again when they move.
    #[test]
    fn the_replayed_elimination_matches_the_reference_to_the_bit() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let (mut replays, mut rerecords, mut singular) = (0, 0, 0);
        for trial in 0..40 {
            let n = 3 + trial % 20;
            let mut c = Circuit::new(n + 1, 1);
            // A random structure with the diagonal.
            let mut entries = vec![false; n * n];
            let mut pattern = ([0u64; MAX_SOLVED], [0u64; MAX_SOLVED]);
            for r in 0..n {
                for col in 0..n {
                    if r == col || rand() < 0.2 {
                        entries[r * n + col] = true;
                        pattern.0[col] |= 1 << r;
                        pattern.1[r] |= 1 << col;
                    }
                }
            }
            c.pattern = [pattern, pattern];
            for solve in 0..30 {
                // Mostly small changes (the pivots hold), at times large ones.
                let scale = if solve % 7 == 3 { 10.0 } else { 0.01 };
                for (k, on) in entries.iter().enumerate() {
                    c.jac[k] = if !*on || rand() < 0.05 {
                        0.0
                    } else {
                        let base = if k / n == k % n { 2.0 } else { 1.0 };
                        base * (1.0 + scale * (rand() - 0.5))
                            * if rand() < 0.5 { -1.0 } else { 1.0 }
                    };
                }
                for k in 0..n {
                    c.f[k] = rand() - 0.5;
                }
                let (mut jac, mut dx) = (c.jac.clone(), c.f.iter().map(|x| -x).collect::<Vec<_>>());
                reference(&mut jac, &mut dx, n, pattern);
                let was = (c.lu.ready, c.lu.piv);
                c.linear_solve(1);
                if was.0 {
                    if was.1[..n] == c.lu.piv[..n] {
                        replays += 1;
                    } else {
                        rerecords += 1;
                    }
                }
                // A singular system (a zero pivot) gives a correction that is not finite
                // either way, and Newton gives up the step on that alone: there the two
                // need only agree that it is not finite (the replay's extra additions of
                // zeros times an infinite factor make NaNs where the reference leaves
                // infinities).
                let finite = |x: &[f64]| x.iter().all(|v| v.is_finite());
                if !finite(&dx) {
                    assert!(
                        !finite(&c.dx[..n]),
                        "trial {trial}, solve {solve}: {:?}",
                        &c.dx[..n]
                    );
                    singular += 1;
                    continue;
                }
                for (k, (&a, &b)) in c.dx.iter().zip(&dx).enumerate() {
                    assert!(
                        a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0),
                        "trial {trial}, solve {solve}, unknown {k}: {a} replayed, {b} by the reference"
                    );
                }
            }
        }
        println!(
            "{replays} solves replayed, {rerecords} recorded again after a pivot moved, {singular} singular"
        );
        assert!(replays > 100 && rerecords > 10, "{replays} {rerecords}");
    }
}
