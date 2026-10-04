//! Linear circuits in real time: a circuit's small-signal model at its operating point
//! ([`crate::mna::Circuit::small_signal`]: G x + C dx/dt = -g_in u over its solved nodes),
//! its frequency response, and its discretisation by the trapezoidal rule for one input
//! (x1 = A x0 + B (u0 + u1)): a fixed matrix update per step.

/// A small-signal model: G, C (row-major, n x n) and the input's column.
#[derive(Debug, Clone)]
pub struct SmallSignal {
    pub n: usize,
    pub g: Vec<f64>,
    pub c: Vec<f64>,
    pub g_in: Vec<f64>,
}

/// Solves `m x = rhs` in place for a dense n x n system (partial pivoting); `m` is
/// destroyed. Complex numbers as (re, im) pairs.
fn solve_complex(n: usize, m: &mut [(f64, f64)], rhs: &mut [(f64, f64)]) {
    let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let div = |a: (f64, f64), b: (f64, f64)| {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    };
    let abs = |a: (f64, f64)| a.0.hypot(a.1);
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| abs(m[i * n + col]).total_cmp(&abs(m[j * n + col])))
            .unwrap_or(col);
        if piv != col {
            for c in 0..n {
                m.swap(col * n + c, piv * n + c);
            }
            rhs.swap(col, piv);
        }
        let d = m[col * n + col];
        for r in col + 1..n {
            let f = div(m[r * n + col], d);
            if f != (0.0, 0.0) {
                for c in col..n {
                    let t = mul(f, m[col * n + c]);
                    m[r * n + c] = (m[r * n + c].0 - t.0, m[r * n + c].1 - t.1);
                }
                let t = mul(f, rhs[col]);
                rhs[r] = (rhs[r].0 - t.0, rhs[r].1 - t.1);
            }
        }
    }
    for col in (0..n).rev() {
        let mut s = rhs[col];
        for c in col + 1..n {
            let t = mul(m[col * n + c], rhs[c]);
            s = (s.0 - t.0, s.1 - t.1);
        }
        rhs[col] = div(s, m[col * n + col]);
    }
}

/// The inverse of a dense real n x n matrix (Gauss-Jordan with partial pivoting).
/// Inverts `a` (n x n, destroyed) into `inv` (Gauss-Jordan with partial pivoting), without
/// allocating.
fn invert_into(n: usize, a: &mut [f64], inv: &mut [f64]) {
    inv.iter_mut().for_each(|x| *x = 0.0);
    for i in 0..n {
        inv[i * n + i] = 1.0;
    }
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))
            .unwrap_or(col);
        if piv != col {
            for c in 0..n {
                a.swap(col * n + c, piv * n + c);
                inv.swap(col * n + c, piv * n + c);
            }
        }
        let d = a[col * n + col];
        for c in 0..n {
            a[col * n + c] /= d;
            inv[col * n + c] /= d;
        }
        for r in 0..n {
            if r != col {
                let f = a[r * n + col];
                if f != 0.0 {
                    for c in 0..n {
                        a[r * n + c] -= f * a[col * n + c];
                        inv[r * n + c] -= f * inv[col * n + c];
                    }
                }
            }
        }
    }
}

impl SmallSignal {
    /// The response of solved node `out` to the input at `f` Hz: -(G + j 2 pi f C)^-1 g_in,
    /// as (re, im).
    pub fn response(&self, out: usize, f: f64) -> (f64, f64) {
        let n = self.n;
        let w = 2.0 * core::f64::consts::PI * f;
        let mut m: Vec<(f64, f64)> = (0..n * n).map(|k| (self.g[k], w * self.c[k])).collect();
        let mut rhs: Vec<(f64, f64)> = self.g_in.iter().map(|&x| (-x, 0.0)).collect();
        solve_complex(n, &mut m, &mut rhs);
        rhs[out]
    }

    /// The trapezoidal rule at step `h`: (G/2 + C/h) x1 = (C/h - G/2) x0 - g_in (u0 + u1)/2.
    pub fn discrete(&self, h: f64) -> Discrete {
        let n = self.n;
        let mut d = Discrete {
            n,
            a: vec![0.0; n * n],
            b: vec![0.0; n],
            x: vec![0.0; n],
            next: vec![0.0; n],
            u_prev: 0.0,
            h,
            work: vec![0.0; 3 * n * n],
        };
        d.rebuild(self, h);
        d
    }
}

/// A discretised linear circuit: its state is the solved nodes' deviations from the
/// operating point.
#[derive(Debug, Clone)]
pub struct Discrete {
    n: usize,
    a: Vec<f64>,
    b: Vec<f64>,
    x: Vec<f64>,
    next: Vec<f64>,
    u_prev: f64,
    h: f64,
    /// Scratch for [`Discrete::rebuild`]: three n x n matrices.
    work: Vec<f64>,
}

