//! Board 3's dual regulator (circuit No. 10) in ngspice (docs/circuit/board3.md): its rails'
//! output impedance, their coupling, their load regulation and their rejection of the
//! rectifier's ripple, the grounds for modelling them as ideal +-10 V (assumptions A1).

use ca72_lab::bench::Solver;
use ca72_lab::board3::{self as lab, RegulatorAc, RegulatorBench};
use ca72_lab::work_dir;
use ca72_spice::for_test;

fn mag(z: (f64, f64)) -> f64 {
    z.0.hypot(z.1)
}

#[test]
fn the_regulator_holds_its_rails() {
    let Some(spice) = for_test("the_regulator_holds_its_rails") else {
        return;
    };
    let s = Solver::default();
    let work = work_dir("regulator");
    let b = lab::regulator_trim(&spice, &work, &RegulatorBench::default(), s).expect("trim");
    let op = lab::regulator_op(&spice, &work, &b, s).expect("op");
    let mut report = String::new();
    let mut fail = Vec::new();
    let (vp, vn) = (op.scalar("v(outp)"), op.scalar("v(outn)"));
    // The reference's current (R44) and the pairs' tails, as designed: CR3's 7.5 mA.
    let i_z = op.scalar("v(x1.ref)") / 511.0;
    let i_tail_p = (vp - op.scalar("v(x1.em)")) / 560.0;
    let i_tail_n = (op.scalar("v(x1.et)") - vn) / 910.0;
    report.push_str(&format!(
        "trimmed: +10 V {vp:.4} V (R21 wiper {:.3}), -10 V {vn:.4} V (R58 wiper {:.3}); CR3 {:.2} mA (1N821: 7.5), \
         the pairs' tails {:.2} and {:.2} mA\n",
        b.a21,
        b.a58,
        i_z * 1e3,
        i_tail_p * 1e3,
        i_tail_n * 1e3
    ));
    if (vp - 10.0).abs() > 1e-3 || (vn + 10.0).abs() > 1e-3 || !(5e-3..10e-3).contains(&i_z) {
        fail.push("operating point".to_string());
    }
    // Load regulation: each rail's load from 50 to 300 mA.
    let at = |ip: f64, i_n: f64| {
        let mut t = b;
        t.i_p = ip;
        t.i_n = i_n;
        let o = lab::regulator_op(&spice, &work, &t, s).expect("op");
        (o.scalar("v(outp)"), o.scalar("v(outn)"))
    };
    let (p50, n50) = at(0.05, 0.05);
    let (p300, n300) = at(0.3, 0.3);
    let dc_p = (p50 - p300) / 0.25;
    let dc_n = (n300 - n50) / 0.25;
    report.push_str(&format!(
        "load regulation, 50 to 300 mA on both: +10 V moves {:.3} mV ({:.2} mohm), -10 V {:.3} mV ({:.2} mohm)\n",
        (p300 - p50) * 1e3,
        dc_p * 1e3,
        (n300 - n50) * 1e3,
        dc_n.abs() * 1e3
    ));
    // Output impedance and coupling, 10 Hz to 100 kHz.
    let band = (10.0, 100e3, 20);
    let zp = lab::regulator_ac(&spice, &work, &b, RegulatorAc::LoadP, band, s).expect("ac");
    let zn = lab::regulator_ac(&spice, &work, &b, RegulatorAc::LoadN, band, s).expect("ac");
    let f = zp.vec("frequency").to_vec();
    let curve = |p: &ca72_spice::Plot, node: &str| -> Vec<f64> {
        p.complex_vec(&format!("v({node})"))
            .or_else(|| p.complex_vec(node))
            .expect("ac vector")
            .iter()
            .map(|&z| mag(z))
            .collect()
    };
    let (zpp, znn, zpn, znp) = (
        curve(&zp, "outp"),
        curve(&zn, "outn"),
        curve(&zp, "outn"),
        curve(&zn, "outp"),
    );
    let pick = |c: &[f64], hz: f64| -> f64 {
        let i = f
            .iter()
            .position(|&x| x >= hz * 0.999)
            .unwrap_or(f.len() - 1);
        c[i]
    };
    let peak = |c: &[f64]| -> (f64, f64) {
        c.iter()
            .zip(&f)
            .fold((0.0, 0.0), |a, (&z, &fr)| if z > a.0 { (z, fr) } else { a })
    };
    for (name, c) in [
        ("+10 V", &zpp),
        ("-10 V", &znn),
        ("-10 V from +10 V's load", &zpn),
        ("+10 V from -10 V's load", &znp),
    ] {
        let (pk, pf) = peak(c);
        report.push_str(&format!(
            "{name}: {:.2} mohm at 100 Hz, {:.2} at 1 kHz, {:.1} at 10 kHz; peak {:.1} mohm at {:.0} Hz\n",
            pick(c, 100.0) * 1e3,
            pick(c, 1e3) * 1e3,
            pick(c, 10e3) * 1e3,
            pk * 1e3,
            pf
        ));
    }
    // Ripple: 120 Hz on each unregulated rail.
    let lp =
        lab::regulator_ac(&spice, &work, &b, RegulatorAc::LineP, (120.0, 120.0, 1), s).expect("ac");
    let ln =
        lab::regulator_ac(&spice, &work, &b, RegulatorAc::LineN, (120.0, 120.0, 1), s).expect("ac");
    let first = |p: &ca72_spice::Plot, node: &str| {
        mag(p.complex_vec(&format!("v({node})")).expect("ac")[0])
    };
    let db = |x: f64| 20.0 * x.log10();
    let (pp, pn, np, nn) = (
        first(&lp, "outp"),
        first(&lp, "outn"),
        first(&ln, "outp"),
        first(&ln, "outn"),
    );
    // 150 mA from 1000 uF at 120 Hz: 1.25 V peak to peak.
    let ripple = 0.15 / (120.0 * 1000e-6);
    report.push_str(&format!(
        "ripple rejection at 120 Hz: +UNREG to +10 V {:.1} dB, to -10 V {:.1} dB; -UNREG to -10 V {:.1} dB, to +10 V {:.1} dB; \
         with {ripple:.2} V p-p on each unregulated rail: {:.0} uV p-p on +10 V, {:.0} uV on -10 V\n",
        db(pp),
        db(pn),
        db(nn),
        db(np),
        (pp + np) * ripple * 1e6,
        (pn + nn) * ripple * 1e6
    ));
    eprintln!("{report}");
    // Grounds for ideal rails (board3.md): under 0.1 ohm across the audio band, and ripple
    // held 50 dB down.
    let worst_z = peak(&zpp)
        .0
        .max(peak(&znn).0)
        .max(peak(&zpn).0)
        .max(peak(&znp).0);
    if worst_z > 0.1 {
        fail.push(format!(
            "output impedance peaks at {:.1} mohm",
            worst_z * 1e3
        ));
    }
    if db(pp.max(pn).max(np).max(nn)) > -50.0 {
        fail.push("ripple rejection under 50 dB".into());
    }
    assert!(fail.is_empty(), "{fail:?}\n{report}");
}
