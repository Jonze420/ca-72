//! Device parameters, the same values as `circuits/models/mm-devices.lib` (a lab test
//! checks they agree), and the few device equations the real-time models use.

/// Boltzmann's constant over the electron charge, V/K.
pub const K_OVER_Q: f64 = 8.617_333_262e-5;

/// Thermal voltage at a temperature in Celsius.
pub fn vt(celsius: f64) -> f64 {
    K_OVER_Q * (celsius + 273.15)
}

/// The Gummel-Poon parameters the real-time models use (DC, forward active region).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bjt {
    /// Saturation current at 25 C, A.
    pub is: f64,
    /// Ideal forward beta.
    pub bf: f64,
    /// Base-emitter leakage saturation current, A, and its emission coefficient.
    pub ise: f64,
    pub ne: f64,
    /// Forward Early voltage, V.
    pub vaf: f64,
    /// Forward high-injection knee current, A (infinite if the model has none).
    pub ikf: f64,
    /// Ideal reverse beta (saturation).
    pub br: f64,
    /// Base, emitter and collector resistance, ohm.
    pub rb: f64,
    pub re: f64,
    pub rc: f64,
    /// Junction capacitances at zero bias (F), built-in potentials (V) and grading
    /// exponents, base-emitter then base-collector; forward transit time (s). Used for the
    /// small parasitic capacitances some models add at their operating point; not
    /// temperature-scaled (docs/circuit/assumptions.md A4).
    pub cje: f64,
    pub vje: f64,
    pub mje: f64,
    pub cjc: f64,
    pub vjc: f64,
    pub mjc: f64,
    pub tf: f64,
    /// Temperature laws: saturation current exponent, beta exponent, energy gap (eV); the
    /// parameters above are at `tnom` C.
    pub xti: f64,
    pub xtb: f64,
    pub eg: f64,
    pub tnom: f64,
}

/// One transistor of a CA3046 array (`CA3046_NPN`, data-sheet derived; components.md).
pub const CA3046: Bjt = Bjt {
    is: 9.3e-16,
    bf: 110.0,
    ise: 9.1e-13,
    ne: 2.0,
    vaf: 64.0,
    ikf: f64::INFINITY,
    br: 1.0,
    rb: 100.0,
    re: 2.0,
    rc: 20.0,
    cje: 1.0e-12,
    vje: 0.75,
    mje: 0.33,
    cjc: 1.0e-12,
    vjc: 0.75,
    mjc: 0.33,
    tf: 0.28e-9,
    xti: 3.0,
    xtb: 1.5,
    eg: 1.11,
    tnom: 25.0,
};

/// A differential pair behind its series drop: the solution of z = a - b tanh(z / two_vt)
/// (a: the pair's open-circuit input, b: the drop per unit of its tanh), as tanh(z / two_vt)
/// and its derivative in a.
pub fn degenerated(a: f64, b: f64, two_vt: f64) -> (f64, f64) {
    let mut z = f64::NAN;
    degenerated_from(a, b, two_vt, &mut z)
}

/// [`degenerated`] from a warm start `z` (the same pair's last solution; NaN for none),
/// which it updates: the same solution to its tolerance in fewer steps (performance).
pub fn degenerated_from(a: f64, b: f64, two_vt: f64, z: &mut f64) -> (f64, f64) {
    let mut x = if z.is_finite() {
        *z
    } else {
        a / (1.0 + b / two_vt)
    };
    let mut t = 0.0;
    let mut last = 0.0;
    for _ in 0..8 {
        t = crate::ulp::tanh(x / two_vt);
        last = (x + b * t - a) / (1.0 + b * (1.0 - t * t) / two_vt);
        x -= last;
        if last.abs() <= 1e-15 * (1.0 + a.abs()) {
            break;
        }
    }
    if last.abs() > 1e-15 * (1.0 + a.abs()) {
        crate::unconverged::note(crate::unconverged::Solver::DegeneratedPair);
    }
    *z = x;
    // tanh at the final x from the last one: the step was at most 1e-15 (1 + |a|), so the
    // first-order update is exact to far below an ulp (one tanh fewer a call).
    let t = if last.abs() <= 1e-12 * (1.0 + a.abs()) {
        t - (1.0 - t * t) * last / two_vt
    } else {
        crate::ulp::tanh(x / two_vt)
    };
    let s = (1.0 - t * t) / two_vt;
    (t, s / (1.0 + b * s))
}

