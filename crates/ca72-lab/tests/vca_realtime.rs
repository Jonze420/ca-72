//! The real-time VCAs and output stage against ngspice (docs/circuit/board4.md): the
//! tails and the resting output over the loudness contour, the factory balance procedure,
//! the gain over contour and frequency, the static transfer, the thump of a contour step
//! and the distortion of a driven VCA; the EXT. LOUDNESS jack (J3) plugged, steady and at
//! audio rate.
//!
//! The known difference: the output pair loads the second pair's collectors about 5 %
//! less in the model (its linearised base current), which leaves the gain 0.1 dB high.

use ca72::vca::{CONTOUR_FULL, Vca, VcaCircuit};
use ca72_lab::bench::Solver;
use ca72_lab::vca::{self as lab, VcaBench};
use ca72_lab::work_dir;
use ca72_spice::{Ngspice, Plot, for_test};
use std::f64::consts::PI;

fn circuit(b: &VcaBench) -> VcaCircuit {
    // A source plugged into J3 holds the input itself (the normal contact open).
    let (ext, r_j3) = match b.j3 {
        Some(v) => (v, 0.0),
        None => (b.ext, VcaCircuit::default().r_j3),
    };
    VcaCircuit {
        r2: b.r2,
        r8: b.r8,
        r40: b.r40,
        r14_pos: b.r14,
        r12_pos: b.r12,
        load: b.load,
        ext,
        r_j3,
        ..VcaCircuit::default()
    }
}

/// The calibrated real-time circuit and the bench with the same trims.
fn calibrated() -> (VcaCircuit, VcaBench) {
    let c = VcaCircuit::default().calibrated();
    (
        c,
        VcaBench {
            r12: c.r12_pos,
            r14: c.r14_pos,
            ..VcaBench::default()
        },
    )
}

/// The real-time VCA's gain at `hz` from its input to the main output, dB.
fn rt_gain(b: &VcaBench, hz: f64) -> f64 {
    rt_gain_every(b, hz, None)
}

/// [`rt_gain`] with the bias solved every `every` samples (None: the VCA's own interval).
fn rt_gain_every(b: &VcaBench, hz: f64, every: Option<usize>) -> f64 {
    let sr = 192_000.0;
    let mut v = Vca::new(circuit(b), sr, b.cont);
    if let Some(e) = every {
        v.control_every = e;
    }
    let a = 1e-3;
    // Past C2's and C6's transients (110 ms, 53 ms), then whole periods.
    let settle = 0.8;
    let periods = (0.1 * hz).ceil().max(4.0);
    let m = (periods / hz * sr).round() as usize;
    let n = (settle * sr) as usize + m;
    let out: Vec<f64> = (0..n)
        .map(|i| v.tick(a * (2.0 * PI * hz * i as f64 / sr).sin(), b.cont))
        .collect();
    let (mut p, mut q) = (0.0, 0.0);
    for (k, y) in out[n - m..].iter().enumerate() {
        let ph = 2.0 * PI * hz * (n - m + k) as f64 / sr;
        p += y * ph.sin();
        q += y * ph.cos();
    }
    20.0 * ((2.0 * (p * p + q * q).sqrt() / m as f64) / a).log10()
}

