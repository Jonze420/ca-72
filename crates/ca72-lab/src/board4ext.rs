//! Board 4's A-440 reference oscillator (circuit No. 13, `board4-a440.lib`) and external
//! preamplifier with its overload lamp driver (circuit No. 12, `board4-preamp.lib`) on the
//! bench (docs/circuit/board4.md).

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::path::Path;

/// The A-440's bench: R68's wiper and the selected parts, and a load on its output (R40 and
/// C8 into Q14's collector, an AC ground behind `r_vca`; `r40` infinite for none). The
/// table is measured unloaded (board4.md, B4-7: loaded as drawn, the oscillator cannot be
/// trimmed to 440 Hz).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct A440Bench {
    pub a68: f64,
    pub c17: f64,
    pub c25: f64,
    pub r1: f64,
    pub r40: f64,
    pub c8: f64,
    pub r_vca: f64,
}

impl Default for A440Bench {
    fn default() -> Self {
        A440Bench {
            a68: 0.5,
            c17: 900e-12,
            c25: 900e-12,
            r1: 680e3,
            r40: f64::INFINITY,
            c8: 0.1e-6,
            // R18 330 and half of R12 (100): Q14's collector's resistance to its supply.
            r_vca: 380.0,
        }
    }
}

/// The netlist: SW18 closes at t = 0 (the rail steps to +10 V in 10 us).
pub fn a440_netlist(b: &A440Bench, solver: Solver) -> String {
    let dir = circuits_dir();
    format!(
        "minimoog board 4 A-440\n.include {md}\n.include {lib}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method=gear maxord=2\n\
         vr rail 0 pwl(0 0 10u 10)\n\
         x1 rail out mm_a440 a68={a68} cs17={c17:e} cs25={c25:e} rs1={r1}\n{load}",
        md = dir.join("models/mm-devices.lib").display(),
        lib = dir.join("boards/board4-a440.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        a68 = b.a68,
        c17 = b.c17,
        c25 = b.c25,
        r1 = b.r1,
        load = if b.r40.is_finite() {
            format!(
                "r40 out j {}\nc8 j q14 {:e}\nrvca q14 0 {}\n",
                b.r40, b.c8, b.r_vca
            )
        } else {
            String::new()
        },
    )
}

/// The oscillator's output from switch-on to `t_stop` s, resampled at `rate`.
pub fn a440_run(
    spice: &Ngspice,
    work: &Path,
    b: &A440Bench,
    t_stop: f64,
    rate: f64,
    solver: Solver,
) -> Result<Vec<f64>, Error> {
    let net = a440_netlist(b, solver);
    let step = 1.0 / rate;
    let p: Plot = spice
        .run(
            &net,
            &[&format!("tran {step:e} {t_stop:e} 0 {:e}", step / 2.0)],
            work,
        )?
        .pop()
        .ok_or_else(|| Error::Raw("no tran plot".into()))?;
    let n = (t_stop * rate) as usize;
    Ok(ca72_spice::resample(
        p.vec("time"),
        p.vec("v(out)"),
        0.0,
        rate,
        n,
    ))
}

/// The frequency of `x` (sampled at `rate`) over `[from, end)` s, from its rising crossings
/// of its mean, interpolated.
pub fn frequency(x: &[f64], rate: f64, from: f64) -> f64 {
    let s = &x[(from * rate) as usize..];
    let m = s.iter().sum::<f64>() / s.len() as f64;
    let ups: Vec<f64> = s
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < m && w[1] >= m)
        .map(|(i, w)| i as f64 + (m - w[0]) / (w[1] - w[0]))
        .collect();
    rate * (ups.len() - 1) as f64 / (ups[ups.len() - 1] - ups[0])
}

/// The A-440's waveform as the table carries it: after the factory's trim (5.7: R68 for 440
/// Hz), its steady state (DC, and each harmonic's amplitude and phase at a rising crossing
/// of the fundamental), and its start from switch-on (DC and the fundamental's amplitude,
/// relative to the steady state's, every millisecond).
#[derive(Debug, Clone, PartialEq)]
pub struct A440Table {
    pub a68: f64,
    pub hz: f64,
    pub dc: f64,
    pub harmonics: Vec<(f64, f64)>,
    pub start_dc: Vec<f64>,
    pub start_amp: Vec<f64>,
}

