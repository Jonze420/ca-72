//! Board 4's filter on a bench: its control current, frequency response and large-signal
//! behaviour, for the real-time filter to be checked against.

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::path::Path;

/// Front panel settings for the filter bench.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VcfBench {
    /// The CUTOFF FREQUENCY control's wiper voltage (its 5K pot spans -10..+10 V), into R55.
    pub cutoff: f64,
    /// The keyboard voltage into R53 300K and R54 150K (both keyboard control switches on).
    pub kbd: f64,
    /// EMPHASIS: R14's resistance in circuit (0 ohm at 10, 50K at 0).
    pub r14: f64,
    /// R73 REGEN CAL wiper position, R39 RANGE wiper, R49 SCALE ohms.
    pub r73: f64,
    pub r39: f64,
    pub r49: f64,
    /// The signal's source: one mixer channel's 33K from this AC/transient source.
    pub mix_r: f64,
    /// The rear FILTER control jack's source, V, into R51 100K (None: R51 left out, as the
    /// benches before the jacks had it).
    pub ext_ctl: Option<f64>,
}

impl Default for VcfBench {
    fn default() -> Self {
        VcfBench {
            cutoff: 0.0,
            kbd: 0.0,
            r14: 50e3,
            r73: 0.5,
            r39: 0.5,
            r49: 250.0,
            mix_r: 33e3,
            ext_ctl: None,
        }
    }
}