#[test]
fn bias_matches_the_circuit() {
    let Some(spice) = for_test("bias_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    let (cal, cal_b) = calibrated();
    for (name, c, bench) in [
        ("as drawn", VcaCircuit::default(), VcaBench::default()),
        ("calibrated", cal, cal_b),
    ] {
        for cont in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0] {
            let b = VcaBench { cont, ..bench };
            let t = lab::tails(
                &spice,
                &work_dir(&format!("vca-bias-{name}-{cont}")),
                &b,
                Solver::default(),
            )
            .expect("op");
            let mut v = Vca::new(c, 48e3, cont);
            let o = v.rest_output(cont);
            let r = v.bias();
            // Relative, with a floor of 1 uA (the first VCA's tail is nanoamps when off).
            let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-6);
            let worst = rel(r.i_a, t[0]).max(rel(r.i_b, t[1])).max(rel(r.i_c, t[2]));
            fail |= worst > 2e-3 || (o - t[3]).abs() > 0.01;
            report.push_str(&format!(
                "{name} contour {cont:+}: Q18 {:.1} uA, Q21 {:.1} uA, Q1 {:.3} mA (worst {:.3} %); output at rest {:.4} V, RT {:+.1} mV\n",
                t[0] * 1e6,
                t[1] * 1e6,
                t[2] * 1e3,
                worst * 100.0,
                t[3],
                (o - t[3]) * 1e3
            ));
        }
    }
    eprintln!("{report}");
    assert!(!fail, "budget: tails 0.2 %, output at rest 10 mV\n{report}");
}

/// The feedthrough the balance procedures null: the output's change for 1 V less behind J3.
fn ngspice_leak(spice: &Ngspice, b: &VcaBench) -> f64 {
    let s = Solver::default();
    let a = lab::tails(spice, &work_dir("vca-cal-a"), b, s).expect("op")[3];
    let c = lab::tails(
        spice,
        &work_dir("vca-cal-b"),
        &VcaBench {
            ext: b.ext - 1.0,
            ..*b
        },
        s,
    )
    .expect("op")[3];
    a - c
}

#[test]
fn calibration_matches_the_circuit() {
    let Some(spice) = for_test("calibration_matches_the_circuit") else {
        return;
    };
    // The factory procedure in ngspice: R12 with the first VCA off, then R14 at full sustain.
    let null = |b: VcaBench, set: fn(&mut VcaBench, f64)| {
        let (mut lo, mut hi) = (0.0, 1.0);
        let mut b = b;
        set(&mut b, lo);
        let f_lo = ngspice_leak(&spice, &b);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            set(&mut b, mid);
            if (ngspice_leak(&spice, &b) > 0.0) == (f_lo > 0.0) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        set(&mut b, 0.5 * (lo + hi));
        b
    };
    let b = null(
        VcaBench {
            cont: 0.0,
            ..VcaBench::default()
        },
        |b, x| b.r12 = x,
    );
    let b = null(
        VcaBench {
            cont: CONTOUR_FULL,
            ..b
        },
        |b, x| b.r14 = x,
    );
    let c = VcaCircuit::default().calibrated();
    let report = format!(
        "2nd VCA BAL R12: ngspice {:.4}, RT {:.4}; 1st VCA BAL R14: ngspice {:.4}, RT {:.4}",
        b.r12, c.r12_pos, b.r14, c.r14_pos
    );
    eprintln!("{report}");
    assert!(
        (b.r12 - c.r12_pos).abs() < 0.005 && (b.r14 - c.r14_pos).abs() < 0.005,
        "{report}"
    );
}

#[test]
fn gain_matches_the_circuit() {
    let Some(spice) = for_test("gain_matches_the_circuit") else {
        return;
    };
    let (_, bench) = calibrated();
    let mut report = String::new();
    let mut fail = false;
    for cont in [0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0] {
        let b = VcaBench { cont, ..bench };
        let ac = lab::ac_response(
            &spice,
            &work_dir(&format!("vca-gain-{cont}")),
            &b,
            1000.0,
            1000.0,
            1,
            Solver::default(),
        )
        .expect("ac");
        let d = rt_gain(&b, 1000.0) - ac[0].1;
        fail |= d.abs() > 0.15;
        report.push_str(&format!(
            "contour {cont:+}: ngspice {:+.2} dB at 1 kHz, RT {d:+.3} dB\n",
            ac[0].1
        ));
    }
    let b = VcaBench {
        cont: CONTOUR_FULL,
        ..bench
    };
    let ac = lab::ac_response(
        &spice,
        &work_dir("vca-gain-f"),
        &b,
        10.0,
        20_000.0,
        3,
        Solver::default(),
    )
    .expect("ac");
    for (f, g) in ac {
        let d = rt_gain(&b, f) - g;
        fail |= d.abs() > 0.15;
        report.push_str(&format!(
            "contour {CONTOUR_FULL}: {f:7.0} Hz ngspice {g:+.2} dB, RT {d:+.3} dB\n"
        ));
    }
    eprintln!("{report}");
    assert!(!fail, "budget: 0.15 dB\n{report}");
}