/// A pair's warm start for [`degenerated_warm`]: its last solution, and an anchor, a point
/// whose tanh was taken exactly (libm) near which tanh follows from the anchor's by the
/// addition formula (performance).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairWarm {
    z: f64,
    at: f64,
    t: f64,
}

impl PairWarm {
    /// No solution yet.
    pub const COLD: PairWarm = PairWarm {
        z: f64::NAN,
        at: f64::NAN,
        t: f64::NAN,
    };

    /// tanh(u): within 0.01 of the anchor from the anchor's exact value,
    /// tanh(a + d) = (tanh a + tanh d) / (1 + tanh a tanh d) with tanh d to its d^7 term
    /// (what that leaves is under 3e-18 of tanh d), within an ulp or two of libm's; farther,
    /// libm's (inside a High Fidelity or Potato tick the fast one, within 2e-14 of it:
    /// decisions.md R13), which becomes the anchor. Every value comes from an exact anchor,
    /// so the rounding does not accumulate.
    fn tanh(&mut self, u: f64) -> f64 {
        let d = u - self.at;
        if d.abs() < 0.01 {
            let d2 = d * d;
            let td = d * (1.0 - d2 * (1.0 / 3.0 - d2 * (2.0 / 15.0 - d2 * (17.0 / 315.0))));
            (self.t + td) / (1.0 + self.t * td)
        } else {
            let t = crate::ulp::tanh_in_tick(u);
            self.at = u;
            self.t = t;
            t
        }
    }
}

/// [`degenerated_from`] with its tanh from the pair's anchor ([`PairWarm`]): the same
/// solution to within an ulp or two of its tanh, most of libm's tanh calls saved.
pub fn degenerated_warm(a: f64, b: f64, two_vt: f64, w: &mut PairWarm) -> (f64, f64) {
    degenerated_warm_to(a, b, two_vt, w, 1e-15)
}

/// [`degenerated_warm`] to a relative tolerance `tol` of its Newton step (Potato's
/// looser).
pub fn degenerated_warm_to(a: f64, b: f64, two_vt: f64, w: &mut PairWarm, tol: f64) -> (f64, f64) {
    let mut x = if w.z.is_finite() {
        w.z
    } else {
        a / (1.0 + b / two_vt)
    };
    let mut t = 0.0;
    let mut last = 0.0;
    for _ in 0..8 {
        t = w.tanh(x / two_vt);
        last = (x + b * t - a) / (1.0 + b * (1.0 - t * t) / two_vt);
        x -= last;
        if last.abs() <= tol * (1.0 + a.abs()) {
            break;
        }
    }
    if last.abs() > tol * (1.0 + a.abs()) {
        crate::unconverged::note(crate::unconverged::Solver::DegeneratedPair);
    }
    w.z = x;
    let t = if last.abs() <= 1e-12 * (1.0 + a.abs()) {
        t - (1.0 - t * t) * last / two_vt
    } else {
        w.tanh(x / two_vt)
    };
    let s = (1.0 - t * t) / two_vt;
    (t, s / (1.0 + b * s))
}

/// The diode parameters the real-time models use (DC).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Diode {
    pub is: f64,
    pub n: f64,
    pub rs: f64,
    /// Temperature laws: energy gap (eV) and saturation current exponent; `is` is at 25 C.
    pub eg: f64,
    pub xti: f64,
}

