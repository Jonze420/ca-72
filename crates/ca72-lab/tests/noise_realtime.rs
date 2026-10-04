//! Board 3's noise generator in real time against ngspice (docs/circuit/board3.md): its
//! operating point, its small-signal response from Q15's junction to the white, pink and red
//! outputs as loaded, the discretised response against the continuous one, and the
//! generated noise's spectrum against the response.

use ca72::noise::{self, Noise, NoiseCircuit, NoiseLoads};
use ca72_lab::bench::Solver;
use ca72_lab::board3::{self as lab, NoiseBench};
use ca72_lab::work_dir;
use ca72_spice::for_test;

fn db(z: (f64, f64)) -> f64 {
    10.0 * (z.0 * z.0 + z.1 * z.1).log10()
}

fn phase(z: (f64, f64)) -> f64 {
    z.1.atan2(z.0).to_degrees()
}

#[test]
fn the_noise_generator_matches_the_circuit() {
    let Some(spice) = for_test("the_noise_generator_matches_the_circuit") else {
        return;
    };
    // Loads as the NOISE switch at WHITE and the mixer's VOLUME at 10 leave them: white into
    // the VOLUME pot (25K), pink into R24 and half the MODULATION MIX pot; red as drawn.
    let loads = NoiseLoads {
        white: 25e3,
        pink: 24e3 + 12.5e3 * 43e3 / (12.5e3 + 43e3),
        red: f64::INFINITY,
    };
    let k = NoiseCircuit::default();
    let bench = NoiseBench {
        r26: k.r26,
        vbr: k.vbr,
        rd: k.rd,
        white: loads.white,
        pink: loads.pink,
        red: loads.red,
    };
    let (op, ac) = lab::noise_ac(
        &spice,
        &work_dir("noise-ac"),
        &bench,
        1.0,
        20e3,
        10,
        Solver::default(),
    )
    .expect("ac");
    let mut report = String::new();
    let mut fail = Vec::new();
    // The operating point: the transistors' collectors and emitters.
    let c = noise::circuit(&k, &loads).expect("dc");
    let names = ["x1.c12n", "x1.e4", "x1.c3n", "x1.e3", "x1.c6n", "x1.e6"];
    let nodes = [7usize, 10, 16, 17, 22, 23];
    let mut worst_op = 0.0f64;
    for (name, &n) in names.iter().zip(&nodes) {
        let d = c.v(n) - op.scalar(&format!("v({name})"));
        worst_op = worst_op.max(d.abs());
    }
    report.push_str(&format!(
        "operating point: within {:.3} mV of ngspice at the transistors\n",
        worst_op * 1e3
    ));
    if worst_op > 1e-3 {
        fail.push(format!("operating point {:.3} mV", worst_op * 1e3));
    }
    // The responses.
    let rate = 48_000.0;
    let mut nz = Noise::new(k, loads, rate, 1).expect("noise");
    let (model, sys, out) = nz.model_of();
    let (model, sys) = (model.clone(), sys.clone());
    let f = ac.vec("frequency");
    for (k_out, name) in ["white", "pink", "red"].iter().enumerate() {
        let ng = ac
            .complex_vec(&format!("v({name})"))
            .or_else(|| ac.complex_vec(name))
            .expect("ac vector");
        let (mut worst_a, mut worst_p, mut worst_d) = ((0.0, 0.0f64), 0.0f64, (0.0, 0.0f64));
        // The discretisation where the output matters: within 40 dB of its peak.
        let peak = f
            .iter()
            .map(|&fi| db(model.response(out[k_out], fi)))
            .fold(f64::MIN, f64::max);
        for (i, &fi) in f.iter().enumerate() {
            let rt = model.response(out[k_out], fi);
            let d = db(rt) - db(ng[i]);
            if d.abs() > worst_a.1.abs() {
                worst_a = (fi, d);
            }
            let mut dp = phase(rt) - phase(ng[i]);
            dp -= 360.0 * (dp / 360.0).round();
            worst_p = worst_p.max(dp.abs());
            // Weighted: the trapezoidal rule's warping grows toward 20 kHz (3.8 % there at
            // 4x): 0.3 dB allowed to 16 kHz, 0.6 dB to 20 kHz.
            let dd = db(sys.response(out[k_out], fi)) - db(rt);
            let allowed = if fi <= 16e3 { 0.3 } else { 0.6 };
            if fi <= 20e3 && db(rt) > peak - 40.0 && dd.abs() / allowed > worst_d.1.abs() / 0.3 {
                worst_d = (fi, dd * 0.3 / allowed);
            }
        }
        let at = |fi: f64| db(model.response(out[k_out], fi));
        report.push_str(&format!(
            "{name}: {:+.1} dB at 100 Hz, {:+.1} at 1 kHz, {:+.1} at 10 kHz; against ngspice within {:+.3} dB ({:.0} Hz), \
             {:.2} degrees; discretised (4x) within {:+.3} dB of continuous as weighted ({:.0} Hz; 0.3 dB to 16 kHz, 0.6 to 20 kHz; within 40 dB of its peak)\n",
            at(100.0),
            at(1e3),
            at(10e3),
            worst_a.1,
            worst_a.0,
            worst_p,
            worst_d.1,
            worst_d.0
        ));
        if worst_a.1.abs() > 0.05 || worst_p > 0.5 || worst_d.1.abs() > 0.3 {
            fail.push(format!(
                "{name}: {:+.3} dB, {:.2} degrees, discretised {:+.3} dB",
                worst_a.1, worst_p, worst_d.1
            ));
        }
    }
    // The generated noise: a unit density source, its spectrum (averaged periodograms)
    // against the continuous response in octave bands.
    nz.set_density(1.0);
    let lat = 2 * rate as usize / 10;
    for _ in 0..lat {
        nz.tick();
    }
    let seg = 4096usize;
    let segs = 200usize;
    let mut psd = [vec![0.0; seg / 2], vec![0.0; seg / 2], vec![0.0; seg / 2]];
    let win: Vec<f64> = (0..seg)
        .map(|i| 0.5 - 0.5 * (2.0 * core::f64::consts::PI * i as f64 / seg as f64).cos())
        .collect();
    let wpow: f64 = win.iter().map(|w| w * w).sum();
    let mut buf = [vec![0.0; seg], vec![0.0; seg], vec![0.0; seg]];
    for _ in 0..segs {
        for i in 0..seg {
            let o = nz.tick();
            buf[0][i] = o.white * win[i];
            buf[1][i] = o.pink * win[i];
            buf[2][i] = o.red * win[i];
        }
        for k_out in 0..3 {
            // A direct DFT at the bins used (octave bands up to 16 kHz).
            for (bin, p) in psd[k_out].iter_mut().enumerate().skip(1) {
                let fb = bin as f64 * rate / seg as f64;
                if !(20.0..=16e3).contains(&fb) || !is_band_bin(bin, rate, seg) {
                    continue;
                }
                let (mut re, mut im) = (0.0, 0.0);
                let w = 2.0 * core::f64::consts::PI * bin as f64 / seg as f64;
                for (i, &x) in buf[k_out].iter().enumerate() {
                    re += x * (w * i as f64).cos();
                    im -= x * (w * i as f64).sin();
                }
                // One-sided density, V^2/Hz.
                *p += 2.0 * (re * re + im * im) / (wpow * rate) / segs as f64;
            }
        }
    }
    let mut worst_s = (String::new(), 0.0f64);
    for (k_out, name) in ["white", "pink", "red"].iter().enumerate() {
        // Octave bands, the last from 11.3 to 16 kHz (the decimators' transition starts at
        // 20 kHz).
        for band in [31.25, 125.0, 500.0, 2000.0, 8000.0, 16000.0] {
            let (lo, hi) = if band > 10e3 {
                (11.3e3, 16e3)
            } else {
                (band / 1.414, band * 1.414)
            };
            let bins: Vec<usize> = (1..seg / 2)
                .filter(|&b| {
                    let fb = b as f64 * rate / seg as f64;
                    fb >= lo && fb < hi && is_band_bin(b, rate, seg)
                })
                .collect();
            if bins.is_empty() {
                continue;
            }
            let meas: f64 = bins.iter().map(|&b| psd[k_out][b]).sum::<f64>() / bins.len() as f64;
            let want: f64 = bins
                .iter()
                .map(|&b| {
                    let z = model.response(out[k_out], b as f64 * rate / seg as f64);
                    z.0 * z.0 + z.1 * z.1
                })
                .sum::<f64>()
                / bins.len() as f64;
            let d = 10.0 * (meas / want).log10();
            if d.abs() > worst_s.1.abs() {
                worst_s = (format!("{name} at {band} Hz"), d);
            }
        }
    }
    report.push_str(&format!("generated noise against the response (octave bands, 20 Hz to 16 kHz): within {:+.2} dB ({})\n", worst_s.1, worst_s.0));
    // 200 averages of a few bins: about 0.3 dB of statistical spread at the lowest band.
    if worst_s.1.abs() > 0.8 {
        fail.push(format!("spectrum {:+.2} dB ({})", worst_s.1, worst_s.0));
    }
    eprintln!("{report}");
    assert!(fail.is_empty(), "{fail:?}\n{report}");
}

/// Bins the spectrum test measures: every bin up to 256, then every eighth (a DFT per bin
/// is slow).
fn is_band_bin(bin: usize, _rate: f64, _seg: usize) -> bool {
    bin < 256 || bin.is_multiple_of(8)
}
