//! Oscillator 3's reverse sawtooth (Q37) in real time against ngspice: the stage driven by
//! ngspice's own ramp buffer, its sawtooth as R171 loads it and its output, at 110 Hz and
//! slow enough for C10's ripple to show.

use ca72::revsaw::{RevSaw, RevSawCircuit};
use ca72_lab::bench::Solver;
use ca72_lab::vco::{self as lab, Controls, LOW_A, Osc, Range, Trims};
use ca72_lab::work_dir;
use ca72_spice::for_test;

#[test]
fn the_reverse_sawtooth_matches_the_circuit() {
    let Some(spice) = for_test("the_reverse_sawtooth_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    for (name, osc, range) in [
        (
            "110 Hz",
            Osc::Three {
                freq: 0.5,
                control: true,
            },
            Range::R8,
        ),
        (
            "LO, CONTROL off",
            Osc::Three {
                freq: 0.8,
                control: false,
            },
            Range::Lo,
        ),
    ] {
        let ctl = Controls {
            osc,
            kbd: LOW_A,
            range,
            ..Controls::default()
        };
        let m = lab::measure(
            &spice,
            &work_dir(&format!("revsaw-{name}")),
            &Trims::default(),
            &ctl,
            Solver::default(),
            4,
        )
        .expect("measure");
        let p = &m.plot;
        let t = p.vec("time");
        // The stage at 1 MHz (the ramp's reset takes microseconds), driven by ngspice's
        // buffer through the sawtooth's divider.
        let rate = 1e6;
        let t0 = t[t.len() / 3];
        let n = ((t[t.len() - 1] - t0) * rate) as usize;
        let rs = |name: &str| ca72_spice::resample(t, p.vec(name), t0, rate, n);
        let (buf, saw_ng, rev_ng) = (rs("v(buf)"), rs("v(saw)"), rs("v(rev)"));
        let mut st = RevSaw::new(RevSawCircuit::default(), rate).expect("dc");
        // Settle C10 on the first period, then compare.
        let settle = n / 3;
        let (mut worst_saw, mut worst_rev, mut peak) = (0.0f64, 0.0f64, (f64::MAX, f64::MIN));
        // Away from the resets: 5 us after the buffer last moved fast (the stage's
        // transistor capacitances take microseconds to recover from the reset's 8 V edge,
        // where backward Euler at 1 us steps lags ngspice by about a step).
        let mut since_edge = usize::MAX;
        for k in 0..n {
            let o = st.tick(buf[k] * 4700.0 / 9000.0);
            let fast = k > 0 && (buf[k] - buf[k - 1]).abs() * rate > 1e3 * m.hz.max(1.0);
            since_edge = if fast {
                0
            } else {
                since_edge.saturating_add(1)
            };
            let quiet = since_edge >= 5;
            if k >= settle && quiet {
                worst_saw = worst_saw.max((o.saw - saw_ng[k]).abs());
                worst_rev = worst_rev.max((o.rev - rev_ng[k]).abs());
                peak = (peak.0.min(rev_ng[k]), peak.1.max(rev_ng[k]));
            }
        }
        fail |= worst_saw > 2e-3 || worst_rev > 2e-3 || st.failed > 0;
        report.push_str(&format!(
            "{name} ({:.3} Hz): reverse sawtooth {:+.3}..{:+.3} V unloaded; the loaded sawtooth within {:.2} mV, the reverse within {:.2} mV\n",
            m.hz,
            peak.0,
            peak.1,
            worst_saw * 1e3,
            worst_rev * 1e3
        ));
    }
    // The output's source resistance: the model's small-signal estimate against ngspice's
    // stage loaded with 10K to -10 V at rest.
    let r_rt = RevSaw::new(RevSawCircuit::default(), 48e3)
        .expect("dc")
        .r_out();
    let dir = ca72_lab::circuits_dir();
    let net = |load: &str| {
        format!(
            "revsaw\n.include {}\n.include {}\nvp vp 0 10\nvn vn 0 -10\nvsaw s 0 0\nrsaw s saw {}\n\
             x37 saw rev vp vn mm_revsaw\n{load}",
            dir.join("models/mm-devices.lib").display(),
            dir.join("boards/board1-osc23.lib").display(),
            4300.0 * 4700.0 / 9000.0
        )
    };
    let op = |load: &str| {
        spice
            .run(&net(load), &["op"], &work_dir("revsaw-rout"))
            .expect("op")[0]
            .scalar("v(rev)")
    };
    let (v0, v1) = (op(""), op("rload rev vn 10k\n"));
    let r_ng = (v0 - v1) / ((v1 + 10.0) / 10e3);
    report.push_str(&format!(
        "output resistance: {r_rt:.1} ohm, ngspice {r_ng:.1} ohm\n"
    ));
    fail |= (r_rt / r_ng - 1.0).abs() > 0.01;
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: the sawtooth within 2 mV, the reverse within 5 mV\n{report}"
    );
}
