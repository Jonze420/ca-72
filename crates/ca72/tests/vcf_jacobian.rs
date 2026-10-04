//! The filter loop's analytic Jacobian against finite differences, across the states'
//! working range (small signals, the ladder's tanh saturated, the followers saturated),
//! with the pairs solved behind their drops and taken at once (Potato's).

use ca72::vcf::{Drive, STATES, VcfCircuit};

#[test]
fn filter_jacobian_matches_finite_differences() {
    let c = VcfCircuit {
        r14: 3e3,
        ..VcfCircuit::default()
    };
    for plain in [false, true] {
        let d = Drive {
            i_n: 0.3 / 33e3,
            g_n: 2.0 / 33e3,
            bias: c.bias(400e-6, 25.0),
            p: 1.02,
            plain,
        };
        check(&c, &d);
    }
}

fn check(c: &VcfCircuit, d: &Drive) {
    let states: [[f64; STATES]; 4] = [
        [1e-3, -2e-3, 5e-4, 1e-3, 2e-3, 1e-3, 1e-4, -2e-4, 0.1],
        [0.08, -0.1, 0.12, 0.09, 0.05, 0.3, 2e-3, 1e-3, -0.2],
        [-0.02, 0.03, -0.05, 0.06, 0.15, -0.4, -1e-3, 3e-3, 0.4],
        [0.0, 0.0, 0.0, 0.0, -0.3, 0.0, 0.0, 0.0, 0.0],
    ];
    for y in states {
        let mut jac = [[0.0; STATES]; STATES];
        let (f, _) = c.eval(&y, d, Some(&mut jac));
        for n in 0..STATES {
            let h = 1e-7;
            let mut yp = y;
            let mut ym = y;
            yp[n] += h;
            ym[n] -= h;
            let (fp, _) = c.eval(&yp, d, None);
            let (fm, _) = c.eval(&ym, d, None);
            for m in 0..STATES {
                let num = (fp[m] - fm[m]) / (2.0 * h);
                let scale = jac[m]
                    .iter()
                    .fold(0.0f64, |a, v| a.max(v.abs()))
                    .max(f[m].abs())
                    .max(1.0);
                assert!(
                    (num - jac[m][n]).abs() <= 1e-5 * scale,
                    "d f{m} / d y{n} at {y:?}: analytic {}, numeric {num}",
                    jac[m][n]
                );
            }
        }
    }
}

/// The series junction solver returns the junction voltage that balances the resistor's
/// current, wherever Newton's method converges (it once returned a bisection midpoint when
/// the converged point sat on the bracket's edge).
#[test]
fn series_junction_balances_its_resistor() {
    use ca72::contour::{D1N34A, Q2N3392};
    use ca72::devices::series_junction;
    let n = Q2N3392.at(25.0);
    let base = n.base_law();
    let ge = D1N34A.law(25.0);
    for k in 0..20_000 {
        let v = -12.0 + 24.0 * k as f64 / 20_000.0;
        for (r, law) in [
            (47_010.0, &base as &dyn Fn(f64) -> (f64, f64)),
            (100_020.0, &ge),
        ] {
            let (vj, i) = series_junction(v, r, law);
            let err = (v - vj) / r - i;
            assert!(
                err.abs() <= 1e-9 * (1.0 + i.abs() * 1e6),
                "v {v}: vj {vj}, i {i}, R's {}",
                (v - vj) / r
            );
        }
    }
}