/// Seconds simulated: the oscillation builds up within about 200 ms.
pub const A440_SETTLE: f64 = 0.45;
/// The start's table: 1 ms steps for its first 300 ms.
pub const A440_START_STEP: f64 = 1e-3;
pub const A440_START_POINTS: usize = 300;
/// Harmonics kept.
pub const A440_HARMONICS: usize = 24;
const RATE: f64 = 1e6;

/// The DFT of `x` (sampled at `rate`) at `f` Hz over `[a, b)` samples: (amplitude, phase).
fn component(x: &[f64], rate: f64, f: f64, a: usize, b: usize) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &v) in x[a..b].iter().enumerate() {
        let w = 2.0 * std::f64::consts::PI * f * i as f64 / rate;
        re += v * w.cos();
        im -= v * w.sin();
    }
    let n = (b - a) as f64;
    (2.0 * (re * re + im * im).sqrt() / n, im.atan2(re))
}

/// Trims R68 toward 440 Hz (5.7; secant steps on the frequency, within R68's travel) and
/// measures the table. As modelled R68 cannot reach 440 Hz (B4-7): the trim stops at its
/// end nearest, and the table records the frequency reached.
pub fn a440_table(
    spice: &Ngspice,
    work: &Path,
    base: &A440Bench,
    solver: Solver,
) -> Result<A440Table, Error> {
    let mut b = *base;
    let freq = |a68: f64| -> Result<(f64, Vec<f64>), Error> {
        let x = a440_run(
            spice,
            work,
            &A440Bench { a68, ..b },
            A440_SETTLE,
            RATE,
            solver,
        )?;
        Ok((frequency(&x, RATE, 0.3), x))
    };
    let (mut a0, mut a1) = (0.0, 0.3);
    let (mut f0, mut x0) = freq(a0)?;
    let (mut f1, mut x1) = freq(a1)?;
    // The end of R68's travel nearest 440 Hz, if 440 Hz lies beyond it.
    let (lo_f, hi_f) = if f0 > f1 { (f1, f0) } else { (f0, f1) };
    if !(lo_f..=hi_f).contains(&440.0) {
        let (f_end, _) = freq(1.0)?;
        if !(f0.min(f_end)..=f0.max(f_end)).contains(&440.0) {
            let (a, f, x) = if (f0 - 440.0).abs() < (f_end - 440.0).abs() {
                (0.0, f0, x0)
            } else {
                (1.0, f_end, freq(1.0)?.1)
            };
            (a1, f1, x1) = (a, f, x);
            a0 = a1;
            f0 = f1;
            x0 = Vec::new();
        }
    }
    for _ in 0..12 {
        if (f1 - 440.0).abs() < 0.002 || a0 == a1 {
            break;
        }
        let a2 = (a1 - (f1 - 440.0) * (a1 - a0) / (f1 - f0)).clamp(0.0, 1.0);
        let (f2, x2) = freq(a2)?;
        (a0, f0, a1, f1, x1) = (a1, f1, a2, f2, x2);
    }
    drop(x0);
    b.a68 = a1;
    let x = x1;
    // The steady state: whole periods from 0.3 s, starting at a rising crossing of the mean.
    let period = RATE / f1;
    let from = (0.3 * RATE) as usize;
    let tail = &x[from..];
    let m = tail.iter().sum::<f64>() / tail.len() as f64;
    let start = from
        + tail
            .windows(2)
            .position(|w| w[0] < m && w[1] >= m)
            .unwrap_or(0)
        + 1;
    let periods = (((x.len() - start) as f64) / period).floor() - 1.0;
    let end = start + (periods * period) as usize;
    let dc = x[start..end].iter().sum::<f64>() / (end - start) as f64;
    let harmonics = (1..=A440_HARMONICS)
        .map(|k| component(&x, RATE, k as f64 * f1, start, end))
        .collect::<Vec<_>>();
    let a_fund = harmonics[0].0;
    // The start: one period's window centred on each millisecond.
    let w = period as usize;
    let (mut start_dc, mut start_amp) = (Vec::new(), Vec::new());
    for i in 0..A440_START_POINTS {
        let c = (i as f64 * A440_START_STEP * RATE) as usize;
        let a = c.saturating_sub(w / 2);
        let e = (a + w).min(x.len());
        let mean = x[a..e].iter().sum::<f64>() / (e - a) as f64;
        start_dc.push(mean);
        start_amp.push(component(&x, RATE, f1, a, e).0 / a_fund);
    }
    Ok(A440Table {
        a68: b.a68,
        hz: f1,
        dc,
        harmonics,
        start_dc,
        start_amp,
    })
}