#[test]
fn static_transfer_matches_the_circuit() {
    let Some(spice) = for_test("static_transfer_matches_the_circuit") else {
        return;
    };
    let (c, bench) = calibrated();
    let mut report = String::new();
    let mut fail = false;
    for cont in [1.0, CONTOUR_FULL] {
        let b = VcaBench { cont, ..bench };
        let st = lab::static_transfer(
            &spice,
            &work_dir(&format!("vca-static-{cont}")),
            &b,
            -0.03,
            0.03,
            0.0025,
            Solver::default(),
        )
        .expect("dc");
        let mut v = Vca::new(c, 48e3, cont);
        let mut worst = (0.0, 0.0f64);
        for (w, o) in st {
            let d = v.static_output(w, cont) - o;
            if d.abs() > worst.1.abs() {
                worst = (w, d);
            }
        }
        // The output swings 0..6.45 V.
        fail |= worst.1.abs() > 0.05;
        report.push_str(&format!(
            "contour {cont}: worst {:+.1} mV at {:+.4} V on the first pair\n",
            worst.1 * 1e3,
            worst.0
        ));
    }
    eprintln!("{report}");
    assert!(!fail, "budget: 50 mV of a 6.45 V swing\n{report}");
}

/// Uniformly resampled `v(main)` from a transient plot.
fn resampled(p: &Plot, t0: f64, sr: f64, n: usize) -> Vec<f64> {
    ca72_spice::resample(p.vec("time"), p.vec("main"), t0, sr, n)
}

#[test]
fn thump_matches_the_circuit() {
    let Some(spice) = for_test("thump_matches_the_circuit") else {
        return;
    };
    let (c, bench) = calibrated();
    let b = VcaBench { cont: 0.0, ..bench };
    let mut report = String::new();
    let mut fail = false;
    // A note: the contour rises to 5 V in `rise`, holds, falls as fast; no signal. The
    // fastest as a quick ATTACK makes it (the model's bias interpolated at 3 kHz once left a
    // spike of 0.8 V at the output here).
    for rise in [5e-3, 1e-3, 0.3e-3] {
        let pwl = format!(
            "vcont cont 0 pwl(0 0 0.05 0 {} 5 0.4 5 {} 0 1 0)",
            0.05 + rise,
            0.4 + rise
        );
        let cont = |t: f64| {
            if t < 0.05 {
                0.0
            } else if t < 0.05 + rise {
                (t - 0.05) / rise * 5.0
            } else if t < 0.4 {
                5.0
            } else if t < 0.4 + rise {
                5.0 - (t - 0.4) / rise * 5.0
            } else {
                0.0
            }
        };
        let tstop = 0.8;
        let p = lab::transient(
            &spice,
            &work_dir(&format!("vca-thump-{rise}")),
            &b,
            ("vsrc src 0 0", Some(&pwl)),
            tstop,
            (rise / 50.0).min(20e-6),
            Solver::default(),
        )
        .expect("tran");
        let sr = 48_000.0;
        let n = (tstop * sr) as usize;
        let ng = resampled(&p, 0.0, sr, n);
        let mut v = Vca::new(c, sr, 0.0);
        let rt: Vec<f64> = (0..n).map(|i| v.tick(0.0, cont(i as f64 / sr))).collect();
        let peak = ng.iter().fold(0.0f64, |a, x| a.max(x.abs()));
        let (k, err) = ng
            .iter()
            .zip(&rt)
            .enumerate()
            .fold((0, 0.0f64), |a, (k, (x, y))| {
                if (x - y).abs() > a.1 {
                    (k, (x - y).abs())
                } else {
                    a
                }
            });
        fail |= err >= 0.03 * peak;
        report.push_str(&format!(
            "thump, rise {:.1} ms: ngspice peak {:.1} mV; RT worst difference {:.2} mV at {:.4} s\n",
            rise * 1e3,
            peak * 1e3,
            err * 1e3,
            k as f64 / sr
        ));
    }
    eprintln!("{report}");
    assert!(!fail, "budget: 3 % of the peak\n{report}");
}

