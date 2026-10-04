//! The real-time filter against ngspice (docs/circuit/board4.md, numerics.md): the
//! small-signal response over cutoff and emphasis, the resonance, self-oscillation and
//! the distortion of a driven filter. Both are driven with the same ladder current (Q28's
//! collector from ngspice), so the ladder, its coupling networks, the output pair and the
//! emphasis loop are what is compared; the exponential converter is checked separately.
//!
//! The error budgets are the documented differences (docs/circuit/numerics.md,
//! assumptions.md A11): the trapezoidal rule's frequency warping at 4x, which grows
//! towards 20 kHz, and the output stage's transistor capacitances, which the real-time
//! model leaves out (up to 4 cents and 2.5 dB on a +25 dB peak at 11 kHz;
//! `capacitance_budget` measures them).

use ca72::vcf::Vcf;
use ca72_lab::bench::Solver;
use ca72_lab::vcf::{self as lab, VcfBench};
use ca72_lab::work_dir;
use ca72_spice::{Ngspice, for_test};
use std::f64::consts::PI;

const SR: f64 = 48_000.0;
/// One mixer channel's 33K and the test signal's size (small signal).
const G_MIX: f64 = 1.0 / 33e3;
const A_SMALL: f64 = 0.02;

/// The real-time filter's complex response to a small impulse on one mixer channel (the
/// FFT of `n` samples, the resamplers' latency removed) at the given frequencies.
fn rt_complex(i0: f64, r14: f64, freqs: &[f64], os: usize, n: usize) -> Vec<(f64, f64)> {
    let mut f = Vcf::new(SR, os);
    f.circuit.r14 = r14;
    let mut re: Vec<f64> = (0..n)
        .map(|i| f.tick(if i == 0 { A_SMALL * G_MIX } else { 0.0 }, G_MIX, i0) / A_SMALL)
        .collect();
    let lat = f.latency();
    let mut im = vec![0.0; n];
    ca72_analysis::fft::fft(&mut re, &mut im);
    freqs
        .iter()
        .map(|&fr| {
            let k = fr / SR * n as f64;
            let (i, t) = (k.floor() as usize, k.fract());
            let (r, m) = (
                (1.0 - t) * re[i] + t * re[i + 1],
                (1.0 - t) * im[i] + t * im[i + 1],
            );
            let ph = 2.0 * PI * fr * lat / SR;
            (r * ph.cos() - m * ph.sin(), r * ph.sin() + m * ph.cos())
        })
        .collect()
}

fn db(c: (f64, f64)) -> f64 {
    10.0 * (c.0 * c.0 + c.1 * c.1).log10()
}

fn setup(spice: &Ngspice, b: &VcfBench, tag: &str) -> f64 {
    lab::ladder_current(spice, &work_dir(tag), b, Solver::default()).expect("op")
}

#[test]
fn small_signal_response_matches_the_circuit() {
    let Some(spice) = for_test("small_signal_response_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    for cutoff in [-6.0, -3.0, 0.0, 3.0, 6.0, 8.0] {
        for r14 in [50e3, 10e3, 3e3, 1.5e3] {
            let b = VcfBench {
                cutoff,
                r14,
                ..VcfBench::default()
            };
            let tag = format!("vcf-ac-{cutoff}-{r14}");
            let i0 = setup(&spice, &b, &tag);
            let ac = lab::ac_response(
                &spice,
                &work_dir(&tag),
                &b,
                20.0,
                20_000.0,
                20,
                Solver::default(),
            )
            .expect("ac");
            let freqs: Vec<f64> = ac.iter().map(|p| p.0).collect();
            let peak = ac.iter().map(|p| p.1).fold(f64::MIN, f64::max);
            // At 4x (the default) everywhere; at 8x where the cutoff is high, to show the
            // error above 10 kHz is the integrator's.
            let rates: &[usize] = if cutoff >= 6.0 { &[4, 8] } else { &[4] };
            for &os in rates {
                let rt = rt_complex(i0, r14, &freqs, os, 1 << 18);
                let (mut lo, mut hi) = ((0.0, 0.0f64), (0.0, 0.0f64));
                for ((f, ng), r) in ac.iter().zip(&rt) {
                    if *ng < peak - 40.0 {
                        continue;
                    }
                    let e = db(*r) - ng;
                    let w = if *f <= 10_000.0 { &mut lo } else { &mut hi };
                    if e.abs() > w.1.abs() {
                        *w = (*f, e);
                    }
                }
                let (lo_budget, hi_budget) = if os == 4 { (0.3, 1.0) } else { (0.2, 0.35) };
                fail |= lo.1.abs() > lo_budget || hi.1.abs() > hi_budget;
                report.push_str(&format!(
                    "cutoff {cutoff:+} V, R14 {r14:>6}, {os}x: I0 {:7.2} uA, peak {peak:+5.1} dB; worst {:+.2} dB at {:.0} Hz (<= 10 kHz), {:+.2} dB at {:.0} Hz (above)\n",
                    i0 * 1e6, lo.1, lo.0, hi.1, hi.0
                ));
            }
        }
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: to 10 kHz and above, 0.3 and 1.0 dB at 4x, 0.2 and 0.35 dB at 8x\n{report}"
    );
}

