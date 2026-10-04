//! The circuit reference's numbers do not depend on the solver's settings: the frequency
//! of three notes across the oscillator's range under tighter tolerances, a finer time step
//! and the other integration method (docs/circuit/numerics.md).

use ca72_lab::bench::Solver;
use ca72_lab::vco::{self, Controls, Range, Trims, key_volts};
use ca72_lab::work_dir;
use ca72_spice::for_test;

#[test]
fn frequency_is_converged_in_the_solver_settings() {
    let Some(spice) = for_test("frequency_is_converged_in_the_solver_settings") else {
        return;
    };
    // Calibrated trims (Folkman 1973), so the notes are the ones the instrument plays.
    let trims = Trims {
        r11: 183.65,
        a8: 0.13518,
        octave_step: 0.29889,
        ..Trims::default()
    };
    let notes = [(Range::R32, 0u32), (Range::R8, 21), (Range::R2, 42)];
    let base = Solver::default();
    let variants = [
        (
            "reltol 1e-5",
            Solver {
                reltol: 1e-5,
                ..base
            },
        ),
        (
            "4x finer steps",
            Solver {
                steps_per_period: 1600.0,
                ..base
            },
        ),
        (
            "trapezoidal",
            Solver {
                method: "trap",
                ..base
            },
        ),
    ];
    for (range, key) in notes {
        let ctl = Controls {
            kbd: key_volts(f64::from(key)),
            range,
            tune: 7.5315,
            ..Controls::default()
        };
        let tag = format!("num-{range:?}-{key}");
        let f0 = vco::measure(&spice, &work_dir(&tag), &trims, &ctl, base, 6)
            .expect("runs")
            .hz;
        for (name, s) in variants {
            let f = vco::measure(
                &spice,
                &work_dir(&format!("{tag}-{name}")),
                &trims,
                &ctl,
                s,
                6,
            )
            .expect("runs")
            .hz;
            let cents = 1200.0 * (f / f0).log2();
            eprintln!("{range:?} key {key}: {f0:.5} Hz; {name}: {f:.5} Hz ({cents:+.4} cents)");
            assert!(
                cents.abs() < 0.05,
                "{range:?} key {key}, {name}: {cents:+.4} cents"
            );
        }
    }
}
