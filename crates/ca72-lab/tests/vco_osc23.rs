//! Oscillators 2 and 3 (board1.md, "Oscillators 2 and 3"): their FREQUENCY controls and
//! OSC. 3 CONTROL in ngspice against the service manual's figures, then the real-time
//! converter against ngspice after each is tuned by the same procedure.

use ca72::tuning::{
    self as rt_tuning, KEY_STEP as RT_KEY_STEP, Osc as RtOsc, Tuning, measure_hz, osc_drive,
};
use ca72::vco::Vco;
use ca72_lab::bench::Solver;
use ca72_lab::vco::{
    self as lab, Controls, FREQ_CENTRE, HIGH_A, LOW_A, Osc, Procedure, Range, Trims,
};
use ca72_lab::work_dir;
use ca72_spice::for_test;

fn rt_range(r: Range) -> rt_tuning::Range {
    match r {
        Range::Lo => rt_tuning::Range::Lo,
        Range::R32 => rt_tuning::Range::R32,
        Range::R16 => rt_tuning::Range::R16,
        Range::R8 => rt_tuning::Range::R8,
        Range::R4 => rt_tuning::Range::R4,
        Range::R2 => rt_tuning::Range::R2,
    }
}

fn rt_osc(o: Osc) -> RtOsc {
    match o {
        Osc::One => RtOsc::One,
        Osc::Two { freq } => RtOsc::Two { freq },
        Osc::Three { freq, control } => RtOsc::Three { freq, control },
    }
}

fn semis(a: f64, b: f64) -> f64 {
    12.0 * (b / a).log2()
}