/// The resonant peak's frequency and height by a parabola through the highest point and
/// its neighbours (dB against log frequency).
fn peak(curve: &[(f64, f64)]) -> (f64, f64) {
    let k = (1..curve.len() - 1)
        .max_by(|&a, &b| curve[a].1.total_cmp(&curve[b].1))
        .expect("a curve of at least three points");
    let (x0, x1, x2) = (curve[k - 1].0.ln(), curve[k].0.ln(), curve[k + 1].0.ln());
    let (y0, y1, y2) = (curve[k - 1].1, curve[k].1, curve[k + 1].1);
    let d1 = (y1 - y0) / (x1 - x0);
    let d2 = (y2 - y1) / (x2 - x1);
    let a = (d2 - d1) / (x2 - x0);
    let b = d1 - a * (x0 + x1);
    let x = -b / (2.0 * a);
    (x.exp(), y1 + (x - x1) * (b + a * (x + x1)))
}

/// A fine AC sweep around the resonance: (Hz, dB).
fn ngspice_peak_curve(
    spice: &Ngspice,
    b: &VcfBench,
    tag: &str,
    net: Option<String>,
) -> Vec<(f64, f64)> {
    let solver = Solver::default();
    let coarse =
        lab::ac_response(spice, &work_dir(tag), b, 20.0, 20_000.0, 50, solver).expect("ac");
    let (f_c, _) = peak(&coarse);
    let net = net.unwrap_or_else(|| lab::netlist(b, "vsrc src 0 dc 0 ac 1", solver));
    let plots = spice
        .run(
            &net,
            &[&format!("ac dec 4000 {} {}", f_c / 1.2, f_c * 1.2)],
            &work_dir(tag),
        )
        .expect("ac");
    let out = plots[0].complex_vec("out").expect("v(out)");
    plots[0]
        .vec("frequency")
        .iter()
        .zip(out)
        .map(|(&f, &c)| (f, db(c)))
        .collect()
}

#[test]
fn resonance_matches_the_circuit() {
    let Some(spice) = for_test("resonance_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    for cutoff in [-3.0, 0.0, 3.0, 6.0] {
        for r14 in [1.5e3, 700.0] {
            let b = VcfBench {
                cutoff,
                r14,
                ..VcfBench::default()
            };
            let tag = format!("vcf-res-{cutoff}-{r14}");
            let i0 = setup(&spice, &b, &tag);
            let ng = ngspice_peak_curve(&spice, &b, &tag, None);
            let freqs: Vec<f64> = ng.iter().map(|p| p.0).collect();
            let rt: Vec<(f64, f64)> = freqs
                .iter()
                .zip(rt_complex(i0, r14, &freqs, 4, 1 << 20))
                .map(|(&f, c)| (f, db(c)))
                .collect();
            let (fn_, hn) = peak(&ng);
            let (fr, hr) = peak(&rt);
            let cents = 1200.0 * (fr / fn_).log2();
            // Peaks above +20 dB are where the omitted output capacitances show most.
            let h_budget = if hn > 20.0 { 3.0 } else { 0.6 };
            fail |= cents.abs() > 8.0 || (hr - hn).abs() > h_budget;
            report.push_str(&format!(
                "cutoff {cutoff:+} V, R14 {r14:>5}: ngspice {fn_:8.1} Hz {hn:+6.2} dB; RT {cents:+5.2} cents {:+5.2} dB\n",
                hr - hn
            ));
        }
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: 8 cents; 0.6 dB (3 dB for peaks above +20 dB)\n{report}"
    );
}

