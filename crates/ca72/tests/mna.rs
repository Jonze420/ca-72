//! The nodal solver (`mna.rs`): its Jacobian against finite differences, and its solutions
//! against closed forms.

use ca72::contour::{D1N34A, Q2N3392};
use ca72::devices::{series_junction, vt};
use ca72::mna::{Circuit, Depletion, GND, J2N4303, Part};
use ca72::vca::Q2N4058;

const CAP: Depletion = Depletion {
    cj0: 20e-12,
    vj: 1.0,
    m: 0.33,
    fc: 0.5,
};

/// A circuit with every part: NPN and PNP, a JFET (conducting both ways across the
/// cases), a diode with charges, resistors and capacitors.
fn every_part() -> Circuit {
    // Nodes: 0 ground, 1 +10 V, 2 -10 V held; 3..=10 solved.
    let mut c = Circuit::new(11, 3);
    c.set(1, 10.0);
    c.set(2, -10.0);
    let npn = Q2N3392.at(25.0);
    let pnp = Q2N4058.at(25.0);
    c.add(Part::Resistor {
        a: 1,
        b: 3,
        r: 6.2e3,
    });
    c.add(Part::Bjt {
        c: 3,
        b: 4,
        e: 5,
        m: npn,
        vaf: Q2N3392.vaf,
        pnp: false,
    });
    c.add(Part::Resistor {
        a: 5,
        b: 2,
        r: 43e3,
    });
    c.add(Part::Bjt {
        c: 6,
        b: 10,
        e: 1,
        m: pnp,
        vaf: Q2N4058.vaf,
        pnp: true,
    });
    c.add(Part::Resistor {
        a: 3,
        b: 10,
        r: 1e3,
    });
    c.add(Part::Resistor {
        a: 6,
        b: 2,
        r: 1.5e3,
    });
    c.add(Part::Jfet {
        d: 6,
        g: 7,
        s: 8,
        m: J2N4303,
    });
    c.add(Part::Capacitor {
        a: 8,
        b: GND,
        c: 1e-6,
    });
    c.add(Part::Diode {
        a: 7,
        k: 9,
        is: 7e-9,
        nvt: 1.9 * vt(25.0),
        cap: CAP,
        tt: 3e-6,
    });
    c.add(Part::Resistor {
        a: 9,
        b: 2,
        r: 100e3,
    });
    c.add(Part::Resistor {
        a: 4,
        b: GND,
        r: 10e3,
    });
    c.add(Part::Junction {
        a: 7,
        k: 8,
        cap: CAP,
    });
    c
}

#[test]
fn jacobian_matches_finite_differences() {
    // Nodes 3..=10: NPN collector, base, emitter; PNP collector (the JFET's drain); the
    // JFET's gate (the diode's anode); its source; the diode's cathode; the PNP's base.
    let cases: [[f64; 8]; 4] = [
        // Both transistors forward active; the JFET's drain above its source.
        [7.0, 0.6, 0.0, 3.0, -1.0, -2.0, -1.3, 9.35],
        // The JFET reversed (its source above its drain); the diode forward.
        [8.0, 0.65, 0.02, -2.0, 0.5, 1.0, 0.2, 9.3],
        // The NPN saturated, the PNP off, the diode reversed.
        [0.3, 0.75, 0.1, -9.0, -3.0, -3.2, -2.8, 10.2],
        // The PNP saturated.
        [7.0, 0.62, 0.0, 9.8, 1.0, 2.0, 0.7, 9.2],
    ];
    let mut worst = (0.0f64, String::new());
    for (ci, v) in cases.iter().enumerate() {
        for h in [None, Some(1e-5)] {
            let mut c = every_part();
            for (k, &x) in v.iter().enumerate() {
                c.set(3 + k, x);
            }
            let (f0, jac) = c.residual_and_jacobian(h);
            let n = f0.len();
            for col in 0..n {
                // Central differences.
                let d = 1e-6;
                let at = |sign: f64| {
                    let mut cp = every_part();
                    for (k, &x) in v.iter().enumerate() {
                        cp.set(3 + k, x + if k == col { sign * d } else { 0.0 });
                    }
                    cp.residual_and_jacobian(h).0
                };
                let (f1, fm) = (at(1.0), at(-1.0));
                for row in 0..n {
                    let fd = (f1[row] - fm[row]) / (2.0 * d);
                    let an = jac[row * n + col];
                    // Relative; slopes below 10 nS (GMIN's, say) absolutely, since
                    // finite differences cannot resolve them against milliampere currents.
                    let scale = an.abs().max(fd.abs());
                    if scale < 1e-8 {
                        assert!(
                            (fd - an).abs() < 1e-11,
                            "case {ci} h {h:?}: d f{row} / d v{col}: {an:.6e} against {fd:.6e}"
                        );
                        continue;
                    }
                    let err = (fd - an).abs() / scale;
                    if err > 1e-4 {
                        panic!(
                            "case {ci} h {h:?}: d f{row} / d v{col}: analytic {an:.6e}, finite differences {fd:.6e}"
                        );
                    }
                    if err > worst.0 {
                        worst = (
                            err,
                            format!(
                                "case {ci} h {h:?} d f{row} / d v{col}: {an:.6e} against {fd:.6e}"
                            ),
                        );
                    }
                }
            }
        }
    }
    eprintln!("worst relative difference {:.2e} ({})", worst.0, worst.1);
}

#[test]
fn a_diode_and_resistor_match_their_closed_form() {
    // 5 V through 1K into a 1N34A.
    let mut c = Circuit::new(4, 2);
    c.set(1, 5.0);
    c.add(Part::Resistor { a: 1, b: 2, r: 1e3 });
    c.add(Part::Resistor {
        a: 2,
        b: 3,
        r: D1N34A.rs,
    });
    let nvt = D1N34A.n * vt(25.0);
    c.add(Part::Diode {
        a: 3,
        k: GND,
        is: D1N34A.is,
        nvt,
        cap: CAP,
        tt: 0.0,
    });
    // A wild start: the limiting must bring it in.
    c.set(2, 5.0);
    c.set(3, 5.0);
    let it = c.dc().expect("dc");
    let law = D1N34A.law(25.0);
    let (vj, i) = series_junction(5.0, 1e3 + D1N34A.rs, law);
    let i_mna = (5.0 - c.v(2)) / 1e3;
    eprintln!(
        "{it} iterations: junction {:.9} V against {vj:.9} V; {i_mna:.9e} A against {i:.9e} A",
        c.v(3)
    );
    assert!(
        (c.v(3) - vj).abs() < 1e-6,
        "junction {} against {vj}",
        c.v(3)
    );
    assert!(
        (i_mna - i).abs() < 1e-9 * i.abs().max(1e-6),
        "current {i_mna} against {i}"
    );
}

#[test]
fn an_rc_step_is_backward_euler() {
    // 1 V into 1K and 1 uF: backward Euler's v_n = (v_{n-1} + a) / (1 + a), a = h / RC.
    let mut c = Circuit::new(3, 2);
    c.add(Part::Resistor { a: 1, b: 2, r: 1e3 });
    c.add(Part::Capacitor {
        a: 2,
        b: GND,
        c: 1e-6,
    });
    c.dc().expect("dc");
    c.set(1, 1.0);
    let h = 1e-4;
    let a = h / 1e-3;
    let mut v = 0.0;
    for _ in 0..20 {
        c.step(h).expect("step");
        v = (v + a) / (1.0 + a);
        assert!((c.v(2) - v).abs() < 1e-9, "{} against {v}", c.v(2));
    }
}
