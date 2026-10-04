//! The noise generator's loads changed in place (a VOLUME turning, the NOISE switch) against
//! the circuit solved afresh with the new loads.

use ca72::noise::{LOAD_EVERY, Noise, NoiseCircuit, NoiseLoads};

#[test]
fn changed_loads_match_the_circuit_solved_with_them() {
    let rate = 48_000.0;
    let a = NoiseLoads {
        white: 7.6e3,
        pink: 40e3,
        red: f64::INFINITY,
    };
    // The NOISE switch at PINK with the VOLUME at 3.
    let b = NoiseLoads {
        white: f64::INFINITY,
        pink: 24e3 + 18e3,
        red: 35e3,
    };
    let mut moved = Noise::new(NoiseCircuit::default(), a, rate, 7).expect("a");
    moved.set_density(1e-6);
    for _ in 0..LOAD_EVERY {
        moved.tick();
    }
    let before = moved.tick();
    moved.set_loads(b);
    // Applied on the next tick; the state carries over (no jump to rest).
    let after = moved.tick();
    let fresh = Noise::new(NoiseCircuit::default(), b, rate, 7).expect("b");
    let (m, _, out) = moved.model_of();
    let (f, _, fout) = fresh.model_of();
    assert_eq!(out, fout);
    let mut worst = 0.0f64;
    for k in 0..3 {
        for hz in [1.0, 10.0, 100.0, 1e3, 1e4, 2e4] {
            let (p, q) = (m.response(out[k], hz), f.response(fout[k], hz));
            let d =
                ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt() / (q.0.hypot(q.1)).max(1e-30);
            worst = worst.max(d);
        }
    }
    eprintln!(
        "loads changed in place against the circuit solved with them: within {worst:.1e} (relative); \
         pink before {:.3e}, after {:.3e}",
        before.pink, after.pink
    );
    assert!(worst < 1e-6, "{worst:.1e}");
    assert!(after.pink != 0.0 && after.red.is_finite());
}

/// Potato's model (the transistors without RB, RE and RC) against the circuit's, at each
/// output from 10 Hz to 20 kHz, under two sets of loads: within 0.01 dB and 0.1 degree
/// (Potato; RE folded into the emitter resistors, RB and RC left out). Loads changed
/// while in Potato reach both models, and a switch back carries the nodes' state (no jump
/// to rest).
#[test]
fn potatos_noise_model_follows_the_circuits() {
    use ca72::voice::Quality;
    let rate = 48_000.0;
    let a = NoiseLoads {
        white: 7.6e3,
        pink: 40e3,
        red: f64::INFINITY,
    };
    let b = NoiseLoads {
        white: f64::INFINITY,
        pink: 24e3 + 18e3,
        red: 35e3,
    };
    let (mut db, mut deg) = (0.0f64, 0.0f64);
    let mut compare = |full: &Noise, plain: &Noise| {
        let (m, _, out) = full.model_of();
        let (p, _, pout) = plain.model_of();
        for k in 0..3 {
            for hz in [10.0, 100.0, 1e3, 5e3, 1e4, 2e4] {
                let (x, y) = (m.response(out[k], hz), p.response(pout[k], hz));
                let (mx, my) = (x.0.hypot(x.1), y.0.hypot(y.1));
                if mx < 1e-9 {
                    continue;
                }
                db = db.max((20.0 * (my / mx).log10()).abs());
                let dp = (y.1.atan2(y.0) - x.1.atan2(x.0)).to_degrees();
                deg = deg.max(((dp + 180.0).rem_euclid(360.0) - 180.0).abs());
            }
        }
    };
    let full = Noise::new(NoiseCircuit::default(), a, rate, 7).expect("a");
    let mut plain = full.clone();
    plain.set_quality(Quality::Potato);
    compare(&full, &plain);
    // New loads while in Potato.
    plain.set_density(1e-6);
    plain.set_loads(b);
    for _ in 0..8 * LOAD_EVERY + 1 {
        plain.tick();
    }
    let full_b = Noise::new(NoiseCircuit::default(), b, rate, 7).expect("b");
    compare(&full_b, &plain);
    eprintln!("Potato's noise model against the circuit's: within {db:.2e} dB, {deg:.2e} degrees");
    assert!(db < 0.01 && deg < 0.1, "{db} dB, {deg} degrees");
    // Back to the circuit: the loads it took in Potato, and the nodes' state carried.
    let before = plain.tick();
    plain.set_quality(Quality::NoCompromises);
    let after = plain.tick();
    let (m, _, out) = plain.model_of();
    let (f, _, fout) = full_b.model_of();
    for k in 0..3 {
        let (p, q) = (m.response(out[k], 1e3), f.response(fout[k], 1e3));
        let d = (p.0 - q.0).hypot(p.1 - q.1) / q.0.hypot(q.1).max(1e-30);
        assert!(d < 1e-9, "output {k}: {d:e}");
    }
    eprintln!(
        "pink before the switch {:.3e}, after {:.3e}",
        before.pink, after.pink
    );
    assert!(after.pink != 0.0 && after.pink.is_finite());
}
