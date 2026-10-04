//! The real-time contour generators against ngspice (docs/circuit/board2.md): whole
//! contours for a held key, a key released during the attack, the sustain's extremes and a
//! retrigger soon after a release, with the DECAY switch on and off; EXT. S-TRIG shorted
//! alone and pulsed during a held key. ngspice's waveforms
//! are compared at the model's rate: the flip-flops' resets put 30 ns spikes on the +9.3 V
//! rail and so on the outputs, which are not part of a contour.

use ca72::contour::{ContourCircuit, Contours, Controls, Panel};
use ca72_lab::bench::Solver;
use ca72_lab::contour::{self as lab, ContourBench, ContourControls};
use ca72_lab::work_dir;
use ca72_spice::for_test;

fn panel(b: &ContourBench) -> Panel {
    let c = |x: ContourControls| Controls {
        attack: x.attack,
        decay: x.decay,
        sustain: x.sustain,
    };
    Panel {
        filter: c(b.filter),
        loudness: c(b.loudness),
        decay_on: b.decay_on,
        s_trig: false,
    }
}

/// A scenario: the bench, when the key is held (several presses for a retrigger) and when
/// EXT. S-TRIG is shorted.
struct Case {
    name: &'static str,
    bench: ContourBench,
    presses: &'static [(f64, f64)],
    s_trig: &'static [(f64, f64)],
    tstop: f64,
}