/// A netlist of the filter driven by `src` (a source line for node `src`).
pub fn netlist(b: &VcfBench, src: &str, solver: Solver) -> String {
    let dir = circuits_dir();
    format!(
        "minimoog board 4 filter\n\
         .include {m}\n.include {v}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp vp 0 10\nvn vn 0 -10\n\
         vcut vcut 0 {c}\nr55 vcut ctl 200k\nvkbd kbd 0 {k}\nr53 kbd ctl 300k\nr54 kbd ctl 150k\n\
         vamt amt 0 0\n{src}\nrmix src ain {mr}\n\
         r14 emo emi {r14}\n{ext}\
         x1 ain ctl amt emo emi out vp vn mm_vcf r39={r39} r49={r49} r73={r73}\n",
        m = dir.join("models/mm-devices.lib").display(),
        v = dir.join("boards/board4-vcf.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        c = b.cutoff,
        k = b.kbd,
        mr = b.mix_r,
        r14 = b.r14.max(1e-3),
        r39 = b.r39,
        r49 = b.r49,
        r73 = b.r73,
        ext = b.ext_ctl.map_or(String::new(), |v| format!(
            "vx51 x51 0 {v}\nr51 x51 ctl 100k\n"
        )),
    )
}

/// The ladder's current (Q28's collector) at the bench's settings, A.
pub fn ladder_current(
    spice: &Ngspice,
    work: &Path,
    b: &VcfBench,
    solver: Solver,
) -> Result<f64, Error> {
    let net = netlist(b, "vsrc src 0 0", solver);
    let op = spice.run(&format!("{net}.save all @q.x1.q28[ic]\n"), &["op"], work)?;
    Ok(op[0].scalar("@q.x1.q28[ic]"))
}

/// The ladder's current (Q28's collector) and the converter's nodes over a sweep of the
/// CUTOFF control's voltage (the bench's other settings fixed): (cutoff V, I0 A).
pub fn control_sweep(
    spice: &Ngspice,
    work: &Path,
    b: &VcfBench,
    from: f64,
    to: f64,
    step: f64,
    solver: Solver,
) -> Result<Vec<(f64, f64)>, Error> {
    let net = netlist(b, "vsrc src 0 0", solver);
    let plots = spice.run(
        &format!("{net}.save all @q.x1.q28[ic]\n"),
        &[&format!("dc vcut {from} {to} {step}")],
        work,
    )?;
    let p = &plots[0];
    Ok(p.vec("vcut")
        .iter()
        .zip(p.vec("@q.x1.q28[ic]"))
        .map(|(&v, &i)| (v, i))
        .collect())
}

/// Small-signal response from the source (through the mixer resistor) to the filter's
/// output, at `points` frequencies per decade from `f0` to `f1`: (Hz, gain in dB).
pub fn ac_response(
    spice: &Ngspice,
    work: &Path,
    b: &VcfBench,
    f0: f64,
    f1: f64,
    points: usize,
    solver: Solver,
) -> Result<Vec<(f64, f64)>, Error> {
    let net = netlist(b, "vsrc src 0 dc 0 ac 1", solver);
    let plots = spice.run(&net, &[&format!("ac dec {points} {f0} {f1}")], work)?;
    let p = &plots[0];
    let f = p.vec("frequency");
    let out = p
        .complex_vec("out")
        .ok_or_else(|| Error::Raw("no v(out)".into()))?;
    Ok(f.iter()
        .zip(out)
        .map(|(&f, &(re, im))| (f, 10.0 * (re * re + im * im).log10()))
        .collect())
}

/// A transient run with the source's line given (for example a sine), for `tstop` s with
/// steps no longer than `tmax`.
pub fn transient(
    spice: &Ngspice,
    work: &Path,
    b: &VcfBench,
    src: &str,
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    let net = netlist(b, src, solver);
    let plots = spice.run(
        &net,
        &[&format!("tran {tmax:e} {tstop:e} 0 {tmax:e}")],
        work,
    )?;
    plots
        .into_iter()
        .next()
        .ok_or_else(|| Error::Raw("no plot".into()))
}

/// The gain recovery amplifier's DC transfer (Q8, Q6, Q7, Q5 and their resistors, from
/// board4-vcf.lib) against the differential voltage `w` added to the two followers' bases
/// (C5's and C1's sides) at their operating point, with R7's top (nt) held at its
/// operating voltage (the real-time model carries nt, R6 and C9 itself), each as the
/// change from w = 0:
/// `di`, the current through R3 (what `ca72::tables::OUT_DI` holds), and `dib`,
/// Q8's base current less Q6's (`OUT_DIB`: the followers' load on the coupling network).
pub const OUT_W_MIN: f64 = -1.0;
pub const OUT_W_STEP: f64 = 0.002;
pub const OUT_POINTS: usize = 1001;

#[derive(Debug, Clone, PartialEq)]
pub struct OutputStage {
    pub di: Vec<f64>,
    pub dib: Vec<f64>,
}

pub fn output_stage_table(
    spice: &Ngspice,
    work: &Path,
    solver: Solver,
) -> Result<OutputStage, Error> {
    // The operating point of the bases (base currents through R31/R38).
    let b = VcfBench::default();
    let op = spice.run(
        &netlist(&b, "vsrc src 0 0", solver),
        &["op"],
        &work.join("op"),
    )?;
    let (b8, b6, nt) = (
        op[0].scalar("x1.b8"),
        op[0].scalar("x1.b6"),
        op[0].scalar("x1.nt"),
    );
    let dir = circuits_dir();
    let net = format!(
        "gain recovery amplifier\n.include {m}\n\
         .options temp={t} tnom=25 reltol=1e-6 abstol=1e-14 vntol=1e-8\n\
         vp vp 0 10\nvn vn 0 -10\n\
         vw w 0 0\n\
         eb8 b8 0 value={{{b8} + v(w)/2}}\neb6 b6 0 value={{{b6} - v(w)/2}}\n\
         q8 out b8 e8 QTIS97\nq6 vp b6 e6 QTIS97\nr24 e8 vn 220k\nr20 e6 vn 100k\n\
         q7 out e8 e57 QTIS97\nq5 vp e6 e57 QTIS97\nr26 e57 vn 1k\n\
         r3 nu out 1k\nr7 nt nu 180\nvnt nt 0 {nt}\n\
         .save all @q8[ib] @q6[ib]\n",
        m = dir.join("models/mm-devices.lib").display(),
        t = solver.temp,
    );
    let hi = OUT_W_MIN + OUT_W_STEP * (OUT_POINTS - 1) as f64;
    let plots = spice.run(
        &net,
        &[&format!("dc vw {OUT_W_MIN} {hi:.6} {OUT_W_STEP}")],
        &work.join("dc"),
    )?;
    let p = &plots[0];
    let i: Vec<f64> = p
        .vec("nu")
        .iter()
        .zip(p.vec("out"))
        .map(|(u, o)| (u - o) / 1000.0)
        .collect();
    let ib: Vec<f64> = p
        .vec("@q8[ib]")
        .iter()
        .zip(p.vec("@q6[ib]"))
        .map(|(a, b)| a - b)
        .collect();
    if i.len() != OUT_POINTS || ib.len() != OUT_POINTS {
        return Err(Error::Raw(format!("{} points", i.len())));
    }
    let rest = |v: &[f64]| {
        let r = v[OUT_POINTS / 2];
        v.iter().map(|x| x - r).collect()
    };
    Ok(OutputStage {
        di: rest(&i),
        dib: rest(&ib),
    })
}