/// Fundamental (from rising zero crossings of the signal less its mean) and the first
/// harmonics' amplitudes (projections over whole periods) of a uniformly sampled signal.
fn harmonics(y: &[f64], sr: f64, count: usize) -> (f64, Vec<f64>) {
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let t: Vec<f64> = (0..y.len()).map(|i| i as f64 / sr).collect();
    let z: Vec<f64> = y.iter().map(|v| v - mean).collect();
    let c = ca72_spice::crossings(&t, &z, 0.0, true);
    let f0 = (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]);
    let (i0, i1) = (
        (c[0] * sr).round() as usize,
        (c[c.len() - 1] * sr).round() as usize,
    );
    let n = (i1 - i0) as f64;
    let amps = (1..=count)
        .map(|k| {
            let (mut a, mut b) = (0.0, 0.0);
            for (i, v) in z[i0..i1].iter().enumerate() {
                let ph = 2.0 * PI * k as f64 * f0 * i as f64 / sr;
                a += v * ph.cos();
                b += v * ph.sin();
            }
            2.0 * (a * a + b * b).sqrt() / n
        })
        .collect();
    (f0, amps)
}

/// ngspice's and the real-time filter's output over the last `window` s of `tstop`, with
/// the mixer channel's source a function of time and ngspice's step limit a function of
/// the ladder current.
fn transient_pair(
    spice: &Ngspice,
    b: &VcfBench,
    tag: &str,
    (src_line, src): (&str, impl Fn(f64) -> f64),
    (tstop, window): (f64, f64),
    tmax: impl Fn(f64) -> f64,
) -> (Vec<f64>, Vec<f64>) {
    let i0 = setup(spice, b, tag);
    let p = lab::transient(
        spice,
        &work_dir(tag),
        b,
        src_line,
        tstop,
        tmax(i0),
        Solver::default(),
    )
    .expect("tran");
    let ng = ca72_spice::resample(
        p.vec("time"),
        p.vec("out"),
        tstop - window,
        192_000.0,
        (window * 192_000.0) as usize,
    );
    let mut f = Vcf::new(SR, 4);
    f.circuit.r14 = b.r14;
    let n = (tstop * SR) as usize;
    let out: Vec<f64> = (0..n)
        .map(|i| f.tick(src(i as f64 / SR) * G_MIX, G_MIX, i0))
        .collect();
    (ng, out[n - (window * SR) as usize..].to_vec())
}

/// The ladder's corner for a tail current, Hz (I0 / 4 C Vt).
fn corner_hz(i0: f64) -> f64 {
    i0 / (4.0 * 0.068e-6 * ca72::devices::vt(25.0)) / (2.0 * PI)
}

/// ngspice's time steps per period of the oscillation: its frequency has converged to
/// 0.1 cent there (`oscillation_reference_converges`).
const POINTS: f64 = 2000.0;