/// The first harmonics' amplitudes of a signal holding whole periods of `hz`.
fn harmonics_at(y: &[f64], sr: f64, hz: f64, count: usize) -> Vec<f64> {
    (1..=count)
        .map(|k| {
            let (mut a, mut b) = (0.0, 0.0);
            for (i, v) in y.iter().enumerate() {
                let ph = 2.0 * PI * k as f64 * hz * i as f64 / sr;
                a += v * ph.cos();
                b += v * ph.sin();
            }
            2.0 * (a * a + b * b).sqrt() / y.len() as f64
        })
        .collect()
}

#[test]
fn drive_distortion_matches_the_circuit() {
    let Some(spice) = for_test("drive_distortion_matches_the_circuit") else {
        return;
    };
    let (c, bench) = calibrated();
    let hz = 1000.0;
    let mut report = String::new();
    let mut fail = false;
    for (cont, amp) in [
        (5.0, 0.5),
        (5.0, 1.0),
        (5.0, 2.0),
        (5.0, 3.0),
        (5.0, 5.0),
        (2.0, 3.0),
    ] {
        let b = VcaBench { cont, ..bench };
        let tstop = 0.5;
        let p = lab::transient(
            &spice,
            &work_dir(&format!("vca-drive-{cont}-{amp}")),
            &b,
            (&format!("vsrc src 0 sin(0 {amp} {hz})"), None),
            tstop,
            1.0 / (hz * 500.0),
            Solver::default(),
        )
        .expect("tran");
        let sr = 192_000.0;
        let n = (0.1 * sr) as usize;
        let ng = resampled(&p, tstop - 0.1, sr, n);
        let mut v = Vca::new(c, sr, cont);
        let total = (tstop * sr) as usize;
        let rt: Vec<f64> = (0..total)
            .map(|i| v.tick(amp * (2.0 * PI * hz * i as f64 / sr).sin(), cont))
            .collect();
        let hn = harmonics_at(&ng, sr, hz, 7);
        let hr = harmonics_at(&rt[total - n..], sr, hz, 7);
        let mut line = format!("contour {cont}, {amp} V:");
        for (k, (a, r)) in hn.iter().zip(&hr).enumerate() {
            let level = 20.0 * (a / hn[0]).log10();
            let d = 20.0 * (r / a).log10();
            let budget = match k {
                0 => 0.2,
                1 | 2 => 0.6,
                _ => 1.5,
            };
            if level > -50.0 {
                fail |= d.abs() > budget;
                line.push_str(&format!(" H{} {level:.1}/{d:+.2}", k + 1));
            }
        }
        report.push_str(&line);
        report.push('\n');
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: H1 0.2 dB, H2 and H3 0.6 dB, the rest above -50 dB 1.5 dB (level/RT minus ngspice)\n{report}"
    );
}

