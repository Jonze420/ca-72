//! Board 2's dual contour generator on a bench (circuit No. 8, `board2-contour.lib`): a
//! key held for a while and released, with the front panel's ATTACK, DECAY and SUSTAIN
//! controls and the left hand controller's DECAY switch, for the real-time contour
//! generators to be checked against.

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::path::Path;

/// One contour generator's front panel controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourControls {
    /// ATTACK TIME and DECAY TIME: the 1M audio pots' resistance in circuit, ohm (they
    /// are rheostats: the taper is the front panel's concern).
    pub attack: f64,
    pub decay: f64,
    /// SUSTAIN LEVEL: the 5K linear pot's wiper position from its grounded end, 0..1.
    pub sustain: f64,
}

impl Default for ContourControls {
    fn default() -> Self {
        ContourControls {
            attack: 20e3,
            decay: 100e3,
            sustain: 0.5,
        }
    }
}

/// The bench: both generators' controls, the DECAY switch, and the key's timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourBench {
    pub filter: ContourControls,
    pub loudness: ContourControls,
    /// The left hand controller's DECAY switch: on, the release decays at the DECAY
    /// time; off, R1401 1.5K dumps the capacitors.
    pub decay_on: bool,
    /// The key goes down at `key_down` and up at `key_up`, s.
    pub key_down: f64,
    pub key_up: f64,
    /// The filter contour's load: AMOUNT OF CONTOUR (R17, 5K linear) to ground.
    pub amount: f64,
}

impl Default for ContourBench {
    fn default() -> Self {
        ContourBench {
            filter: ContourControls::default(),
            loudness: ContourControls::default(),
            decay_on: true,
            key_down: 0.01,
            key_up: 0.4,
            amount: 5e3,
        }
    }
}

fn sustain_pot(name: &str, node: &str, pos: f64) -> String {
    // R18/R19 5K linear: +10 V to ground, the wiper at `node`.
    let top = (5e3 * (1.0 - pos)).max(1e-3);
    let bottom = (5e3 * pos).max(1e-3);
    format!("r{name}t vp10 {node} {top}\nr{name}b {node} 0 {bottom}\n")
}

/// The netlist of the bench.
pub fn netlist(b: &ContourBench, solver: Solver) -> String {
    let dir = circuits_dir();
    let (f, l) = (b.filter, b.loudness);
    let dump = if b.decay_on {
        // DECAY on: the dump line open.
        "rdumpf fdump 0 1e12\nrdumpl ldump 0 1e12\n".to_string()
    } else {
        // DECAY off: both dump lines through R1401 1.5K to V-trig.
        "r1401 dump vtrig 1.5k\nrdf fdump dump 1m\nrdl ldump dump 1m\n".to_string()
    };
    format!(
        "minimoog board 2 contour generators\n\
         .include {m}\n.include {c}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp10 vp10 0 10\nvn10 vn10 0 -10\nvp15 vp15 0 15\n\
         * the key: its contact joins the +10 V bar to the trigger bus\n\
         vkey key 0 pwl(0 0 {d0} 0 {d1} 1 {u0} 1 {u1} 0)\n\
         skey vp10 trig key 0 swkey\n.model swkey sw(vt=0.5 vh=0.1 ron=1 roff=1e12)\n\
         * the trigger bus's load on board 2's keyboard circuit (R65, R34, C13)\n\
         r65 trig t2 5.1k\nr34 t2 vn10 100k\nc13 t2 vn10 0.01u\n\
         * EXT. S-TRIG open (a short to ground would trigger)\n\
         rstrig strig 0 1e12\n\
         * front panel: ATTACK and DECAY (1M audio, rheostats), SUSTAIN (5K linear)\n\
         rfatt fatt fcap {fa}\nrfdec fdec fcap {fd}\nrlatt latt lcap {la}\nrldec ldec lcap {ld}\n\
         {fs}{ls}\
         * left hand controller: the final decay through J1401's normal contacts; DECAY\n\
         rfin_f ffin vtrig 1m\nrfin_l lfin vtrig 1m\n{dump}\
         * loads: AMOUNT OF CONTOUR on the filter contour; the VCA's R59 on the loudness\n\
         ramount fout 0 {am}\nrvca lout 0 2meg\n\
         x1 trig strig vtrig fatt fcap fdec fdump ffin fsus fout latt lcap ldec ldump lfin lsus lout vp15 vp10 vn10 mm_contour2\n",
        m = dir.join("models/mm-devices.lib").display(),
        c = dir.join("boards/board2-contour.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        d0 = b.key_down,
        d1 = b.key_down + 1e-6,
        u0 = b.key_up,
        u1 = b.key_up + 1e-6,
        fa = f.attack.max(1.0),
        fd = f.decay.max(1.0),
        la = l.attack.max(1.0),
        ld = l.decay.max(1.0),
        fs = sustain_pot("18", "fsus", f.sustain),
        ls = sustain_pot("19", "lsus", l.sustain),
        am = b.amount,
    )
}

/// A transient run of `tstop` s with steps no longer than `tmax`.
pub fn transient(
    spice: &Ngspice,
    work: &Path,
    b: &ContourBench,
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    let net = netlist(b, solver);
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

/// [`transient`] with the key's contact driven by `key`, a source function for the key's
/// control (1 held, 0 released; for example several presses as a pwl()).
pub fn transient_with_key(
    spice: &Ngspice,
    work: &Path,
    b: &ContourBench,
    key: &str,
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    transient_with_inputs(spice, work, b, key, None, tstop, tmax, solver)
}

/// [`transient_with_key`] with EXT. S-TRIG too: `s_trig`, a source for its switch's
/// control (above 0.5 the jack is shorted to ground, through 1 ohm), or None for open.
#[allow(clippy::too_many_arguments)]
pub fn transient_with_inputs(
    spice: &Ngspice,
    work: &Path,
    b: &ContourBench,
    key: &str,
    s_trig: Option<&str>,
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    let mut net = netlist(b, solver);
    let start = net
        .find("vkey key 0 ")
        .ok_or_else(|| Error::Raw("no key source".into()))?;
    let end = start + net[start..].find('\n').unwrap_or(net.len() - start);
    net = format!("{}vkey key 0 {key}{}", &net[..start], &net[end..]);
    if let Some(src) = s_trig {
        let open = "rstrig strig 0 1e12\n";
        if !net.contains(open) {
            return Err(Error::Raw("no S-TRIG in the bench".into()));
        }
        net = net.replace(
            open,
            &format!("vst st 0 {src}\nsstrig strig 0 st 0 swkey\n"),
        );
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
