//! Board 4's external preamplifier with its overload lamp driver (circuit No. 12) and the
//! A-440 (circuit No. 13) in real time against ngspice (docs/circuit/board4.md).

use ca72::preamp::{Preamp, PreampCircuit, SUBSTEPS};
use ca72_lab::bench::Solver;
use ca72_lab::board4ext::{self as lab, PreampBench};
use ca72_lab::work_dir;
use ca72_spice::for_test;

const RATE: f64 = 48_000.0;

/// ngspice's preamplifier driven by `source` for `seconds`: its output (Q33's collector), the
/// bus current through R46, and the lamp's current, at the output rate.
fn circuit(spice: &ca72_spice::Ngspice, name: &str, source: &str, seconds: f64) -> [Vec<f64>; 3] {
    let b = PreampBench::default();
    let net = lab::preamp_netlist(&b, source, Solver::default());
    let fine = RATE * SUBSTEPS as f64;
    let step = 1.0 / fine;
    let p = spice
        .run(
            &net,
            &[&format!("tran {step:e} {seconds:e} 0 {:e}", step / 2.0)],
            &work_dir(name),
        )
        .expect("tran")
        .pop()
        .expect("plot");
    let n = (seconds * fine) as usize;
    let t = p.vec("time");
    let get = |name: &str| ca72_spice::resample(t, p.vec(name), 0.0, fine, n + 1);
    let (amp, out, lamp) = (get("v(amp)"), get("v(out)"), get("v(lamp)"));
    let at = |x: &[f64], f: &dyn Fn(f64) -> f64| -> Vec<f64> {
        (0..n / SUBSTEPS).map(|k| f(x[k * SUBSTEPS])).collect()
    };
    [
        at(&amp, &|v| v),
        at(&out, &|v| v / 33e3),
        // The lamp's current, sampled at each sample's end as the model's lamp is.
        (0..n / SUBSTEPS)
            .map(|k| (15.0 - lamp[(k + 1) * SUBSTEPS]) / b.r_lamp)
            .collect(),
    ]
}

/// The real-time model driven by `v(t)` for `seconds`: output, bus current, lamp current.
fn model(v: &dyn Fn(f64) -> f64, seconds: f64) -> ([Vec<f64>; 3], usize) {
    let c = PreampCircuit::default();
    let mut p = Preamp::new(c, RATE, PreampBench::default().r_src).expect("dc");
    let n = (seconds * RATE) as usize;
    let mut out = [Vec::new(), Vec::new(), Vec::new()];
    for k in 0..n {
        let o = p.tick(v(k as f64 / RATE));
        out[0].push(o.amp);
        out[1].push(o.i_bus);
        out[2].push(o.lamp * c.i_lamp);
    }
    (out, p.failed)
}

fn rms(x: &[f64]) -> f64 {
    let m = x.iter().sum::<f64>() / x.len() as f64;
    (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64).sqrt()
}

#[test]
fn the_preamplifier_matches_the_circuit() {
    let Some(spice) = for_test("the_preamplifier_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = Vec::new();
    // Small signals: 5 mV through R9's 100K at four frequencies; the gain over the last
    // 20 ms against ngspice's.
    for hz in [100.0, 1000.0, 10e3, 20e3] {
        let a = 0.005;
        let secs = 0.06;
        let ng = circuit(
            &spice,
            &format!("preamp-{hz}"),
            &format!("vin src 0 sin(0 {a} {hz})"),
            secs,
        );
        let (rt, failed) = model(&|t| a * (2.0 * std::f64::consts::PI * hz * t).sin(), secs);
        let tail = (0.04 * RATE) as usize..(secs * RATE) as usize;
        let (g_ng, g_rt) = (
            rms(&ng[0][tail.clone()]) / (a / 2f64.sqrt()),
            rms(&rt[0][tail.clone()]) / (a / 2f64.sqrt()),
        );
        let d = 20.0 * (g_rt / g_ng).log10();
        let (b_ng, b_rt) = (rms(&ng[1][tail.clone()]), rms(&rt[1][tail.clone()]));
        let db = 20.0 * (b_rt / b_ng).log10();
        report.push_str(&format!(
            "{hz} Hz: gain {:.2} dB (ngspice {:.2} dB, {d:+.3} dB); the bus current {db:+.3} dB\n",
            20.0 * g_rt.log10(),
            20.0 * g_ng.log10(),
        ));
        // The model's input is 48 kHz samples; ngspice's a continuous sine: the resamplers'
        // passband droop shows toward 20 kHz.
        let allowed = if hz > 15e3 { 0.3 } else { 0.05 };
        if d.abs() > allowed || db.abs() > allowed || failed > 0 {
            fail.push(format!("{hz} Hz: {d:+.3} dB, {failed} failed"));
        }
    }
    // Overdriven: a 1 kHz burst of 200 mV (about 20 V wanted at the output) for 50 ms, then
    // silence: the clipped waveform and the lamp lighting and letting go.
    let secs = 0.6;
    let burst = |t: f64| {
        if t < 0.05 {
            0.2 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()
        } else {
            0.0
        }
    };
    let ng = circuit(
        &spice,
        "preamp-burst",
        "bsrc src 0 v = 0.2*sin(6.283185307179586*1000*time)*(1-u(time-0.05))",
        secs,
    );
    let (rt, failed) = model(&burst, secs);
    let span = (0.01 * RATE) as usize..(0.05 * RATE) as usize;
    let peaks = |x: &[f64]| {
        span.clone()
            .fold((f64::MIN, f64::MAX), |(a, b), k| (a.max(x[k]), b.min(x[k])))
    };
    let (pk_hi, pk_lo) = peaks(&ng[0]);
    let (rt_hi, rt_lo) = peaks(&rt[0]);
    // The clipped output's two levels (the resamplers delay the model's by a few samples).
    let worst_amp = (rt_hi - pk_hi).abs().max((rt_lo - pk_lo).abs());
    let lit = |x: &[f64]| {
        let on = x.iter().position(|&i| i > 0.5 * 0.06);
        let off = x.iter().rposition(|&i| i > 0.5 * 0.06);
        (on.map(|k| k as f64 / RATE), off.map(|k| k as f64 / RATE))
    };
    let (ng_lit, rt_lit) = (lit(&ng[2]), lit(&rt[2]));
    let peak_lamp = |x: &[f64]| x.iter().fold(0.0f64, |a, &b| a.max(b));
    report.push_str(&format!(
        "overdriven: output {pk_lo:+.2}..{pk_hi:+.2} V in ngspice, the model's clip levels within {:.3} V; the lamp over half its current from {:?} to {:?} s \
         (ngspice {:?} to {:?}), peak {:.1} mA (ngspice {:.1})\n",
        worst_amp,
        rt_lit.0,
        rt_lit.1,
        ng_lit.0,
        ng_lit.1,
        peak_lamp(&rt[2]) * 1e3,
        peak_lamp(&ng[2]) * 1e3
    ));
    let close = |a: Option<f64>, b: Option<f64>, tol: f64| match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() <= tol,
        (None, None) => true,
        _ => false,
    };
    if worst_amp > 0.3
        || failed > 0
        || !close(rt_lit.0, ng_lit.0, 0.002)
        || !close(rt_lit.1, ng_lit.1, 0.01)
    {
        fail.push(format!(
            "overdriven: {worst_amp:.3} V, lamp {rt_lit:?} against {ng_lit:?}, {failed} failed"
        ));
    }
    eprintln!("{report}");
    assert!(fail.is_empty(), "{fail:?}\n{report}");
}