#[test]
fn the_loudness_jack_matches_the_circuit() {
    let Some(spice) = for_test("the_loudness_jack_matches_the_circuit") else {
        return;
    };
    let (_, bench) = calibrated();
    let mut report = String::new();
    let mut fail = false;
    for v in [1.0, 2.0, 3.0, 4.0, 4.5, 5.0, 5.5, 6.0, 8.0, 10.0] {
        let b = VcaBench {
            cont: CONTOUR_FULL,
            j3: Some(v),
            ..bench
        };
        let ac = lab::ac_response(
            &spice,
            &work_dir(&format!("vca-j3-{v}")),
            &b,
            1000.0,
            1000.0,
            1,
            Solver::default(),
        )
        .expect("ac");
        let rt = rt_gain(&b, 1000.0);
        let d = rt - ac[0].1;
        // Above about 7 V an ideal source overdrives Q21 and shuts the VCA: both off
        // (below -60 dB) counts as agreement.
        let off = rt < -60.0 && ac[0].1 < -60.0;
        fail |= d.abs() > 0.15 && !off;
        report.push_str(&format!(
            "J3 at {v:+} V: ngspice {:+.2} dB at 1 kHz, RT {d:+.3} dB{}\n",
            ac[0].1,
            if off { " (both off)" } else { "" }
        ));
    }
    eprintln!("{report}");
    assert!(!fail, "budget: 0.15 dB (as the contour's)\n{report}");
}

/// EXT. LOUDNESS driven past its useful range, with the loudness contour low (the VCA's
/// first tail off, as between notes; -0.35 V is where the voice's contour rests) and full:
/// above about 6.4 V an ideal source overdrives Q21 and the second pair cuts off, which
/// shuts the VCA. The tails, the output at rest and the gain against ngspice, and the bias
/// solve settling each sample (a modular's 0 to 10 V envelope is an ordinary source here).
#[test]
fn the_loudness_jack_overdrive_matches_the_circuit() {
    let Some(spice) = for_test("the_loudness_jack_overdrive_matches_the_circuit") else {
        return;
    };
    let (c, bench) = calibrated();
    let mut report = String::new();
    let mut fail = false;
    for cont in [-0.35, 0.0, 2.0, CONTOUR_FULL] {
        for v in [5.0, 5.5, 6.0, 6.2, 6.4, 6.6, 7.0, 8.0, 9.0] {
            let b = VcaBench {
                cont,
                j3: Some(v),
                ..bench
            };
            let tag = format!("vca-over-{cont}-{v}");
            let t = lab::tails(&spice, &work_dir(&tag), &b, Solver::default()).expect("op");
            let ac = lab::ac_response(
                &spice,
                &work_dir(&format!("{tag}-ac")),
                &b,
                1000.0,
                1000.0,
                1,
                Solver::default(),
            )
            .expect("ac");
            let mut rt = Vca::new(circuit(&b), 48e3, cont);
            let rest = rt.rest_output(cont);
            let r = rt.bias();
            // Solved every sample as with the jack plugged: how many solves do not settle.
            rt.control_every = 1;
            ca72::unconverged::take();
            for _ in 0..200 {
                rt.tick(0.0, cont);
            }
            let unsettled: u64 = ca72::unconverged::take().iter().map(|(_, n)| n).sum();
            // As the voice runs it with the jack plugged: the bias solved every sample.
            let gain = rt_gain_every(&b, 1000.0, Some(1));
            let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-6);
            let worst = rel(r.i_a, t[0]).max(rel(r.i_b, t[1])).max(rel(r.i_c, t[2]));
            let off = gain < -60.0 && ac[0].1 < -60.0;
            let bad = worst > 2e-3
                || (rest - t[3]).abs() > 0.01
                || ((gain - ac[0].1).abs() > 0.15 && !off)
                || unsettled > 0;
            fail |= bad;
            report.push_str(&format!(
                "contour {cont:+}, J3 {v} V: ngspice Q18 {:.2} uA, Q21 {:.1} uA, Q1 {:.3} mA, rest {:.4} V, gain {:+.2} dB; RT tails worst {:.3} %, rest {:+.1} mV, gain {:+.3} dB{}; unsettled {unsettled}/200{}\n",
                t[0] * 1e6,
                t[1] * 1e6,
                t[2] * 1e3,
                t[3],
                ac[0].1,
                worst * 100.0,
                (rest - t[3]) * 1e3,
                gain - ac[0].1,
                if off { " (both off)" } else { "" },
                if bad { "  <--" } else { "" }
            ));
        }
    }
    let _ = c;
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: tails 0.2 % (floor 1 uA), output at rest 10 mV, gain 0.15 dB or both off, every solve settled\n{report}"
    );
}