/// The table as Rust source (`crates/ca72/src/a440_table.rs`).
pub fn a440_source(t: &A440Table, version: &str) -> String {
    let list = |v: &[f64]| {
        v.chunks(6)
            .map(|c| {
                format!(
                    "    {},",
                    c.iter()
                        .map(|x| format!("{x:.9e}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let amps: Vec<f64> = t.harmonics.iter().map(|h| h.0).collect();
    let phases: Vec<f64> = t.harmonics.iter().map(|h| h.1).collect();
    format!(
        "//! The A-440's waveform derived from its circuit (board4-a440.lib, unloaded; board4.md B4-7)\n\
         //! by `ca72-lab tables` (do not edit), with {version}. The lab test `tables_match_the_circuit`\n\
         //! regenerates it and compares.\n\n\
         /// R68's wiper after the factory's trim (5.7), and the frequency the circuit gives there,\n\
         /// Hz (it falls short of 440 Hz: board4.md B4-7).\n\
         pub const A68: f64 = {a68:.9e};\n\
         pub const HZ: f64 = {hz:.9e};\n\
         /// The steady state: the output's mean, V, and each harmonic's amplitude (V) and phase\n\
         /// (rad) from a rising crossing of the mean.\n\
         pub const DC: f64 = {dc:.9e};\n\
         #[rustfmt::skip]\n\
         pub const AMP: [f64; {k}] = [\n{amps}\n];\n\
         #[rustfmt::skip]\n\
         pub const PHASE: [f64; {k}] = [\n{phases}\n];\n\
         /// From switch-on, every {step} s: the output's mean (V) and the fundamental's amplitude\n\
         /// relative to the steady state's.\n\
         pub const START_STEP: f64 = {step:e};\n\
         #[rustfmt::skip]\n\
         pub const START_DC: [f64; {n}] = [\n{sdc}\n];\n\
         #[rustfmt::skip]\n\
         pub const START_AMP: [f64; {n}] = [\n{samp}\n];\n",
        a68 = t.a68,
        hz = t.hz,
        dc = t.dc,
        k = t.harmonics.len(),
        amps = list(&amps),
        phases = list(&phases),
        step = A440_START_STEP,
        n = t.start_dc.len(),
        sdc = list(&t.start_dc),
        samp = list(&t.start_amp),
    )
}

/// The preamplifier's bench: a source (`src` volts behind `r_src`) into pin 11, SW10 on
/// (pin 17 through R46 33K to the bus, an AC ground), the lamp B1 (`r_lamp`) from +15 V.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreampBench {
    pub r_src: f64,
    pub r_lamp: f64,
}

impl Default for PreampBench {
    fn default() -> Self {
        PreampBench {
            r_src: 100e3,
            r_lamp: 200.0,
        }
    }
}

/// The netlist with the source's line given (`vin src 0 ...`).
pub fn preamp_netlist(b: &PreampBench, source: &str, solver: Solver) -> String {
    let dir = circuits_dir();
    format!(
        "minimoog board 4 external preamplifier\n.include {md}\n.include {lib}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp10 p10 0 10\nvn10 n10 0 -10\nvp15 p15 0 15\n{source}\n\
         rsrc src in {rs}\n\
         x1 in out amp lamp 0 p10 n10 p15 mm_preamp\n\
         rlamp p15 lamp {rl}\nr46 out 0 33k\n",
        md = dir.join("models/mm-devices.lib").display(),
        lib = dir.join("boards/board4-preamp.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        rs = b.r_src,
        rl = b.r_lamp,
    )
}
