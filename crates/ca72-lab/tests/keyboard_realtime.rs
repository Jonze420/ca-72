//! The real-time keyboard circuit against ngspice (docs/circuit/board2.md): the string's
//! current and every key's output, then playing: detached and legato notes with GLIDE off,
//! glides up and down, a glide released half way and held, and releases with the trigger
//! contact opening ahead of the pitch contact.

use ca72::keyboard::{Keyboard, KeyboardCircuit};
use ca72_lab::bench::Solver;
use ca72_lab::keyboard::{self as lab, KeyboardBench};
use ca72_lab::work_dir;
use ca72_spice::for_test;

const SR: f64 = 48_000.0;

#[test]
fn the_string_and_every_key_match_the_circuit() {
    let Some(spice) = for_test("the_string_and_every_key_match_the_circuit") else {
        return;
    };
    let (keys, i_ng) = lab::static_keys(
        &spice,
        &work_dir("keyboard-static"),
        lab::LOAD_DEFAULT,
        Solver::default(),
    )
    .expect("op");
    let rt = Keyboard::new(KeyboardCircuit::default(), SR);
    let i_rt = rt.string_current();
    let mut worst = (0, 0.0f64);
    for (k, &(_, out)) in keys.iter().enumerate() {
        let d = rt.static_out(k).expect("settle") - out;
        if d.abs() > worst.1.abs() {
            worst = (k, d);
        }
    }
    eprintln!(
        "string current: ngspice {:.5} mA, RT {:.5} mA ({:+.1e}); keys 0..43 ({:.4}..{:.4} V): worst {:+.4} mV at key {}",
        i_ng * 1e3,
        i_rt * 1e3,
        i_rt / i_ng - 1.0,
        keys[0].1,
        keys[43].1,
        worst.1 * 1e3,
        worst.0
    );
    assert!((i_rt / i_ng - 1.0).abs() < 1e-5, "string current");
    // 0.01 mV is 0.012 cent at the keyboard's 84 mV per key.
    assert!(
        worst.1.abs() < 1e-5,
        "key {}: {:+.4} mV",
        worst.0,
        worst.1 * 1e3
    );
    assert_eq!(rt.failed, 0);
}

struct Case {
    name: &'static str,
    presses: &'static [(usize, f64, f64)],
    glide: Option<f64>,
    contact_lead: f64,
    tstop: f64,
}

/// The contacts for sample `i` (the step from i / SR): the lowest and highest keys whose
/// pitch contacts are closed, and whether any trigger contact is, as the bench closes them
/// (each time rounded to its sample: 0.1 + 0.002 s must not land a sample late).
fn contacts(c: &Case, i: usize) -> (Option<(usize, usize)>, bool) {
    let at = |t: f64| (t * SR).round() as usize;
    let held = || {
        c.presses
            .iter()
            .filter(|&&(_, d, u)| i >= at(d) && i < at(u))
            .map(|&(k, _, _)| k)
    };
    let key = held().min().zip(held().max());
    let trig = c.presses.iter().any(|&(_, d, u)| {
        let d = if d <= 0.0 { 0.0 } else { d + c.contact_lead };
        i >= at(d) && i < at(u - c.contact_lead)
    });
    (key, trig)
}

#[test]
fn playing_matches_the_circuit() {
    let Some(spice) = for_test("playing_matches_the_circuit") else {
        return;
    };
    let cases = [
        Case {
            name: "GLIDE off: detached notes, legato down and back",
            presses: &[
                (16, 0.0, 0.03),
                (4, 0.04, 0.07),
                (16, 0.08, 0.16),
                (4, 0.11, 0.14),
                (40, 0.17, 0.2),
            ],
            glide: None,
            contact_lead: 0.0,
            tstop: 0.25,
        },
        Case {
            name: "GLIDE 100K: legato up and down",
            presses: &[(4, 0.0, 0.3), (40, 0.0, 0.7), (4, 0.5, 0.9)],
            glide: Some(100e3),
            contact_lead: 0.0,
            tstop: 1.0,
        },
        Case {
            name: "GLIDE 1M: released half way, held, played again",
            presses: &[(0, 0.0, 0.05), (43, 0.1, 0.25), (20, 0.4, 0.5)],
            glide: Some(1e6),
            contact_lead: 0.0,
            tstop: 0.8,
        },
        Case {
            name: "the trigger contact 2 ms ahead of the pitch contact",
            presses: &[(20, 0.0, 0.05), (4, 0.1, 0.15), (40, 0.2, 0.25)],
            glide: None,
            contact_lead: 2e-3,
            tstop: 0.35,
        },
    ];
    let mut report = String::new();
    let mut fail = false;
    for c in &cases {
        let b = KeyboardBench {
            presses: c.presses.to_vec(),
            glide: c.glide,
            contact_lead: c.contact_lead,
            ..KeyboardBench::default()
        };
        let p = lab::transient(
            &spice,
            &work_dir(&format!(
                "keyboard-{}",
                c.name.split(':').next().unwrap_or("case").replace(' ', "_")
            )),
            &b,
            c.tstop,
            2e-6,
            Solver::default(),
        )
        .expect("tran");
        let n = (c.tstop * SR) as usize;
        let ng = ca72_spice::resample(p.vec("time"), p.vec("v(out)"), 0.0, SR, n + 1);
        let mut rt = Keyboard::new(KeyboardCircuit::default(), SR);
        rt.set_glide(c.glide.unwrap_or(0.0));
        let (k0, t0) = contacts(c, 0);
        rt.contacts(k0, t0);
        rt.settle().expect("settle");
        let start = std::time::Instant::now();
        let out: Vec<f64> = (0..n)
            .map(|i| {
                let (k, t) = contacts(c, i);
                rt.tick(k, t)
            })
            .collect();
        let cost = start.elapsed().as_secs_f64();
        // Sample i is the state at (i + 1) / SR. Everywhere, and where the circuit's
        // output is quasi-static (moving under 2 V/s).
        let (mut all, mut still) = ((0.0, 0.0f64), (0.0, 0.0f64));
        for i in 0..n {
            let d = out[i] - ng[i + 1];
            let t = (i + 1) as f64 / SR;
            if d.abs() > all.1.abs() {
                all = (t, d);
            }
            let slope = (ng[(i + 2).min(n)] - ng[i]).abs() * SR / 2.0;
            if slope < 2.0 && d.abs() > still.1.abs() {
                still = (t, d);
            }
        }
        fail |= all.1.abs() > 0.015 || still.1.abs() > 1e-4 || rt.failed > 0;
        report.push_str(&format!(
            "{}: worst {:+.3} mV at {:.5} s; quasi-static {:+.4} mV at {:.4} s; RT {:.3} s for {} s ({} subdivided, {} failed)\n",
            c.name,
            all.1 * 1e3,
            all.0,
            still.1 * 1e3,
            still.0,
            cost,
            c.tstop,
            rt.subdivided,
            rt.failed
        ));
    }
    eprintln!("{report}");
    // 15 mV anywhere: in the slews with GLIDE off (5 V/ms) 3 microseconds. 0.1 mV
    // (0.12 cent) wherever the output is quasi-static.
    assert!(
        !fail,
        "budget: 15 mV anywhere, 0.1 mV quasi-static, no failed steps\n{report}"
    );
}