#[test]
fn the_loudness_jack_follows_audio_rate_cv() {
    let Some(spice) = for_test("the_loudness_jack_follows_audio_rate_cv") else {
        return;
    };
    let (_, bench) = calibrated();
    // A 1 kHz tone at 50 mV into the VCA, the loudness contour full, J3 swept by a tremolo
    // between 1 and 5 V (the input's useful range) at `hz`; the voice's rate.
    let mut report = String::new();
    let mut fail = false;
    for hz in [0.0, 5.0, 100.0, 1000.0] {
        let b = VcaBench {
            cont: CONTOUR_FULL,
            j3: Some(0.0),
            ..bench
        };
        // Long enough for C2's and C6's transients from the two runs' starting points to die
        // away (110 and 53 ms).
        let tstop = 1.5;
        let net = lab::netlist(&b, "vsrc src 0 sin(0 0.05 1000)", Solver::default()).replace(
            "vext ext 0 0\n",
            // (ngspice takes a sine of 0 Hz as one at 1 / tstop: a steady 3 V as DC.)
            &if hz == 0.0 {
                "vext ext 0 3\n".to_string()
            } else {
                format!("vext ext 0 sin(3 2 {hz})\n")
            },
        );
        let plots = spice
            .run(
                &net,
                &[&format!("tran {:e} {tstop:e} 0 {:e}", 2e-6, 2e-6)],
                &work_dir(&format!("vca-j3-am-{hz}")),
            )
            .expect("tran");
        let sr = 48_000.0;
        let n = (0.2 * sr) as usize;
        let ng = resampled(&plots[0], tstop - 0.2, sr, n);
        let mut v = Vca::new(circuit(&b), sr, CONTOUR_FULL);
        // As the voice runs it with the jack plugged: the bias solved every sample.
        v.control_every = 1;
        let total = (tstop * sr) as usize;
        let rt: Vec<f64> = (0..total)
            .map(|i| {
                let t = i as f64 / sr;
                v.circuit.ext = 3.0 + 2.0 * (2.0 * PI * hz * t).sin();
                v.circuit.r_j3 = 0.0;
                v.tick(0.05 * (2.0 * PI * 1000.0 * t).sin(), CONTOUR_FULL)
            })
            .collect();
        let tail = &rt[total - n..];
        let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
        let err: Vec<f64> = ng.iter().zip(tail).map(|(a, b)| a - b).collect();
        let worst = err.iter().fold(0.0f64, |a, e| a.max(e.abs()));
        let below = 20.0 * (rms(&ng) / rms(&err)).log10();
        // A tremolo at 1 kHz is 27.9 dB below: the model's step, which converges as it is
        // oversampled (31.4, 33.8, 35.2 dB at 2, 4, 8 times; `tremolo_step_budget`). The
        // model before its fast-attack correction read 35.5 dB here by an accident: its
        // tails, two Newton steps behind, cancelled part of that error.
        fail |= below < if hz < 500.0 { 33.0 } else { 27.5 };
        report.push_str(&format!(
            "tremolo at {hz} Hz: ngspice {:.2} mV rms, RT difference {:.3} mV rms ({:.1} dB below), worst {:.3} mV\n",
            rms(&ng) * 1e3,
            rms(&err) * 1e3,
            below,
            worst * 1e3
        ));
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: the difference 33 dB below the output up to 100 Hz (the VCA's thump and its\n\
         pairs' linearised base current; board4.md), 27.5 dB at 1 kHz (the model's step)\n{report}"
    );
}

/// Diagnostic for the tremolo's budget: the real-time VCA at 1, 2, 4 and 8 times the
/// voice's rate against ngspice, a 100 Hz and a 1 kHz tremolo at EXT. LOUDNESS; and at the
/// voice's rate against ngspice with every transistor's junction capacitances and transit
/// times zeroed (whether the difference is the devices' charges or the model's step).
/// `cargo test -p ca72-lab --test vca_realtime tremolo_step_budget -- --ignored --nocapture`
#[test]
#[ignore]
fn tremolo_step_budget() {
    let Some(spice) = for_test("tremolo_step_budget") else {
        return;
    };
    let (_, bench) = calibrated();
    let sr = 48_000.0;
    let n = (0.2 * sr) as usize;
    let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    let below = |ng: &[f64], rt: &[f64]| {
        let err: Vec<f64> = ng.iter().zip(rt).map(|(a, b)| a - b).collect();
        20.0 * (rms(ng) / rms(&err)).log10()
    };
    // The device models with CJE, CJC, TF and TR set to 0.
    let dev = ca72_lab::circuits_dir().join("models/mm-devices.lib");
    let no_charges: String = std::fs::read_to_string(&dev)
        .unwrap()
        .split_inclusive([' ', '\n'])
        .map(|tok| {
            let bare = tok.trim_end_matches([' ', '\n', ')']);
            match bare.split_once('=') {
                Some((k, _)) if ["CJE", "CJC", "TF", "TR"].contains(&k) => {
                    format!("{k}=0{}", &tok[bare.len()..])
                }
                _ => tok.to_string(),
            }
        })
        .collect();
    assert!(
        no_charges.contains("CJE=0 ")
            && !no_charges.contains("CJE=5p")
            && !no_charges.contains("TR=60n"),
        "the device models' charges were not all zeroed"
    );
    for hz in [100.0, 1000.0] {
        let b = VcaBench {
            cont: CONTOUR_FULL,
            j3: Some(0.0),
            ..bench
        };
        let tstop = 1.5;
        let net = lab::netlist(&b, "vsrc src 0 sin(0 0.05 1000)", Solver::default())
            .replace("vext ext 0 0\n", &format!("vext ext 0 sin(3 2 {hz})\n"));
        let tran = format!("tran {:e} {tstop:e} 0 {:e}", 1e-6, 1e-6);
        let dir = work_dir(&format!("vca-j3-os-{hz}"));
        let ng = resampled(
            &spice.run(&net, &[&tran], &dir).expect("tran")[0],
            tstop - 0.2,
            sr,
            n,
        );
        let dir = work_dir(&format!("vca-j3-nocharge-{hz}"));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = dir.join("devices.lib");
        std::fs::write(&lib, &no_charges).unwrap();
        let bare = net.replace(&dev.display().to_string(), &lib.display().to_string());
        let ng_bare = resampled(
            &spice.run(&bare, &[&tran], &dir).expect("tran")[0],
            tstop - 0.2,
            sr,
            n,
        );
        for os in [1usize, 2, 4, 8] {
            let rate = sr * os as f64;
            let mut v = Vca::new(circuit(&b), rate, CONTOUR_FULL);
            v.control_every = 1;
            let total = (tstop * rate) as usize;
            let rt: Vec<f64> = (0..total)
                .map(|i| {
                    let t = i as f64 / rate;
                    v.circuit.ext = 3.0 + 2.0 * (2.0 * PI * hz * t).sin();
                    v.circuit.r_j3 = 0.0;
                    v.tick(0.05 * (2.0 * PI * 1000.0 * t).sin(), CONTOUR_FULL)
                })
                .collect();
            let tail: Vec<f64> = (0..n).map(|i| rt[total - n * os + i * os]).collect();
            eprintln!(
                "tremolo {hz} Hz, RT at {os}x: {:.1} dB below ngspice ({:.1} dB below ngspice without the transistors' charges)",
                below(&ng, &tail),
                below(&ng_bare, &tail)
            );
        }
    }
}