impl Diode {
    /// The junction's current law at a temperature: (I, dI/dV) at the junction voltage.
    pub fn law(&self, celsius: f64) -> impl Fn(f64) -> (f64, f64) + '_ {
        let d = self.at(celsius);
        move |v: f64| d.current(v)
    }

    /// The law's constants at a temperature ([`Diode::law`]'s, to keep).
    pub fn at(&self, celsius: f64) -> DiodeAt {
        let t = celsius + 273.15;
        let tn = 298.15;
        let nvt = self.n * K_OVER_Q * t;
        let is = self.is
            * crate::ulp::pow(t / tn, self.xti / self.n)
            * crate::ulp::exp((t / tn - 1.0) * self.eg / nvt);
        DiodeAt { is, nvt }
    }
}

/// A diode's law at a temperature ([`Diode::at`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiodeAt {
    is: f64,
    nvt: f64,
}

impl DiodeAt {
    /// (I, dI/dV) at the junction voltage.
    #[inline(always)]
    pub fn current(&self, v: f64) -> (f64, f64) {
        let e = crate::ulp::exp((v / self.nvt).min(80.0));
        (self.is * (e - 1.0), self.is * e / self.nvt)
    }
}

/// A junction in series with a resistance `r` across a total voltage `v`: the junction's
/// voltage and the current, for an increasing current law `law` (current and slope at a
/// junction voltage). Safeguarded Newton on the junction voltage.
pub fn series_junction(v: f64, r: f64, law: impl Fn(f64) -> (f64, f64)) -> (f64, f64) {
    series_junction_from(v, r, law, if v > 0.0 { v.min(0.7) } else { v })
}

/// [`series_junction`] from a guess of the junction's voltage (the last sample's, say).
/// Newton's method safeguarded by bisection (it bisects whenever Newton would leave the
/// bracket or is not halving the residual: the exponential law makes plain Newton creep
/// from a guess far on the forward side). A residual of exactly zero is the root, and a
/// Newton point on the bracket's end is inside it: the root sits within rounding of an end
/// whenever the junction carries next to nothing, and rejecting the point there sent the
/// solve bisecting away from the root it had found (performance: tens of iterations
/// to one or two, the root now exact where it was within 1e-13 V).
pub fn series_junction_from(
    v: f64,
    r: f64,
    law: impl Fn(f64) -> (f64, f64),
    guess: f64,
) -> (f64, f64) {
    if r <= 0.0 {
        return (v, law(v).0);
    }
    // The root of g(vj) = vj + r I(vj) - v lies between min(v, 0) and max(v, 0).
    let (mut lo, mut hi) = if v >= 0.0 { (0.0, v) } else { (v, 0.0) };
    let g = |vj: f64| {
        let (i, di) = law(vj);
        (vj + r * i - v, 1.0 + r * di)
    };
    let mut vj = guess.clamp(lo, hi);
    let mut dx_old = hi - lo;
    for _ in 0..200 {
        let (gv, dg) = g(vj);
        if gv == 0.0 {
            return (vj, law(vj).0);
        }
        if gv > 0.0 {
            hi = vj;
        } else {
            lo = vj;
        }
        let newton = vj - gv / dg;
        let dx = if newton < lo || newton > hi || (2.0 * gv).abs() > (dx_old * dg).abs() {
            let mid = 0.5 * (lo + hi);
            let d = vj - mid;
            vj = mid;
            d
        } else {
            let d = vj - newton;
            vj = newton;
            d
        };
        dx_old = dx;
        if dx.abs() < 1e-13 || hi - lo < 1e-13 {
            return (vj, law(vj).0);
        }
    }
    crate::unconverged::note(crate::unconverged::Solver::SeriesJunction);
    (vj, law(vj).0)
}

/// A junction's depletion capacitance at a forward voltage `v` (SPICE's law, linear above
/// FC VJ with FC = 0.5).
pub fn junction_capacitance(cj: f64, vj: f64, m: f64, v: f64) -> f64 {
    JunctionCapacitance::new(cj, vj, m).at(v)
}

/// [`junction_capacitance`] for one junction, its constant factor taken once
/// (performance: the same values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JunctionCapacitance {
    cj: f64,
    vj: f64,
    m: f64,
    /// (1 - FC)^-(1 + M), the linear part's factor.
    lin: f64,
}

