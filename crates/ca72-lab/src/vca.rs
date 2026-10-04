//! Board 4's VCAs and output stage on a bench (circuit No. 7, `board4-vca.lib`): the tails'
//! currents against the loudness contour, the static transfer from the first pair's input
//! to the output, the small-signal response and large-signal runs, for the real-time VCA
//! to be checked against.

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::path::Path;

/// Settings for the VCA bench.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VcaBench {
    /// The loudness contour's voltage at pin 9 (into R59).
    pub cont: f64,
    /// R2, R8 (= R28) and R40: Folkman's July 1973 values by default (history.md).
    pub r2: f64,
    pub r8: f64,
    pub r40: f64,
    /// The balance trims' wiper positions (R14 1st VCA BAL, R12 2nd VCA BAL).
    pub r14: f64,
    pub r12: f64,
    /// The main output's load to ground, ohm.
    pub load: f64,
    /// The voltage behind J3's 33K at the EXT. LOUDNESS input: +10 V with the jack empty.
    pub ext: f64,
    /// A source plugged into J3 (V; ideal), its normal contact open; None: the jack empty.
    pub j3: Option<f64>,
}

impl Default for VcaBench {
    fn default() -> Self {
        VcaBench {
            cont: 5.0,
            r2: 160e3,
            r8: 4.7,
            r40: 10e3,
            r14: 0.5,
            r12: 0.5,
            load: 10e3,
            ext: 10.0,
            j3: None,
        }
    }
}

fn header(b: &VcaBench, solver: Solver, lib: &str) -> String {
    let dir = circuits_dir();
    format!(
        "minimoog board 4 vca\n\
         .include {m}\n.include {lib}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp vp 0 10\nvn vn 0 -10\n\
         vcont cont 0 {c}\n\
         {j3}\
         va440 a440 0 0\n\
         rmain main 0 {load}\n",
        m = dir.join("models/mm-devices.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        c = b.cont,
        j3 = match b.j3 {
            Some(v) => format!("vext ext 0 {v}\n"),
            None => format!("vext vx 0 {}\nrj3 vx ext 33k\n", b.ext),
        },
        load = b.load,
    )
}

fn params(b: &VcaBench) -> String {
    format!(
        "r2={} r8={} r40={} r14={} r12={}",
        b.r2, b.r8, b.r40, b.r14, b.r12
    )
}

/// The VCA driven at its input (into R2) by `src`, a source line for node `src`.
pub fn netlist(b: &VcaBench, src: &str, solver: Solver) -> String {
    let lib = circuits_dir().join("boards/board4-vca.lib");
    format!(
        "{}{src}\nx1 src cont ext a440 main phones vp vn mm_vca {}\n",
        header(b, solver, &lib.display().to_string()),
        params(b)
    )
}

/// The operating point's tail currents: Q18's (the first pair's), Q21's (the second's)
/// and Q1's (the output pair's), A; and the output's resting voltage (Q17's collector).
pub fn tails(
    spice: &Ngspice,
    work: &Path,
    b: &VcaBench,
    solver: Solver,
) -> Result<[f64; 4], Error> {
    let net = netlist(b, "vsrc src 0 0", solver);
    let op = spice.run(
        &format!("{net}.save all @q.x1.q18[ic] @q.x1.q21[ic] @q.x1.q1[ic]\n"),
        &["op"],
        work,
    )?;
    let p = &op[0];
    Ok([
        p.scalar("@q.x1.q18[ic]"),
        p.scalar("@q.x1.q21[ic]"),
        -p.scalar("@q.x1.q1[ic]"),
        p.scalar("x1.o17"),
    ])
}

/// The board with the first pair's input driven directly: C6 and R34 replaced by a source
/// of `w` volts between Q16's base and the bias node (port `w`).
fn static_lib(work: &Path) -> Result<std::path::PathBuf, Error> {
    let text = std::fs::read_to_string(circuits_dir().join("boards/board4-vca.lib"))
        .map_err(|e| Error::Raw(e.to_string()))?;
    let subs = [
        (
            ".subckt mm_vca vin cont ext a440 main phones vp vn",
            ".subckt mm_vca_w w vin cont ext a440 main phones vp vn",
        ),
        ("c6    n2 b16 0.33u", "ew    b16 bb w 0 1"),
        ("r34   b16 bb 820", "rn2   n2 0 1meg"),
        (".ends mm_vca", ".ends mm_vca_w"),
    ];
    let mut t = text;
    for (a, z) in subs {
        if !t.contains(a) {
            return Err(Error::Raw(format!("board4-vca.lib has no `{a}`")));
        }
        t = t.replacen(a, z, 1);
    }
    std::fs::create_dir_all(work).map_err(|e| Error::Raw(e.to_string()))?;
    let path = work.join("vca-static.lib");
    std::fs::write(&path, t).map_err(|e| Error::Raw(e.to_string()))?;
    Ok(path)
}

/// The static transfer: the output (Q17's collector, V) against the first pair's input
/// voltage `w` (Q16's base less the bias node), from `from` to `to` V.
pub fn static_transfer(
    spice: &Ngspice,
    work: &Path,
    b: &VcaBench,
    from: f64,
    to: f64,
    step: f64,
    solver: Solver,
) -> Result<Vec<(f64, f64)>, Error> {
    let lib = static_lib(work)?;
    let net = format!(
        "{}vw w 0 0\nvsrc src 0 0\nx1 w src cont ext a440 main phones vp vn mm_vca_w {}\n",
        header(b, solver, &lib.display().to_string()),
        params(b)
    );
    let plots = spice.run(&net, &[&format!("dc vw {from} {to} {step}")], work)?;
    let p = &plots[0];
    Ok(p.vec("w")
        .iter()
        .zip(p.vec("x1.o17"))
        .map(|(&w, &o)| (w, o))
        .collect())
}

/// Small-signal response from the input (into R2) to the main output (loaded): (Hz, dB).
pub fn ac_response(
    spice: &Ngspice,
    work: &Path,
    b: &VcaBench,
    f0: f64,
    f1: f64,
    points: usize,
    solver: Solver,
) -> Result<Vec<(f64, f64)>, Error> {
    let net = netlist(b, "vsrc src 0 dc 0 ac 1", solver);
    let plots = spice.run(&net, &[&format!("ac dec {points} {f0} {f1}")], work)?;
    let p = &plots[0];
    let out = p
        .complex_vec("main")
        .ok_or_else(|| Error::Raw("no v(main)".into()))?;
    Ok(p.vec("frequency")
        .iter()
        .zip(out)
        .map(|(&f, &(re, im))| (f, 10.0 * (re * re + im * im).log10()))
        .collect())
}

/// A transient run with the input's source line given, and optionally the contour's (a
/// source for node `cont` replacing the bench's constant), for `tstop` s with steps no
/// longer than `tmax`.
pub fn transient(
    spice: &Ngspice,
    work: &Path,
    b: &VcaBench,
    (src, cont): (&str, Option<&str>),
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    let mut net = netlist(b, src, solver);
    if let Some(line) = cont {
        let fixed = format!("vcont cont 0 {}\n", b.cont);
        if !net.contains(&fixed) {
            return Err(Error::Raw("no contour source in the bench".into()));
        }
        net = net.replace(&fixed, &format!("{line}\n"));
    }
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