#[test]
fn self_oscillation_matches_the_circuit() {
    let Some(spice) = for_test("self_oscillation_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    for cutoff in [-3.0, 0.0, 3.0] {
        // EMPHASIS at 10; a short pulse on the mixer starts the oscillation.
        let b = VcfBench {
            cutoff,
            r14: 1.0,
            ..VcfBench::default()
        };
        let (ng, rt) = transient_pair(
            &spice,
            &b,
            &format!("vcf-osc-{cutoff}"),
            ("vsrc src 0 pulse(0 0.2 1m 1u 1u 100u 1)", |t| {
                if (1e-3..1.1e-3).contains(&t) {
                    0.2
                } else {
                    0.0
                }
            }),
            (0.3, 0.1),
            |i0| 1.0 / (corner_hz(i0) * POINTS),
        );
        let (fn_, hn) = harmonics(&ng, 192_000.0, 3);
        let (fr, hr) = harmonics(&rt, SR, 3);
        let cents = 1200.0 * (fr / fn_).log2();
        let d: Vec<f64> = hn
            .iter()
            .zip(&hr)
            .map(|(a, b)| 20.0 * (b / a).log10())
            .collect();
        fail |= cents.abs() > 5.0 || d[0].abs() > 0.3 || d[1].abs() > 0.5 || d[2].abs() > 0.5;
        report.push_str(&format!(
            "cutoff {cutoff:+} V: ngspice {fn_:7.2} Hz, {:.3} V, H2 {:.1} dB, H3 {:.1} dB; RT {cents:+.2} cents, H1..H3 {:+.2} {:+.2} {:+.2} dB\n",
            hn[0],
            20.0 * (hn[1] / hn[0]).log10(),
            20.0 * (hn[2] / hn[0]).log10(),
            d[0], d[1], d[2]
        ));
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: 5 cents; H1 0.3 dB, H2 and H3 0.5 dB\n{report}"
    );
}

#[test]
fn drive_distortion_matches_the_circuit() {
    let Some(spice) = for_test("drive_distortion_matches_the_circuit") else {
        return;
    };
    let hz = 200.0;
    let mut report = String::new();
    let mut fail = false;
    for (cutoff, r14) in [(6.0, 50e3), (0.0, 1.5e3)] {
        for amp in [0.5, 2.0, 5.0, 10.0, 20.0] {
            let b = VcfBench {
                cutoff,
                r14,
                ..VcfBench::default()
            };
            let (ng, rt) = transient_pair(
                &spice,
                &b,
                &format!("vcf-drive-{cutoff}-{r14}-{amp}"),
                (&format!("vsrc src 0 sin(0 {amp} {hz})"), |t| {
                    amp * (2.0 * PI * hz * t).sin()
                }),
                (0.4, 0.1),
                |_| 1.0 / (hz * 2000.0),
            );
            let (_, hn) = harmonics(&ng, 192_000.0, 7);
            let (_, hr) = harmonics(&rt, SR, 7);
            let mut line = format!("cutoff {cutoff:+} V, R14 {r14:>6}, {amp:>4} V:");
            for (k, (a, b)) in hn.iter().zip(&hr).enumerate() {
                let level = 20.0 * (a / hn[0]).log10();
                let d = 20.0 * (b / a).log10();
                // Harmonics above -100 dB against the fundamental.
                if level > -100.0 {
                    fail |= d.abs() > 0.5;
                    line.push_str(&format!(" H{} {level:.1}/{d:+.2}", k + 1));
                }
            }
            report.push_str(&line);
            report.push('\n');
        }
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: 0.5 dB for harmonics above -100 dB (level/RT minus ngspice)\n{report}"
    );
}

/// The self-oscillation reference's frequency against ngspice's step: doubling the steps
/// per period from `POINTS` moves it by less than 0.1 cent.
#[test]
fn oscillation_reference_converges() {
    let Some(spice) = for_test("oscillation_reference_converges") else {
        return;
    };
    let b = VcfBench {
        cutoff: 0.0,
        r14: 1.0,
        ..VcfBench::default()
    };
    let i0 = setup(&spice, &b, "vcf-osc-conv");
    let mut f = Vec::new();
    for points in [POINTS / 4.0, POINTS, POINTS * 2.0] {
        let p = lab::transient(
            &spice,
            &work_dir(&format!("vcf-osc-conv-{points}")),
            &b,
            "vsrc src 0 pulse(0 0.2 1m 1u 1u 100u 1)",
            0.2,
            1.0 / (corner_hz(i0) * points),
            Solver::default(),
        )
        .expect("tran");
        let y = ca72_spice::resample(p.vec("time"), p.vec("out"), 0.1, 192_000.0, 19_200);
        f.push(harmonics(&y, 192_000.0, 1).0);
    }
    let c = |a: f64, b: f64| 1200.0 * (a / b).log2();
    eprintln!(
        "{} points: {:+.2} cents, {} points: reference, {} points: {:+.3} cents",
        POINTS / 4.0,
        c(f[0], f[1]),
        POINTS,
        POINTS * 2.0,
        c(f[2], f[1])
    );
    assert!(c(f[2], f[1]).abs() < 0.1);
}