impl JunctionCapacitance {
    const FC: f64 = 0.5;

    pub fn new(cj: f64, vj: f64, m: f64) -> JunctionCapacitance {
        JunctionCapacitance {
            cj,
            vj,
            m,
            lin: crate::ulp::pow(1.0 - Self::FC, -(1.0 + m)),
        }
    }

    pub fn at(&self, v: f64) -> f64 {
        let (cj, vj, m) = (self.cj, self.vj, self.m);
        if v < Self::FC * vj {
            cj * crate::ulp::pow(1.0 - v / vj, -m)
        } else {
            cj * self.lin * (1.0 - Self::FC * (1.0 + m) + m * v / vj)
        }
    }
}

/// A transistor's temperature-scaled parameters, as ngspice's BJT model computes them
/// (bjttemp.c: IS scales by exp((T/Tnom - 1) EG/Vt) (T/Tnom)^XTI, BF by (T/Tnom)^XTB,
/// ISE by the IS factor to the power 1/NE, divided by BF's factor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BjtAt {
    pub vt: f64,
    pub is: f64,
    pub bf: f64,
    pub ise: f64,
    pub ne: f64,
    /// Not temperature-scaled (ngspice does not scale it).
    pub ikf: f64,
    /// Scaled like BF.
    pub br: f64,
}

impl Bjt {
    /// The parameters at a temperature in C.
    pub fn at(&self, celsius: f64) -> BjtAt {
        let t = celsius + 273.15;
        let tn = self.tnom + 273.15;
        let vt = K_OVER_Q * t;
        let ratlog = crate::ulp::log(t / tn);
        let factlog = (t / tn - 1.0) * self.eg / vt + self.xti * ratlog;
        let bfactor = crate::ulp::exp(ratlog * self.xtb);
        BjtAt {
            vt,
            is: self.is * crate::ulp::exp(factlog),
            bf: self.bf * bfactor,
            ise: self.ise * crate::ulp::exp(factlog / self.ne) / bfactor,
            ne: self.ne,
            ikf: self.ikf,
            br: self.br * bfactor,
        }
    }
}

