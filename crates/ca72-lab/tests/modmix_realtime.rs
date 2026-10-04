//! Board 3's modulation mix amplifier and the modulation line in real time against ngspice
//! (docs/circuit/board3.md): the amplifier's DC transfer from each end of MODULATION MIX
//! over its travel, and the line after R57 with the wheel and a load.

use ca72::modulation::{self, LineLoads, Modulation};
use ca72_lab::bench::Solver;
use ca72_lab::board3::{ModMixBench, modmix_netlist};
use ca72_lab::work_dir;
use ca72_spice::for_test;

#[test]
fn the_modulation_mix_matches_the_circuit() {
    let Some(spice) = for_test("the_modulation_mix_matches_the_circuit") else {
        return;
    };
    let s = Solver::default();
    let rt = Modulation::new().expect("the amplifier's operating points");
    let mut report = String::new();
    let (mut worst_off, mut worst_gain) = (0.0f64, 0.0f64);
    for mix in [0.0, 0.25, 0.5, 0.8, 1.0] {
        let b = ModMixBench {
            mix,
            wheel: 1e9,
            ..ModMixBench::default()
        };
        let op = |noise: f64, osc3: f64| {
            let net = modmix_netlist(&b, &format!("vn noise 0 {noise}\nvo osc3 0 {osc3}"), s);
            spice
                .run(&net, &["op"], &work_dir("modmix-op"))
                .expect("op")[0]
                .scalar("v(x1.amp)")
        };
        let (o, n, g) = (op(0.0, 0.0), op(1.0, 0.0), op(0.0, 1.0));
        let t = rt.mix(mix);
        let exact = modulation::transfer(mix).expect("transfer");
        worst_off = worst_off.max((t.offset - o).abs());
        let rel = |a: f64, b: f64| {
            if b.abs() > 1e-6 {
                (a / b - 1.0).abs()
            } else {
                (a - b).abs()
            }
        };
        worst_gain = worst_gain
            .max(rel(t.gain_noise, n - o))
            .max(rel(t.gain_osc3, g - o));
        report.push_str(&format!(
            "MODULATION MIX {mix}: ngspice offset {:+.2} mV, gains {:+.4} (noise) {:+.4} (oscillator 3); \
             the model {:+.2} mV, {:+.4}, {:+.4} (at a solved point: {:+.4}, {:+.4})\n",
            o * 1e3,
            n - o,
            g - o,
            t.offset * 1e3,
            t.gain_noise,
            t.gain_osc3,
            exact.gain_noise,
            exact.gain_osc3
        ));
    }
    // The line: oscillator 3's input at -4.3 V, the wheel at 1.2K, loaded by the MOD bus
    // (its Thevenin: 11.2K to +0.107 V).
    let g_bus = 3.0 / 51.1e3 + 1.0 / 33e3;
    let v_bus = (-5.0 * 3.0 / 51.1e3 + 10.0 / 33e3) / g_bus;
    let b = ModMixBench {
        mix: 0.0,
        wheel: modulation::mod_wheel_r(1.0),
        load: 1.0 / g_bus,
        load_v: v_bus,
    };
    let t = rt.mix(0.0);
    let loads = LineLoads {
        osc_mod: true,
        filter_mod: false,
        filter_node: 0.0,
    };
    // Oscillator 3 at its rectangle's low (-4.3 V: the output sources), and positive (the
    // output would have to sink: Q7 cuts off and R30 drives the line).
    let mut worst_line = 0.0f64;
    for v3 in [-4.3, -2.0, 0.0, 1.0, 1.8, 2.5] {
        let net = modmix_netlist(&b, &format!("vn noise 0 0\nvo osc3 0 {v3}"), s);
        let ng_line = spice
            .run(&net, &["op"], &work_dir("modmix-line"))
            .expect("op")[0]
            .scalar("v(mod)");
        let rt_line = Modulation::line(
            t.offset + t.gain_osc3 * v3,
            t.r_out,
            modulation::mod_wheel_r(1.0),
            &loads,
        );
        worst_line = worst_line.max((rt_line - ng_line).abs());
        report.push_str(&format!(
            "the line, the wheel fully forward and the MOD bus, oscillator 3 at {v3:+.1} V: ngspice {ng_line:+.4} V, the model {rt_line:+.4} V\n"
        ));
    }
    report.push_str(&format!(
        "the amplifier's output resistance: {:.1} ohm\n",
        t.r_out
    ));
    eprintln!("{report}");
    assert!(
        worst_off < 0.2e-3,
        "offset within {:.3} mV\n{report}",
        worst_off * 1e3
    );
    assert!(worst_gain < 5e-4, "gains within {worst_gain:.2e}\n{report}");
    // 20 mV: the line's crossover from Q7 sourcing to off is soft in the circuit.
    assert!(
        worst_line < 20e-3,
        "the line within {:.1} mV\n{report}",
        worst_line * 1e3
    );
}