#[test]
fn oscillators_2_and_3_match_the_manual_and_the_circuit() {
    let Some(spice) = for_test("oscillators_2_and_3_match_the_manual_and_the_circuit") else {
        return;
    };
    assert!(
        (lab::KEY_STEP - RT_KEY_STEP).abs() < 1e-12,
        "the benches' key step"
    );
    // IC8's effective input offset in the 741 macromodel, at its operating point.
    {
        let ctl = Controls {
            osc: Osc::Three {
                freq: FREQ_CENTRE,
                control: true,
            },
            kbd: LOW_A,
            range: Range::R8,
            ..Controls::default()
        };
        let net = lab::netlist(
            &Trims::default(),
            &ctl,
            ca72_lab::bench::Supplies::default(),
            Solver::default(),
        );
        let op = spice
            .run(
                &format!("{net}vhold ramp 0 -2\n"),
                &["op"],
                &work_dir("osc23-ic8"),
            )
            .expect("op");
        let vos = op[0].scalar("v(x8.pos8)") - op[0].scalar("v(x8.inv)");
        let ic8 = op[0].scalar("v(x8.out)");
        let rt_ic8 = rt_tuning::osc3_control(FREQ_CENTRE, true);
        let (m5, m4) = (op[0].scalar("v(m5)"), op[0].scalar("v(m4)"));
        let summer = op[0].scalar("v(sum)") - op[0].scalar("v(pos)");
        eprintln!(
            "IC8: offset {:.4} mV (the model's {:.4}); output {ic8:.6} V (the model's {rt_ic8:.6}); \
             the -5 V line {m5:.8} V, the -4V line {m4:.6} V, IC6's offset {:+.2} uV",
            vos * 1e3,
            rt_tuning::IC8_OFFSET * 1e3,
            summer * 1e6
        );
        assert!(
            (vos - rt_tuning::IC8_OFFSET).abs() < 5e-6,
            "IC8's offset {vos}"
        );
        assert!(
            (ic8 - rt_ic8).abs() < 50e-6,
            "IC8's output {ic8} against {rt_ic8}"
        );
        assert!(
            (m5 - ca72::expo::M5_LINE).abs() < 5e-9,
            "the -5 V line {m5}"
        );
        assert!((m4 - ca72::expo::M4_LINE).abs() < 5e-6, "the -4V line {m4}");
        assert!(
            (summer - ca72::expo::SUMMER_OFFSET).abs() < 0.5e-6,
            "IC6's offset {summer}"
        );
    }
    let s = Solver::default();
    let sr = 48_000.0;
    // Oscillator 1 first (TUNE and the octave step are shared), then 2 and 3.
    let c1 = lab::calibrate(
        &spice,
        &work_dir("osc23-cal1"),
        Trims::default(),
        s,
        Procedure::Folkman1973,
    )
    .expect("osc 1");
    let mut vco = Vco::new(sr, 4);
    let (t1, _) = rt_tuning::folkman_1973(&mut vco, sr, Tuning::default());
    let mut report = String::new();
    // Below 4.5 kHz, and above: the core's model is fitted from 0.7 Hz to 4.2 kHz (board1.md).
    let mut worst = (String::new(), 0.0f64);
    let mut worst_high = (String::new(), 0.0f64);
    let mut fail = Vec::new();
    for (name, centre) in [
        ("oscillator 2", Osc::Two { freq: FREQ_CENTRE }),
        (
            "oscillator 3",
            Osc::Three {
                freq: FREQ_CENTRE,
                control: true,
            },
        ),
    ] {
        let cal = lab::calibrate_osc(
            &spice,
            &work_dir(&format!("osc23-cal-{name}")),
            c1.trims,
            s,
            centre,
            c1.tune,
        )
        .expect("calibrate");
        let mut v = Vco::new(sr, 4);
        let (t, _) = rt_tuning::folkman_1973_osc(&mut v, sr, t1, rt_osc(centre), &|k| {
            f64::from(k) * RT_KEY_STEP
        });
        report.push_str(&format!(
            "{name}: ngspice R11' {:.1} ohm, R8' {:.4}; real time {:.1} ohm, {:.4}\n",
            cal.trims.r11, cal.trims.a8, t.r11, t.a8
        ));
        let ng = |osc: Osc, kbd: f64, range: Range| {
            let ctl = Controls {
                osc,
                kbd,
                range,
                tune: c1.tune,
                ..Controls::default()
            };
            lab::measure(&spice, &work_dir("osc23-m"), &cal.trims, &ctl, s, 4)
                .expect("measure")
                .hz
        };
        let rt = |v: &mut Vco, osc: Osc, kbd: f64, range: Range| {
            v.expo.r11 = t.r11;
            v.expo.a8 = t.a8;
            let i = osc_drive(rt_osc(osc), &t, kbd, rt_range(range)).apply(&mut v.expo);
            measure_hz(v, sr, i, 6)
        };
        let settings: Vec<Osc> = match centre {
            Osc::Two { .. } => [0.0, 0.5, 1.0]
                .iter()
                .map(|&f| Osc::Two { freq: f })
                .collect(),
            _ => [true, false]
                .iter()
                .flat_map(|&c| {
                    [0.0, 0.5, 1.0].map(move |f| Osc::Three {
                        freq: f,
                        control: c,
                    })
                })
                .collect(),
        };
        for osc in settings {
            let mut line = format!("  {osc:?}:");
            let mut span = Vec::new();
            for (range, key) in [
                (Range::R8, LOW_A),
                (Range::R8, HIGH_A),
                (Range::R32, LOW_A),
                (Range::R2, HIGH_A),
            ] {
                let (hn, hr) = (ng(osc, key, range), rt(&mut v, osc, key, range));
                let c = 1200.0 * (hr / hn).log2();
                let w = if hn < 4500.0 {
                    &mut worst
                } else {
                    &mut worst_high
                };
                if c.abs() > w.1.abs() {
                    *w = (
                        format!("{name} {osc:?} {range:?} key {:.0}", key / RT_KEY_STEP),
                        c,
                    );
                }
                line.push_str(&format!(
                    " {range:?} {:.0}: {hn:.3} Hz ({c:+.3} c);",
                    key / RT_KEY_STEP
                ));
                span.push(hn);
            }
            report.push_str(&line);
            report.push('\n');
        }
        // The manual's figures, from ngspice.
        let at = |osc: Osc| ng(osc, LOW_A, Range::R8);
        match centre {
            Osc::Two { .. } => {
                let travel = semis(at(Osc::Two { freq: 0.0 }), at(Osc::Two { freq: 1.0 }));
                report.push_str(&format!(
                    "  FREQUENCY's travel: {travel:.2} semitones (5.35: 14-17)\n"
                ));
                if !(14.0..=17.0).contains(&travel) {
                    fail.push(format!("{name}: FREQUENCY travel {travel:.2} semitones"));
                }
            }
            _ => {
                let on = semis(
                    at(Osc::Three {
                        freq: 0.0,
                        control: true,
                    }),
                    at(Osc::Three {
                        freq: 1.0,
                        control: true,
                    }),
                );
                let off = semis(
                    at(Osc::Three {
                        freq: 0.0,
                        control: false,
                    }),
                    at(Osc::Three {
                        freq: 1.0,
                        control: false,
                    }),
                );
                let keys = semis(
                    ng(
                        Osc::Three {
                            freq: 0.5,
                            control: false,
                        },
                        LOW_A,
                        Range::R8,
                    ),
                    ng(
                        Osc::Three {
                            freq: 0.5,
                            control: false,
                        },
                        HIGH_A,
                        Range::R8,
                    ),
                );
                // 5.36: CONTROL off, LO, FREQUENCY at minimum: a click every 2 to 5 s; LO's
                // top above 32''s bottom.
                let lo_min = ng(
                    Osc::Three {
                        freq: 0.0,
                        control: false,
                    },
                    LOW_A,
                    Range::Lo,
                );
                let lo_max = ng(
                    Osc::Three {
                        freq: 1.0,
                        control: false,
                    },
                    LOW_A,
                    Range::Lo,
                );
                let r32_min = ng(
                    Osc::Three {
                        freq: 0.0,
                        control: false,
                    },
                    LOW_A,
                    Range::R32,
                );
                report.push_str(&format!(
                    "  FREQUENCY's travel: {on:.2} semitones with CONTROL on (5.35: 14-17), {off:.1} off (2.3, later board: +-3 octaves); \
                     keys with CONTROL off: {keys:+.3} semitones; LO at minimum {lo_min:.3} Hz ({:.1} s a click; 5.36: 2-5 s), \
                     LO at maximum {lo_max:.2} Hz against 32' at minimum {r32_min:.2} Hz (5.36: they overlap)\n",
                    1.0 / lo_min
                ));
                if !(14.0..=17.0).contains(&on) {
                    fail.push(format!(
                        "{name}: FREQUENCY travel {on:.2} semitones with CONTROL on"
                    ));
                }
                if !(60.0..=80.0).contains(&off) || keys.abs() > 0.01 {
                    fail.push(format!(
                        "{name}: CONTROL off: {off:.1} semitones, keys move {keys:+.3}"
                    ));
                }
                if !(2.0..=5.0).contains(&(1.0 / lo_min)) || lo_max < r32_min {
                    fail.push(format!("{name}: LO at minimum {lo_min:.3} Hz, LO max {lo_max:.2} Hz, 32' min {r32_min:.2} Hz"));
                }
            }
        }
    }
    eprintln!(
        "{report}worst real time against ngspice: {:+.3} cents below 4.5 kHz ({}), {:+.3} above ({})",
        worst.1, worst.0, worst_high.1, worst_high.0
    );
    assert!(fail.is_empty(), "{fail:?}\n{report}");
    // Below 4.5 kHz as for oscillator 1 give or take the macromodels' offsets with OSC. 3
    // CONTROL off (untuned there); above, the core model's limit (-1.2 cents at 5.6 kHz).
    assert!(
        worst.1.abs() < 0.3,
        "worst {:+.3} cents: {}\n{report}",
        worst.1,
        worst.0
    );
    assert!(
        worst_high.1.abs() < 1.5,
        "above 4.5 kHz: {:+.3} cents: {}\n{report}",
        worst_high.1,
        worst_high.0
    );
}