impl BjtAt {
    /// The base-emitter junction's current law in the forward active region (the base
    /// current at a base-emitter voltage, Vbc far negative): for [`series_junction`].
    pub fn base_law(&self) -> impl Fn(f64) -> (f64, f64) + '_ {
        move |v: f64| {
            let e = crate::ulp::exp((v / self.vt).min(80.0));
            let e2 = crate::ulp::exp((v / (self.ne * self.vt)).min(80.0));
            (
                self.is / self.bf * (e - 1.0) + self.ise * (e2 - 1.0),
                self.is / self.bf * e / self.vt + self.ise * e2 / (self.ne * self.vt),
            )
        }
    }

    /// The forward transport current I_f = IS e^(Vbe/Vt) for a collector current and the
    /// base-charge factor qb (Gummel-Poon, VAR infinite): Ic = I_f / qb with
    /// qb = q1 (1 + sqrt(1 + 4 q2)) / 2, q1 = 1 / (1 - Vbc/VAF), q2 = I_f / IKF. With
    /// I_f = Ic qb that is (2 qb / q1 - 1)^2 = 1 + 4 Ic qb / IKF, whose root is
    /// qb = q1 (1 + q1 Ic / IKF): solved in closed form (performance: six fixed-point
    /// passes gave the same to rounding at the ladder's currents, within 1e-12 at 1 mA).
    fn transport(&self, ic: f64, vbc: f64, vaf: f64) -> (f64, f64) {
        let q1 = 1.0 / (1.0 - vbc / vaf);
        let ic = ic.max(1e-30);
        let qb = q1 * (1.0 + q1 * ic / self.ikf);
        (ic * qb, qb)
    }

    /// Base current for a collector current in the forward active region, with the
    /// base-collector voltage for the Early effect:
    /// Ib = I_f / BF + ISE e^(Vbe/(NE Vt)).
    pub fn base_current(&self, ic: f64, vbc: f64, vaf: f64) -> f64 {
        let (i_f, _) = self.transport(ic, vbc, vaf);
        let vbe = self.vt * crate::ulp::log(i_f / self.is);
        i_f / self.bf + self.ise * crate::ulp::exp(vbe / (self.ne * self.vt))
    }

    /// The base current's slope against the collector current (one over the small-signal
    /// beta) at a collector current, from the same laws.
    pub fn base_slope(&self, ic: f64, vbc: f64, vaf: f64) -> f64 {
        let (i_f, qb) = self.transport(ic, vbc, vaf);
        let vbe = self.vt * crate::ulp::log(i_f / self.is);
        let ib_ise = self.ise * crate::ulp::exp(vbe / (self.ne * self.vt));
        (1.0 / self.bf + ib_ise / (self.ne * i_f)) * self.transport_slope(ic, i_f, qb, vbc, vaf)
    }

    /// dI_f / dIc.
    fn transport_slope(&self, ic: f64, i_f: f64, qb: f64, vbc: f64, vaf: f64) -> f64 {
        let q1 = 1.0 / (1.0 - vbc / vaf);
        let dqb = q1 / (self.ikf * libm::sqrt(1.0 + 4.0 * i_f / self.ikf));
        qb / (1.0 - ic.max(1e-30) * dqb)
    }

    /// The internal base-emitter voltage for a collector current in the forward active
    /// region.
    pub fn junction_voltage(&self, ic: f64, vbc: f64, vaf: f64) -> f64 {
        let (i_f, _) = self.transport(ic, vbc, vaf);
        self.vt * crate::ulp::log1p(i_f / self.is)
    }

    /// The transconductance's factor against the ideal Ic / Vt at a collector current:
    /// high injection lowers it by 1 - I_f qb' / qb.
    pub fn gm_factor(&self, ic: f64, vbc: f64, vaf: f64) -> f64 {
        let (i_f, qb) = self.transport(ic, vbc, vaf);
        let q1 = 1.0 / (1.0 - vbc / vaf);
        let dqb = q1 / (self.ikf * libm::sqrt(1.0 + 4.0 * i_f / self.ikf));
        1.0 - i_f * dqb / qb
    }

    /// Collector and base currents at the internal junction voltages (NPN polarity: for
    /// a PNP pass emitter-base and collector-base voltages), in every region: Gummel-Poon
    /// with NF = NR = 1, ISC = 0, VAR and IKR infinite, as ngspice's BJT model computes
    /// them (bjtload.c).
    pub fn currents(&self, vbe: f64, vbc: f64, vaf: f64) -> (f64, f64) {
        let i_f = self.is * crate::ulp::expm1(vbe / self.vt);
        let i_r = self.is * crate::ulp::expm1(vbc / self.vt);
        let q1 = 1.0 / (1.0 - vbc / vaf);
        let q2 = i_f / self.ikf;
        let qb = 0.5 * q1 * (1.0 + libm::sqrt((1.0 + 4.0 * q2).max(0.0)));
        let ib =
            i_f / self.bf + self.ise * crate::ulp::expm1(vbe / (self.ne * self.vt)) + i_r / self.br;
        let ic = (i_f - i_r) / qb - i_r / self.br;
        (ic, ib)
    }

    /// [`BjtAt::currents`] with their slopes against the two junction voltages (for
    /// Newton's method on a circuit); the exponentials are capped at e^80.
    pub fn currents_d(&self, vbe: f64, vbc: f64, vaf: f64) -> Currents {
        // `exp(x)` and `expm1(x)` from one exponential where they agree to an ulp (away
        // from 0, where `exp(x) - 1` loses nothing), both evaluated only near 0
        // (performance).
        let exp_expm1 = |x: f64| {
            let e = crate::ulp::exp(x);
            if x.abs() < 0.5 {
                (e, crate::ulp::expm1(x))
            } else {
                (e, e - 1.0)
            }
        };
        let (xf, xr) = ((vbe / self.vt).min(80.0), (vbc / self.vt).min(80.0));
        let (ef, em1f) = exp_expm1(xf);
        let i_f = self.is * em1f;
        let di_f = self.is * ef / self.vt;
        let (er, em1r) = exp_expm1(xr);
        let i_r = self.is * em1r;
        let di_r = self.is * er / self.vt;
        let q1 = 1.0 / (1.0 - vbc / vaf);
        let dq1 = q1 * q1 / vaf;
        let sq = libm::sqrt((1.0 + 4.0 * i_f / self.ikf).max(0.0));
        let qb = 0.5 * q1 * (1.0 + sq);
        let dqb_f = if sq > 0.0 {
            q1 * di_f / (self.ikf * sq)
        } else {
            0.0
        };
        let dqb_r = 0.5 * (1.0 + sq) * dq1;
        let xe = (vbe / (self.ne * self.vt)).min(80.0);
        let (ee, em1e) = exp_expm1(xe);
        let ib = i_f / self.bf + self.ise * em1e + i_r / self.br;
        let dib = [
            di_f / self.bf + self.ise * ee / (self.ne * self.vt),
            di_r / self.br,
        ];
        let net = i_f - i_r;
        let ic = net / qb - i_r / self.br;
        let dic = [
            di_f / qb - net * dqb_f / (qb * qb),
            -di_r / qb - net * dqb_r / (qb * qb) - di_r / self.br,
        ];
        Currents { ic, ib, dic, dib }
    }
}

