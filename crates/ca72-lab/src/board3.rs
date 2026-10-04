//! Board 3 on the bench (docs/circuit/board3.md): the noise generator (circuit No. 3,
//! `board3-noise.lib`) with its outputs loaded as the NOISE switch loads them, and the
//! modulation mix amplifier (circuit No. 15, `board3-modmix.lib`).

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::path::Path;

/// The noise generator's bench: R26 and Q15's stand-in, and each output's load to ground
/// (ohm; infinite for none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseBench {
    pub r26: f64,
    pub vbr: f64,
    pub rd: f64,
    pub white: f64,
    pub pink: f64,
    pub red: f64,
}

impl Default for NoiseBench {
    fn default() -> Self {
        NoiseBench {
            r26: 1250.0,
            vbr: 7.5,
            rd: 1e3,
            white: f64::INFINITY,
            pink: f64::INFINITY,
            red: f64::INFINITY,
        }
    }
}

/// The netlist: the source's AC value 1 V.
pub fn noise_netlist(b: &NoiseBench, solver: Solver) -> String {
    let dir = circuits_dir();
    let load = |name: &str, node: &str, r: f64| {
        if r.is_finite() {
            format!("rl{name} {node} 0 {r}\n")
        } else {
            String::new()
        }
    };
    format!(
        "minimoog board 3 noise generator\n.include {m}\n.include {n}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-15 vntol=1e-9\n\
         vp10 p10 0 10\nvn10 n10 0 -10\n\
         x1 white pink red p10 n10 mm_noise r26={r26} vbr={vbr} rd={rd} acn=1\n{lw}{lp}{lr}",
        m = dir.join("models/mm-devices.lib").display(),
        n = dir.join("boards/board3-noise.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        r26 = b.r26,
        vbr = b.vbr,
        rd = b.rd,
        lw = load("w", "white", b.white),
        lp = load("p", "pink", b.pink),
        lr = load("r", "red", b.red),
    )
}

/// The operating point and the AC response over `points` frequencies per decade from
/// `f0` to `f1` Hz: (op, ac).
pub fn noise_ac(
    spice: &Ngspice,
    work: &Path,
    b: &NoiseBench,
    f0: f64,
    f1: f64,
    points: usize,
    solver: Solver,
) -> Result<(Plot, Plot), Error> {
    let net = noise_netlist(b, solver);
    let mut plots = spice.run(
        &net,
        &["op", &format!("ac dec {points} {f0:e} {f1:e}")],
        work,
    )?;
    let ac = plots.pop().ok_or_else(|| Error::Raw("no ac plot".into()))?;
    let op = plots.pop().ok_or_else(|| Error::Raw("no op plot".into()))?;
    Ok((op, ac))
}

/// The modulation mix amplifier's bench: the front panel's MODULATION MIX network (R24 24K
/// from the noise, R23 24K from oscillator 3, R3 25K linear between them with its wiper at
/// GND, `mix` 0 at oscillator 3's end), the amplifier, R57, and the left hand controller's
/// MODULATION wheel (`wheel`, ohm to GND) with a load on the line (`load` ohm to `load_v`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModMixBench {
    pub mix: f64,
    pub wheel: f64,
    pub load: f64,
    pub load_v: f64,
}

impl Default for ModMixBench {
    fn default() -> Self {
        ModMixBench {
            mix: 0.0,
            wheel: 1.2e3,
            load: f64::INFINITY,
            load_v: 0.0,
        }
    }
}

/// The netlist with the two sources' lines given (`vn noise 0 ...`, `vo osc3 0 ...`).
pub fn modmix_netlist(b: &ModMixBench, sources: &str, solver: Solver) -> String {
    let dir = circuits_dir();
    let m = b.mix.clamp(0.0, 1.0);
    // CCW (0): the wiper at the noise's end, so oscillator 3's end sees the whole track.
    let (ra, rb) = ((25e3 * m).max(1e-3), (25e3 * (1.0 - m)).max(1e-3));
    let load = if b.load.is_finite() {
        format!("rload mod lv {}\nvload lv 0 {}\n", b.load, b.load_v)
    } else {
        String::new()
    };
    format!(
        "minimoog board 3 modulation mix\n.include {md}\n.include {mm}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp10 p10 0 10\nvn10 n10 0 -10\n{sources}\n\
         r24 noise a 24k\nr23 osc3 b 24k\nr3a a 0 {ra}\nr3b b 0 {rb}\n\
         x1 a b mod p10 n10 mm_modmix\nrwheel mod 0 {w}\n{load}",
        md = dir.join("models/mm-devices.lib").display(),
        mm = dir.join("boards/board3-modmix.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        w = b.wheel.max(1e-3),
    )
}

/// The dual regulator's bench (circuit No. 10, `board3-regulator.lib`): the rectifier's
/// unregulated rails (Figure 9-13: 15 V AC each side of the centre tap into 1000 uF, about
/// +-20 V), the loads each rail carries (A), and the two trims' wipers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegulatorBench {
    pub unp: f64,
    pub unn: f64,
    pub i_p: f64,
    pub i_n: f64,
    pub a21: f64,
    pub a58: f64,
}