impl Discrete {
    /// Recomputes the matrices for `model` (of this size) at step `h` in place, keeping the
    /// state: no allocation, so a parameter's change can be taken on the audio thread.
    pub fn rebuild(&mut self, model: &SmallSignal, h: f64) {
        let n = self.n;
        assert_eq!(model.n, n);
        let (lhs, rest) = self.work.split_at_mut(n * n);
        let (li, rhs) = rest.split_at_mut(n * n);
        for k in 0..n * n {
            lhs[k] = model.g[k] / 2.0 + model.c[k] / h;
            rhs[k] = model.c[k] / h - model.g[k] / 2.0;
        }
        invert_into(n, lhs, li);
        self.a.iter_mut().for_each(|x| *x = 0.0);
        for i in 0..n {
            for k in 0..n {
                let l = li[i * n + k];
                if l != 0.0 {
                    for j in 0..n {
                        self.a[i * n + j] += l * rhs[k * n + j];
                    }
                }
            }
        }
        for i in 0..n {
            self.b[i] = -(0..n).map(|k| li[i * n + k] * model.g_in[k]).sum::<f64>() / 2.0;
        }
        self.h = h;
    }

    /// Takes over another discretisation's state (the same circuit's, its parameters
    /// changed): the nodes keep their voltages across the change.
    pub fn take_state(&mut self, from: &Discrete) {
        if from.n == self.n {
            self.x.copy_from_slice(&from.x);
            self.u_prev = from.u_prev;
        }
    }

    /// Takes over another discretisation's state node by node (a model of the same circuit
    /// with other nodes): `map` pairs its nodes' indices with this one's (with `back`, this
    /// one's with its); the rest start at rest.
    pub fn carry(&mut self, from: &Discrete, map: &[(usize, usize)], back: bool) {
        self.x.iter_mut().for_each(|x| *x = 0.0);
        for &(a, b) in map {
            let (a, b) = if back { (b, a) } else { (a, b) };
            self.x[b] = from.x[a];
        }
        self.u_prev = from.u_prev;
    }

    /// One step with the input's new value.
    pub fn step(&mut self, u: f64) {
        let n = self.n;
        let s = u + self.u_prev;
        // Four rows at a time: each row's sum in the same order as alone (the same result),
        // the four chains of additions overlapping (performance).
        let mut i = 0;
        while i + 4 <= n {
            let rows = &self.a[i * n..(i + 4) * n];
            let (r0, rest) = rows.split_at(n);
            let (r1, rest) = rest.split_at(n);
            let (r2, r3) = rest.split_at(n);
            let mut acc = [
                self.b[i] * s,
                self.b[i + 1] * s,
                self.b[i + 2] * s,
                self.b[i + 3] * s,
            ];
            for (k, x) in self.x.iter().enumerate() {
                acc[0] += r0[k] * x;
                acc[1] += r1[k] * x;
                acc[2] += r2[k] * x;
                acc[3] += r3[k] * x;
            }
            self.next[i..i + 4].copy_from_slice(&acc);
            i += 4;
        }
        for i in i..n {
            let row = &self.a[i * n..(i + 1) * n];
            let mut acc = self.b[i] * s;
            for (a, x) in row.iter().zip(&self.x) {
                acc += a * x;
            }
            self.next[i] = acc;
        }
        std::mem::swap(&mut self.x, &mut self.next);
        self.u_prev = u;
    }

    /// A solved node's deviation.
    pub fn x(&self, i: usize) -> f64 {
        self.x[i]
    }

    /// The discrete system's response at solved node `out` to the input at `f` Hz (for
    /// tests): x = (z - A)^-1 B (z + 1) u at z = exp(j 2 pi f h).
    pub fn response(&self, out: usize, f: f64) -> (f64, f64) {
        let n = self.n;
        let th = 2.0 * core::f64::consts::PI * f * self.h;
        let z = (th.cos(), th.sin());
        let mut m: Vec<(f64, f64)> = (0..n * n)
            .map(|k| {
                let diag = if k / n == k % n { z } else { (0.0, 0.0) };
                (diag.0 - self.a[k], diag.1)
            })
            .collect();
        let zp1 = (z.0 + 1.0, z.1);
        let mut rhs: Vec<(f64, f64)> = self.b.iter().map(|&b| (b * zp1.0, b * zp1.1)).collect();
        solve_complex(n, &mut m, &mut rhs);
        rhs[out]
    }
}