/// A transistor's collector and base currents and their slopes against the base-emitter
/// and base-collector voltages ([`BjtAt::currents_d`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Currents {
    pub ic: f64,
    pub ib: f64,
    pub dic: [f64; 2],
    pub dib: [f64; 2],
}

/// Terminal voltages as a function of a transistor's currents, for [`solve_junctions`]:
/// given the collector and base currents (in their normal directions: into an NPN's
/// terminals, out of a PNP's), the base, emitter and collector node voltages.
pub trait Terminals {
    fn at(&self, ic: f64, ib: f64) -> (f64, f64, f64);
}

impl<F: Fn(f64, f64) -> (f64, f64, f64)> Terminals for F {
    fn at(&self, ic: f64, ib: f64) -> (f64, f64, f64) {
        self(ic, ib)
    }
}

/// A transistor's operating point where its terminals depend on its currents: Newton's
/// method on its two internal junction voltages (base-emitter, base-collector; for a PNP
/// emitter-base and collector-base), with SPICE-like step limits, from `x`. `pnp` flips
/// the polarities. Returns the junction voltages and (ic, ib).
pub fn solve_junctions(
    m: &BjtAt,
    q: &Bjt,
    pnp: bool,
    x: [f64; 2],
    terminals: impl Terminals,
) -> ([f64; 2], f64, f64) {
    solve_junctions_limited(m, q, pnp, x, terminals, 60)
}

/// A tail transistor and the balanced pair whose emitters it feeds (the VCA's Q18, Q21 and
/// Q1 and their pairs), for [`solve_tail_pair`].
#[derive(Debug, Clone, Copy)]
pub struct TailPair<'a> {
    pub tail: (&'a BjtAt, &'a Bjt),
    pub pair: (&'a BjtAt, &'a Bjt),
    /// PNPs (the output pair and Q1): the polarities flipped.
    pub pnp: bool,
    /// The tail's base node: `vb0` behind `rb` (its base current through it), and its
    /// emitter node: `ve0` and `re` (its emitter current through it), in the tail's own
    /// polarity (an NPN's base falls with its base current, its emitter rises with its
    /// emitter current).
    pub vb0: f64,
    pub rb: f64,
    pub ve0: f64,
    pub re: f64,
    /// The pair's bases' mean voltage, and its base-collector voltage (the Early effect).
    pub mid: f64,
    pub pair_vbc: f64,
    /// Resistance in each pair emitter besides RE (the output pair's R8), and from the
    /// tail's collector to the pair's emitters (R35, R33; none for Q1).
    pub r_emitter: f64,
    pub r_collector: f64,
}