/// Diagnostic for the documented budget: the resonance against ngspice with the transistor
/// capacitances removed from the output stage (Q8, Q6, Q7, Q5), the ladder, or both.
/// `cargo test -p ca72-lab --test vcf_realtime capacitance_budget -- --ignored --nocapture`
#[test]
#[ignore]
fn capacitance_budget() {
    let spice = Ngspice::find().expect("ngspice");
    let dev = ca72_lab::circuits_dir().join("models/mm-devices.lib");
    let lib = ca72_lab::circuits_dir().join("boards/board4-vcf.lib");
    let dev_text = std::fs::read_to_string(&dev).unwrap();
    let model = ".model QNOCAP NPN (TNOM=25 IS=5e-15 NF=1 BF=420 ISE=1e-14 NE=1.5 VAF=100 IKF=0.1 BR=4 RB=50 RE=0.5 RC=1 EG=1.11 XTI=3 XTB=1.5)";
    let out_q = ["q8", "q6", "q7", "q5"];
    let lad_q = [
        "q23", "q24", "q19", "q20", "q10", "q11", "q2", "q3", "q29", "q30",
    ];
    let variant = |name: &str, qs: &[&str]| {
        let text: String = std::fs::read_to_string(&lib)
            .unwrap()
            .lines()
            .map(|l| {
                let q = l.split_whitespace().next().unwrap_or("");
                let l = if qs.contains(&q) {
                    l.replacen("QTIS97", "QNOCAP", 1)
                } else {
                    l.to_string()
                };
                l + "\n"
            })
            .collect();
        let dir = work_dir(&format!("vcf-nocap-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("devices.lib"), format!("{dev_text}\n{model}\n")).unwrap();
        std::fs::write(dir.join("vcf.lib"), text).unwrap();
        dir
    };
    let all: Vec<&str> = out_q.iter().chain(lad_q.iter()).copied().collect();
    let dirs = [
        variant("out", &out_q),
        variant("ladder", &lad_q),
        variant("all", &all),
    ];
    for cutoff in [0.0, 3.0, 6.0] {
        for r14 in [1.5e3, 700.0] {
            let b = VcfBench {
                cutoff,
                r14,
                ..VcfBench::default()
            };
            let tag = format!("vcf-capbudget-{cutoff}-{r14}");
            let i0 = setup(&spice, &b, &tag);
            let drawn = ngspice_peak_curve(&spice, &b, &tag, None);
            let freqs: Vec<f64> = drawn.iter().map(|p| p.0).collect();
            let rt: Vec<(f64, f64)> = freqs
                .iter()
                .zip(rt_complex(i0, r14, &freqs, 8, 1 << 20))
                .map(|(&f, c)| (f, db(c)))
                .collect();
            let (fr, hr) = peak(&rt);
            let (fd, hd) = peak(&drawn);
            let mut line = format!(
                "cutoff {cutoff:+} V, R14 {r14:>5}: ngspice {fd:8.1} Hz {hd:+6.2} dB; RT (8x) minus: as drawn {:+5.2} c {:+5.2} dB",
                1200.0 * (fr / fd).log2(),
                hr - hd
            );
            let net = lab::netlist(&b, "vsrc src 0 dc 0 ac 1", Solver::default());
            for (name, dir) in ["output caps 0", "ladder caps 0", "all caps 0"]
                .iter()
                .zip(&dirs)
            {
                let v = ngspice_peak_curve(
                    &spice,
                    &b,
                    &tag,
                    Some(
                        net.replace(
                            &dev.display().to_string(),
                            &dir.join("devices.lib").display().to_string(),
                        )
                        .replace(
                            &lib.display().to_string(),
                            &dir.join("vcf.lib").display().to_string(),
                        ),
                    ),
                );
                let (fv, hv) = peak(&v);
                line.push_str(&format!(
                    "; {name} {:+5.2} c {:+5.2} dB",
                    1200.0 * (fr / fv).log2(),
                    hr - hv
                ));
            }
            eprintln!("{line}");
        }
    }
}