#[test]
fn contours_match_the_circuit() {
    let Some(spice) = for_test("contours_match_the_circuit") else {
        return;
    };
    let quick = ContourControls {
        attack: 2e3,
        decay: 20e3,
        sustain: 0.5,
    };
    let cases = [
        Case {
            name: "held, DECAY on",
            bench: ContourBench {
                filter: quick,
                loudness: quick,
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.6)],
            s_trig: &[],
            tstop: 1.4,
        },
        Case {
            name: "held, DECAY off",
            bench: ContourBench {
                filter: quick,
                loudness: quick,
                decay_on: false,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.6)],
            s_trig: &[],
            tstop: 1.0,
        },
        Case {
            name: "released during the attack",
            bench: ContourBench {
                filter: ContourControls {
                    attack: 50e3,
                    decay: 50e3,
                    sustain: 0.3,
                },
                loudness: ContourControls {
                    attack: 20e3,
                    decay: 5e3,
                    sustain: 0.8,
                },
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.25)],
            s_trig: &[],
            tstop: 1.5,
        },
        Case {
            name: "sustain at 0 and 10",
            bench: ContourBench {
                filter: ContourControls {
                    attack: 500.0,
                    decay: 10e3,
                    sustain: 0.0,
                },
                loudness: ContourControls {
                    attack: 500.0,
                    decay: 10e3,
                    sustain: 1.0,
                },
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.5)],
            s_trig: &[],
            tstop: 1.0,
        },
        Case {
            name: "retriggered 30 ms after a release",
            bench: ContourBench {
                filter: quick,
                loudness: quick,
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.3), (0.33, 0.6)],
            s_trig: &[],
            tstop: 1.0,
        },
        Case {
            name: "S-TRIG alone",
            bench: ContourBench {
                filter: quick,
                loudness: quick,
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[],
            s_trig: &[(0.05, 0.6)],
            tstop: 1.0,
        },
        Case {
            name: "S-TRIG pulsed during a held key",
            bench: ContourBench {
                filter: quick,
                loudness: quick,
                decay_on: true,
                ..ContourBench::default()
            },
            presses: &[(0.05, 0.8)],
            s_trig: &[(0.3, 0.305)],
            tstop: 1.2,
        },
    ];
    let sr = 48_000.0;
    let mut report = String::new();
    let mut fail = false;
    for case in &cases {
        // ngspice: the key's source from the presses.
        let mut b = case.bench;
        let first = case
            .presses
            .first()
            .copied()
            .unwrap_or((2.0 * case.tstop, 3.0 * case.tstop));
        b.key_down = first.0;
        b.key_up = first.1;
        let pwl = |spans: &[(f64, f64)]| {
            let mut w = String::from("pwl(0 0");
            for (d, u) in spans {
                w.push_str(&format!(" {d} 0 {} 1 {u} 1 {} 0", d + 1e-6, u + 1e-6));
            }
            w.push(')');
            w
        };
        let s_trig_src = (!case.s_trig.is_empty()).then(|| pwl(case.s_trig));
        let p = lab::transient_with_inputs(
            &spice,
            &work_dir(&format!("contour-case-{}", case.name.replace(' ', "_"))),
            &b,
            &pwl(case.presses),
            s_trig_src.as_deref(),
            case.tstop,
            50e-6,
            Solver::default(),
        )
        .expect("tran");
        // ngspice at the model's rate (the flip-flops' 30 ns rail spikes are not contour).
        let n = (case.tstop * sr) as usize;
        let rs = |name: &str| ca72_spice::resample(p.vec("time"), p.vec(name), 0.0, sr, n);
        let (f_ng, l_ng, vt_ng) = (rs("fout"), rs("lout"), rs("vtrig"));
        let t: Vec<f64> = (0..n).map(|i| i as f64 / sr).collect();
        // The real-time model at 48 kHz and at the voice's 24 kHz (its samples
        // interpolated onto the 48 kHz grid).
        for rate in [48_000.0, 24_000.0] {
            let pn = panel(&b);
            let mut rt = Contours::new(ContourCircuit::default(), rate);
            rt.settle(&pn);
            let held = |t: f64| case.presses.iter().any(|&(d, u)| t >= d && t < u);
            let m = (case.tstop * rate) as usize + 2;
            let (mut fr, mut lr, mut vr) = (
                Vec::with_capacity(m),
                Vec::with_capacity(m),
                Vec::with_capacity(m),
            );
            let shorted = |t: f64| case.s_trig.iter().any(|&(d, u)| t >= d && t < u);
            for i in 0..m {
                let t = i as f64 / rate;
                let o = rt.tick(
                    held(t),
                    &Panel {
                        s_trig: shorted(t),
                        ..pn
                    },
                );
                fr.push(o.filter);
                lr.push(o.loudness);
                vr.push(o.vtrig);
            }
            let grid = |y: &[f64]| -> Vec<f64> {
                (0..n)
                    .map(|k| {
                        let x = k as f64 / sr * rate;
                        let i = (x.floor() as usize).min(y.len() - 2);
                        y[i] + (y[i + 1] - y[i]) * (x - i as f64)
                    })
                    .collect()
            };
            let (f_rt, l_rt, vt_rt) = (grid(&fr), grid(&lr), grid(&vr));
            // The trigger's edges: V-trig crossing half the rail, each press and release.
            let edges = |y: &[f64]| -> Vec<f64> {
                (1..y.len())
                    .filter(|&k| (y[k - 1] < 4.65) != (y[k] < 4.65))
                    .map(|k| (k as f64 - 1.0 + (4.65 - y[k - 1]) / (y[k] - y[k - 1])) / sr)
                    .collect()
            };
            let (e_ng, e_rt) = (edges(&vt_ng), edges(&vt_rt));
            let edge_err = if e_ng.len() == e_rt.len() {
                e_ng.iter().zip(&e_rt).fold(
                    0.0f64,
                    |a, (x, y)| if (y - x).abs() > a.abs() { y - x } else { a },
                )
            } else {
                f64::INFINITY
            };
            // At 24 kHz a sample is 42 us: the edges and the fastest attack's slope get a
            // sample's worth more.
            let (edge_budget, budget) = if rate > 30e3 {
                (0.3e-3, 0.08)
            } else {
                (0.35e-3, 0.12)
            };
            fail |= edge_err.abs() > edge_budget;
            let mut line = format!(
                "{} at {} kHz: trigger edges within {:+.3} ms;",
                case.name,
                rate / 1e3,
                edge_err * 1e3
            );
            for (name, ng, rtv) in [("filter", &f_ng, &f_rt), ("loudness", &l_ng, &l_rt)] {
                // The largest difference and the peaks.
                let mut worst = (0.0, 0.0f64);
                for k in 0..n {
                    let d = rtv[k] - ng[k];
                    if d.abs() > worst.1.abs() {
                        worst = (t[k], d);
                    }
                }
                let peak_ng = ng.iter().fold(f64::MIN, |a, &x| a.max(x));
                let peak_rt = rtv.iter().fold(f64::MIN, |a, &x| a.max(x));
                fail |= worst.1.abs() > budget || (peak_rt - peak_ng).abs() > 0.03;
                line.push_str(&format!(
                    " {name}: worst {:+.1} mV at {:.4} s, peak {:.3} V ({:+.1} mV);",
                    worst.1 * 1e3,
                    worst.0,
                    peak_ng,
                    (peak_rt - peak_ng) * 1e3
                ));
            }
            report.push_str(&line);
            report.push('\n');
        }
    }
    eprintln!("{report}");
    // 80 mV: the fastest attack runs up to 72 mV ahead mid-attack, where the circuit's
    // +9.3 V rail sags 75 mV under the charging current and the model's is ideal (A16).
    assert!(
        !fail,
        "budget: 80 mV (120 mV at 24 kHz) anywhere, peaks 30 mV, trigger edges 0.3 ms (0.35 ms)\n{report}"
    );
}