/// Solves a tail transistor and the balanced pair it feeds together: the tail's two
/// junction voltages and the pair's base-emitter junction (each side's, the pair balanced)
/// by Newton's method from `x`, Kirchhoff's current law at the pair's emitters closing the
/// system, with the Jacobian from the devices' slopes. Every current is a forward function
/// of a junction voltage (Gummel-Poon in every region), so a tail that overdrives its pair,
/// cutting it off, settles as well as one that feeds it: solved apart, the pair's drop at
/// the tail's current turns steep there and the iterates circled. The step is shortened as
/// a whole to SPICE-like limits (2 Vt forward, 0.2 V back) and halved while it does not
/// lower the residual. Returns the junction voltages, the tail's (ic, ib) and the pair's
/// emitter node voltage.
pub fn solve_tail_pair(
    tp: &TailPair<'_>,
    mut x: [f64; 3],
    iterations: usize,
) -> ([f64; 3], f64, f64, f64) {
    let (mt, qt) = tp.tail;
    let (mp, qp) = tp.pair;
    let p = if tp.pnp { -1.0 } else { 1.0 };
    // The current law's residual in volts (a milliamp is a volt).
    const RS: f64 = 1e3;
    // Series resistances: the base's, the emitter's and from the collector to the pair.
    let rb = tp.rb + qt.rb;
    let re = tp.re + qt.re;
    let rc = tp.r_collector + qt.rc;
    let rep = qp.re + tp.r_emitter;
    // The residuals, their Jacobian, the tail's currents and the pair's emitter node.
    let eval = |x: [f64; 3]| {
        let t = mt.currents_d(x[0], x[1], qt.vaf);
        let q = mp.currents_d(x[2], tp.pair_vbc, qp.vaf);
        let ieh = q.ic + q.ib;
        let dieh = q.dic[0] + q.dib[0];
        let e = tp.mid - p * (x[2] + qp.rb * q.ib + rep * ieh);
        let r = [
            p * (tp.vb0 - tp.ve0) - rb * t.ib - re * (t.ic + t.ib) - x[0],
            p * (tp.vb0 - e) - rb * t.ib + rc * t.ic - x[1],
            (2.0 * ieh - t.ic) * RS,
        ];
        // Each residual's slopes against the tail's two junctions, then the pair's.
        let col = |k: usize| {
            [
                -rb * t.dib[k] - re * (t.dic[k] + t.dib[k]),
                -rb * t.dib[k] + rc * t.dic[k],
                -t.dic[k] * RS,
            ]
        };
        let (c0, c1) = (col(0), col(1));
        let j = [
            [c0[0] - 1.0, c1[0], 0.0],
            [c0[1], c1[1] - 1.0, 1.0 + qp.rb * q.dib[0] + rep * dieh],
            [c0[2], c1[2], 2.0 * dieh * RS],
        ];
        (r, j, t.ic, t.ib, e)
    };
    let norm = |r: &[f64; 3]| r[0].abs().max(r[1].abs()).max(r[2].abs());
    let vts = [mt.vt, mt.vt, mp.vt];
    let mut cur = eval(x);
    for _ in 0..iterations {
        let Some(dx) = solve3(cur.1, cur.0) else {
            break;
        };
        let want = [-dx[0], -dx[1], -dx[2]];
        let mut f: f64 = 1.0;
        for (w, vt) in want.iter().zip(vts) {
            if *w > 2.0 * vt {
                f = f.min(2.0 * vt / w);
            } else if *w < -0.2 {
                f = f.min(-0.2 / w);
            }
        }
        // Halved while the residual does not fall (at most eight times).
        let n0 = norm(&cur.0);
        let mut step = [want[0] * f, want[1] * f, want[2] * f];
        let mut next = eval([x[0] + step[0], x[1] + step[1], x[2] + step[2]]);
        for _ in 0..8 {
            if norm(&next.0) <= n0 || n0 < 1e-9 {
                break;
            }
            step = [0.5 * step[0], 0.5 * step[1], 0.5 * step[2]];
            next = eval([x[0] + step[0], x[1] + step[1], x[2] + step[2]]);
        }
        x = [x[0] + step[0], x[1] + step[1], x[2] + step[2]];
        cur = next;
        if step.iter().all(|s| s.abs() < 1e-12) {
            return (x, cur.2, cur.3, cur.4);
        }
    }
    crate::unconverged::note(crate::unconverged::Solver::Junctions);
    (x, cur.2, cur.3, cur.4)
}

