//! The filter converter's real-time table against its circuit solution over the control
//! node's range, and what a lookup costs.

use ca72::vcf::{EXPO_POINTS, EXPO_V_MIN, EXPO_V_STEP, ExpoTable, FilterExpo};

#[test]
fn table_matches_the_circuit_solution() {
    let expo = FilterExpo::default();
    let t0 = std::time::Instant::now();
    let table = ExpoTable::new(expo, 25.0);
    let build = t0.elapsed();
    let g_node = 1.0 / 2000.0;
    // Below the tail's saturation knee (the node at +0.085 V is CUTOFF at +8.5 V alone),
    // and through the knee and beyond.
    let mut worst = (0.0, 0.0f64);
    let mut worst_sat = (0.0, 0.0f64);
    let top = EXPO_V_MIN + EXPO_V_STEP * (EXPO_POINTS - 1) as f64;
    let n = 997;
    for k in 0..n {
        // Node voltages between the table's points, across its range.
        let v = EXPO_V_MIN + 0.01 + (top - EXPO_V_MIN - 0.02) * k as f64 / (n - 1) as f64;
        let (p, ib26) = expo.at_node(v, 25.0);
        // The Norton current that puts the node at v.
        let i_in = v * g_node - ib26;
        let i = table.current(i_in, g_node);
        let cents = 1200.0 * (i / p.i0).log2();
        let w = if v < 0.085 {
            &mut worst
        } else {
            &mut worst_sat
        };
        if cents.abs() > w.1.abs() {
            *w = (v, cents);
        }
    }
    let t1 = std::time::Instant::now();
    let mut acc = 0.0;
    for k in 0..100_000 {
        acc += table.current(1e-6 * (k % 200) as f64, g_node);
    }
    let per = t1.elapsed().as_secs_f64() / 1e5;
    eprintln!(
        "table built in {:.0} ms; worst {:+.4} cents at {:+.3} V, saturated {:+.4} cents at {:+.3} V; {:.0} ns per lookup ({acc:.3e})",
        build.as_secs_f64() * 1e3,
        worst.1,
        worst.0,
        worst_sat.1,
        worst_sat.0,
        per * 1e9
    );
    assert!(
        worst.1.abs() < 0.01,
        "worst {:+.4} cents at {:+.3} V",
        worst.1,
        worst.0
    );
    assert!(
        worst_sat.1.abs() < 0.5,
        "saturated: worst {:+.4} cents at {:+.3} V",
        worst_sat.1,
        worst_sat.0
    );
}
