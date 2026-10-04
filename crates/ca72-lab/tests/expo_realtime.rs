//! The real-time exponential converter against ngspice's operating points, over the
//! keyboard and every range, at two ramp voltages and three temperatures; and over the rear
//! oscillator control jack's range (its source on the external bus).

use ca72::expo::{ExpoCircuit, Input, OSC1_INPUT_R};
use ca72_lab::bench::Solver;
use ca72_lab::vco::{self, Controls, Range, Trims, key_volts};
use ca72_lab::work_dir;
use ca72_spice::for_test;

/// Constant offsets do not matter: the real-time model is calibrated by the same factory
/// procedure as the circuit (they come from microvolt-level differences such as the -5 V
/// reference's own error). What must agree is how the current varies: over the keyboard,
/// every range, the ramp and temperature. After removing one constant, the real-time
/// converter agrees with ngspice within 0.5 cent on 32'..2' and 1.5 cent on LO (whose
/// switch position is itself assumed, assumptions.md A3).
#[test]
fn realtime_converter_follows_ngspice_over_the_range_and_temperature() {
    let Some(spice) = for_test("realtime_converter_follows_ngspice") else {
        return;
    };
    let trims = Trims {
        r11: 183.65,
        a8: 0.13518,
        octave_step: 0.29889,
        ..Trims::default()
    };
    let circuit = ExpoCircuit {
        r11: trims.r11,
        a8: trims.a8,
        tc20: trims.tc20,
        ..ExpoCircuit::default()
    };
    let mut rows = Vec::new();
    for celsius in [15.0, 25.0, 40.0] {
        for range in [Range::Lo, Range::R32, Range::R8, Range::R2] {
            for key in [0u32, 13, 26, 43] {
                for v_ramp in [-0.2, -3.8] {
                    let ctl = Controls {
                        kbd: key_volts(f64::from(key)),
                        range,
                        tune: 7.5315,
                        ..Controls::default()
                    };
                    let solver = Solver {
                        temp: celsius,
                        ..Solver::default()
                    };
                    let tag = format!("expo-{celsius}-{range:?}-{key}-{v_ramp}");
                    let spice_i =
                        vco::expo_current(&spice, &work_dir(&tag), &trims, &ctl, v_ramp, solver)
                            .expect("op");
                    let v = [
                        ctl.bend,
                        ctl.tune,
                        ctl.kbd,
                        ctl.modulation,
                        ctl.ext,
                        vco::range_tap(&trims, range),
                    ];
                    let inputs: Vec<Input> = OSC1_INPUT_R
                        .iter()
                        .zip(v)
                        .map(|(&r, v)| Input { r, v })
                        .collect();
                    let rt = circuit.collector_current(&inputs, v_ramp, celsius);
                    rows.push((celsius, range, key, v_ramp, 1200.0 * (rt / spice_i).log2()));
                }
            }
        }
    }
    let musical: Vec<f64> = rows
        .iter()
        .filter(|r| r.0 == 25.0 && r.1 != Range::Lo)
        .map(|r| r.4)
        .collect();
    let c0 = musical.iter().sum::<f64>() / musical.len() as f64;
    let mut worst = (0.0f64, 0.0f64);
    for (celsius, range, key, v_ramp, cents) in &rows {
        let d = cents - c0;
        eprintln!("{celsius} C {range:?} key {key} ramp {v_ramp}: {d:+.3} cents");
        if *range == Range::Lo {
            worst.1 = worst.1.max(d.abs());
        } else {
            worst.0 = worst.0.max(d.abs());
        }
    }
    eprintln!(
        "constant {c0:+.3} cents; worst 32'..2' {:.3}, LO {:.3}",
        worst.0, worst.1
    );
    assert!(worst.0 < 0.5 && worst.1 < 1.5, "{worst:?}");
}

/// The oscillator control jack: a source on the external bus (8A) reaches each converter
/// through its 51.1K; the current it gives against ngspice's, one constant removed as
/// above, over -4..+4 V at two ramp voltages.
#[test]
fn the_oscillator_control_jack_matches_the_circuit() {
    let Some(spice) = for_test("the_oscillator_control_jack_matches_the_circuit") else {
        return;
    };
    let trims = Trims {
        r11: 183.65,
        a8: 0.13518,
        octave_step: 0.29889,
        ..Trims::default()
    };
    let circuit = ExpoCircuit {
        r11: trims.r11,
        a8: trims.a8,
        tc20: trims.tc20,
        ..ExpoCircuit::default()
    };
    let mut rows = Vec::new();
    for v_ramp in [-0.2, -3.8] {
        for ext in [-4.0, -2.0, -1.0, 0.0, 1.0, 2.0, 4.0] {
            let ctl = Controls {
                kbd: key_volts(13.0),
                range: Range::R8,
                tune: 7.5315,
                ext,
                ..Controls::default()
            };
            let tag = format!("expo-jack-{ext}-{v_ramp}");
            let spice_i = vco::expo_current(
                &spice,
                &work_dir(&tag),
                &trims,
                &ctl,
                v_ramp,
                Solver::default(),
            )
            .expect("op");
            let v = [
                ctl.bend,
                ctl.tune,
                ctl.kbd,
                ctl.modulation,
                ctl.ext,
                vco::range_tap(&trims, Range::R8),
            ];
            let inputs: Vec<Input> = OSC1_INPUT_R
                .iter()
                .zip(v)
                .map(|(&r, v)| Input { r, v })
                .collect();
            let rt = circuit.collector_current(&inputs, v_ramp, 25.0);
            rows.push((v_ramp, ext, spice_i, 1200.0 * (rt / spice_i).log2()));
        }
    }
    let c0 = rows.iter().map(|r| r.3).sum::<f64>() / rows.len() as f64;
    let mut worst = 0.0f64;
    let mut report = String::new();
    for (v_ramp, ext, i, cents) in &rows {
        let d = cents - c0;
        worst = worst.max(d.abs());
        report.push_str(&format!(
            "ramp {v_ramp} V, jack {ext:+} V: {:.3} uA, {d:+.3} cents\n",
            i * 1e6
        ));
    }
    // The jack's scale: octaves a volt, from ngspice's currents.
    let i_at = |e: f64| {
        rows.iter()
            .find(|r| r.0 == -0.2 && r.1 == e)
            .map(|r| r.2)
            .unwrap_or(f64::NAN)
    };
    let per_volt = (i_at(4.0) / i_at(-4.0)).log2() / 8.0;
    eprintln!(
        "{report}constant {c0:+.3} cents; worst {worst:.3} cents; the jack's scale {per_volt:.4} octaves a volt"
    );
    assert!(worst < 0.5, "budget 0.5 cent (as the keyboard's)\n{report}");
}