/// `a x = b` for a 3 by 3 system (Gaussian elimination with partial pivoting); None if it
/// is singular.
fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let piv = (col..3)
            .max_by(|&i, &k| a[i][col].abs().total_cmp(&a[k][col].abs()))
            .unwrap_or(col);
        if a[piv][col] == 0.0 || !a[piv][col].is_finite() {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let pivot = a[col];
        for r in col + 1..3 {
            let f = a[r][col] / pivot[col];
            for (x, y) in a[r].iter_mut().zip(pivot).skip(col) {
                *x -= f * y;
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 3];
    for col in (0..3).rev() {
        let s = b[col] - (col + 1..3).map(|c| a[col][c] * x[c]).sum::<f64>();
        x[col] = s / a[col][col];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// [`solve_junctions`] with at most `iterations` Newton steps: for tracking a slowly
/// moving operating point from the last one.
pub fn solve_junctions_limited(
    m: &BjtAt,
    q: &Bjt,
    pnp: bool,
    mut x: [f64; 2],
    terminals: impl Terminals,
    iterations: usize,
) -> ([f64; 2], f64, f64) {
    let p = if pnp { -1.0 } else { 1.0 };
    let res = |x: [f64; 2]| {
        let (ic, ib) = m.currents(x[0], x[1], q.vaf);
        let (vb, ve, vc) = terminals.at(ic, ib);
        let vbi = vb - p * q.rb * ib;
        let vei = ve + p * q.re * (ic + ib);
        let vci = vc - p * q.rc * ic;
        ([p * (vbi - vei) - x[0], p * (vbi - vci) - x[1]], ic, ib)
    };
    for _ in 0..iterations {
        let (r, _, _) = res(x);
        let h = 1e-7;
        let (r0, _, _) = res([x[0] + h, x[1]]);
        let (r1, _, _) = res([x[0], x[1] + h]);
        let j = [
            [(r0[0] - r[0]) / h, (r1[0] - r[0]) / h],
            [(r0[1] - r[1]) / h, (r1[1] - r[1]) / h],
        ];
        let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
        let dx = [
            (r[0] * j[1][1] - r[1] * j[0][1]) / det,
            (j[0][0] * r[1] - j[1][0] * r[0]) / det,
        ];
        // The Newton step, shortened as a whole until neither junction moves more than 2 Vt
        // forward or 0.2 V back (SPICE-like limits). Clamping each junction's move on its
        // own (as before) turned the step's direction and could leave the iterates
        // circling: a transistor saturating hard (the VCA's Q21 overdriven) never settled.
        let want = [-dx[0], -dx[1]];
        let mut f: f64 = 1.0;
        for w in want {
            if w > 2.0 * m.vt {
                f = f.min(2.0 * m.vt / w);
            } else if w < -0.2 {
                f = f.min(-0.2 / w);
            }
        }
        let step = [want[0] * f, want[1] * f];
        x = [x[0] + step[0], x[1] + step[1]];
        if step[0].abs() < 1e-12 && step[1].abs() < 1e-12 {
            let (_, ic, ib) = res(x);
            return (x, ic, ib);
        }
    }
    crate::unconverged::note(crate::unconverged::Solver::Junctions);
    let (_, ic, ib) = res(x);
    (x, ic, ib)
}
