//! The filter's control node and exponential converter in real time against ngspice: the
//! ladder's current over the CUTOFF control's range, at three temperatures, from the
//! converter's circuit solution and from its real-time table; and over the rear FILTER
//! control jack's range (R51 100K from its source).

use ca72::expo::Input;
use ca72::vcf::{ExpoTable, FilterExpo};
use ca72_lab::bench::Solver;
use ca72_lab::vcf::{self as lab, VcfBench};
use ca72_lab::work_dir;
use ca72_spice::for_test;

/// The bench's control inputs as the real-time converter takes them: CUTOFF through R55,
/// the keyboard through R53 and R54, AMOUNT OF CONTOUR's wiper through R74.
fn inputs(b: &VcfBench, cutoff: f64) -> Vec<Input> {
    vec![
        Input {
            r: 200e3,
            v: cutoff,
        },
        Input { r: 300e3, v: b.kbd },
        Input { r: 150e3, v: b.kbd },
        Input { r: 47e3, v: 0.0 },
    ]
}

#[test]
fn ladder_current_matches_the_circuit() {
    let Some(spice) = for_test("ladder_current_matches_the_circuit") else {
        return;
    };
    let mut report = String::new();
    let mut fail = false;
    for (celsius, kbd) in [(15.0, 0.0), (25.0, 0.0), (40.0, 0.0), (25.0, 2.0)] {
        let solver = Solver {
            temp: celsius,
            ..Solver::default()
        };
        let b = VcfBench {
            kbd,
            ..VcfBench::default()
        };
        let sweep = lab::control_sweep(
            &spice,
            &work_dir(&format!("vcf-ctl-{celsius}-{kbd}")),
            &b,
            -10.0,
            10.0,
            1.0,
            solver,
        )
        .expect("dc");
        let expo = FilterExpo::default();
        let table = ExpoTable::new(expo, celsius);
        let mut line = format!("{celsius} C, keyboard {kbd} V:");
        for (v, i_ng) in &sweep {
            let ins = inputs(&b, *v);
            let i_solve = expo.current(&ins, celsius);
            let (i_in, g_node) = expo.node(&ins);
            let i_rt = table.current(i_in, g_node);
            let c_solve = 1200.0 * (i_solve / i_ng).log2();
            let c_rt = 1200.0 * (i_rt / i_ng).log2();
            // Q28 saturates above about 1.6 mA (R60's drop takes its collector down).
            let budget = if *i_ng < 1.6e-3 { 0.1 } else { 0.6 };
            fail |= c_solve.abs() > budget || c_rt.abs() > budget;
            line.push_str(&format!(
                " {v:+.0}V {:.2}uA {c_solve:+.2}c {c_rt:+.2}c;",
                i_ng * 1e6
            ));
        }
        report.push_str(&line);
        report.push('\n');
    }
    eprintln!("{report}");
    assert!(
        !fail,
        "budget: 0.1 cent, 0.6 cent where Q28 saturates (circuit solution, table)\n{report}"
    );
}

#[test]
fn the_filter_control_jack_matches_the_circuit() {
    let Some(spice) = for_test("the_filter_control_jack_matches_the_circuit") else {
        return;
    };
    let solver = Solver::default();
    let expo = FilterExpo::default();
    let table = ExpoTable::new(expo, 25.0);
    let mut report = String::from("FILTER control jack (CUTOFF at 0 V):");
    let mut fail = false;
    for v in [-8.0, -4.0, -1.0, 0.0, 1.0, 4.0, 8.0] {
        let b = VcfBench {
            ext_ctl: Some(v),
            ..VcfBench::default()
        };
        let i_ng = lab::ladder_current(&spice, &work_dir(&format!("vcf-jack-{v}")), &b, solver)
            .expect("op");
        let mut ins = inputs(&b, 0.0);
        ins.push(Input { r: 100e3, v });
        let i_solve = expo.current(&ins, 25.0);
        let (i_in, g_node) = expo.node(&ins);
        let i_rt = table.current(i_in, g_node);
        let c_solve = 1200.0 * (i_solve / i_ng).log2();
        let c_rt = 1200.0 * (i_rt / i_ng).log2();
        let budget = if i_ng < 1.6e-3 { 0.1 } else { 0.6 };
        fail |= c_solve.abs() > budget || c_rt.abs() > budget;
        report.push_str(&format!(
            " {v:+.0}V {:.2}uA {c_solve:+.2}c {c_rt:+.2}c;",
            i_ng * 1e6
        ));
    }
    eprintln!("{report}");
    assert!(!fail, "budget as the CUTOFF sweep's\n{report}");
}