impl Default for RegulatorBench {
    fn default() -> Self {
        RegulatorBench {
            unp: 20.0,
            unn: -20.0,
            i_p: 0.15,
            i_n: 0.15,
            a21: 0.5,
            a58: 0.5,
        }
    }
}

/// Where the regulator's AC stimulus goes: a 1 A current drawn from a rail (its output
/// impedance, and the other rail's response), or 1 V on an unregulated rail (ripple).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegulatorAc {
    None,
    LoadP,
    LoadN,
    LineP,
    LineN,
}

/// The netlist: loads as current sources (drawn from +10 V into GND, from GND into -10 V).
pub fn regulator_netlist(b: &RegulatorBench, ac: RegulatorAc, solver: Solver) -> String {
    let dir = circuits_dir();
    let on = |a: RegulatorAc| if ac == a { " ac 1" } else { "" };
    format!(
        "minimoog board 3 dual regulator\n.include {md}\n.include {rg}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7\n\
         vunp unp 0 dc {up}{lp}\nvunn unn 0 dc {un}{ln}\n\
         ip outp 0 dc {ip}{zp}\nin 0 outn dc {inn}{zn}\n\
         x1 unp unn outp outn mm_regulator a21={a21} a58={a58}\n",
        md = dir.join("models/mm-devices.lib").display(),
        rg = dir.join("boards/board3-regulator.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        up = b.unp,
        un = b.unn,
        lp = on(RegulatorAc::LineP),
        ln = on(RegulatorAc::LineN),
        ip = b.i_p,
        inn = b.i_n,
        zp = on(RegulatorAc::LoadP),
        zn = on(RegulatorAc::LoadN),
        a21 = b.a21,
        a58 = b.a58,
    )
}

/// The operating point: every node's voltage and branch current.
pub fn regulator_op(
    spice: &Ngspice,
    work: &Path,
    b: &RegulatorBench,
    solver: Solver,
) -> Result<Plot, Error> {
    let net = regulator_netlist(b, RegulatorAc::None, solver);
    spice
        .run(&net, &["op"], work)?
        .pop()
        .ok_or_else(|| Error::Raw("no op plot".into()))
}

/// The AC response to `ac` from `f0` to `f1` Hz, `points` per decade.
pub fn regulator_ac(
    spice: &Ngspice,
    work: &Path,
    b: &RegulatorBench,
    ac: RegulatorAc,
    (f0, f1, points): (f64, f64, usize),
    solver: Solver,
) -> Result<Plot, Error> {
    let net = regulator_netlist(b, ac, solver);
    spice
        .run(
            &net,
            &["op", &format!("ac dec {points} {f0:e} {f1:e}")],
            work,
        )?
        .pop()
        .ok_or_else(|| Error::Raw("no ac plot".into()))
}

/// Trims the regulator as the service manual's 5.6 does: R21 for +10.000 V, then R58 for
/// -10.000 V (it tracks the +10 V). Bisection on each wiper, its direction from the ends;
/// the bench with its trims set, or an error when a trim cannot reach its rail.
pub fn regulator_trim(
    spice: &Ngspice,
    work: &Path,
    b: &RegulatorBench,
    solver: Solver,
) -> Result<RegulatorBench, Error> {
    let mut b = *b;
    let v = |b: &RegulatorBench, node: &str| -> Result<f64, Error> {
        Ok(regulator_op(spice, work, b, solver)?.scalar(node))
    };
    let trim = |b: &mut RegulatorBench,
                wiper: fn(&mut RegulatorBench) -> &mut f64,
                node: &str,
                target: f64|
     -> Result<(), Error> {
        *wiper(b) = 0.0;
        let at0 = v(b, node)?;
        *wiper(b) = 1.0;
        let at1 = v(b, node)?;
        if (at0 - target) * (at1 - target) > 0.0 {
            return Err(Error::Raw(format!(
                "{node} runs {at0:.4} to {at1:.4} V over its trim, not through {target} V"
            )));
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            *wiper(b) = 0.5 * (lo + hi);
            if (v(b, node)? < target) == (at0 < target) {
                lo = *wiper(b);
            } else {
                hi = *wiper(b);
            }
        }
        Ok(())
    };
    trim(&mut b, |b| &mut b.a21, "v(outp)", 10.0)?;
    trim(&mut b, |b| &mut b.a58, "v(outn)", -10.0)?;
    Ok(b)
}
