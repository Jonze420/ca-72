//! Band-limiting helpers: polynomial band-limited steps for discontinuities at sub-sample
//! times, and halfband decimation from the oversampled rate to the output rate.

/// A signal with discontinuities corrected by two-point polynomial band-limited steps
/// (polyBLEP). Values come out one sample late: a step between samples n-1 and n corrects
/// both, so sample n-1 is held until sample n is known.
#[derive(Debug, Clone, Default)]
pub struct BlepLine {
    held: f64,
    next_correction: f64,
}

impl BlepLine {
    /// Records a step of `delta` at `frac` of the way from the previous sample to the one
    /// about to be pushed (0 < frac <= 1).
    pub fn step(&mut self, delta: f64, frac: f64) {
        // d: time from the step to the coming sample, in samples.
        let d = (1.0 - frac).clamp(0.0, 1.0);
        self.held += delta * d * d * 0.5;
        self.next_correction -= delta * (1.0 - d) * (1.0 - d) * 0.5;
    }

    /// Records an impulse of `area` (value x samples) at `frac` of the way from the previous
    /// sample to the coming one: the limit of two opposite steps closing together, spread
    /// over the two samples by their distance (its area is kept exactly).
    pub fn impulse(&mut self, area: f64, frac: f64) {
        let d = (1.0 - frac).clamp(0.0, 1.0);
        self.held += area * d;
        self.next_correction += area * (1.0 - d);
    }

    /// Pushes the naive value of the new sample; returns the previous sample, corrected.
    pub fn push(&mut self, naive: f64) -> f64 {
        let out = self.held;
        self.held = naive + self.next_correction;
        self.next_correction = 0.0;
        out
    }
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let y = x * x / 4.0;
    for k in 1..60 {
        term *= y / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Kaiser-windowed halfband low-pass taps (odd length), for decimation by two.
pub fn halfband_taps(len: usize, beta: f64) -> Vec<f64> {
    let n = len | 1;
    let m = (n / 2) as f64;
    let i0b = bessel_i0(beta);
    (0..n)
        .map(|i| {
            let k = i as f64 - m;
            let sinc = if k == 0.0 {
                0.5
            } else {
                crate::ulp::sin(core::f64::consts::PI * k / 2.0) / (core::f64::consts::PI * k)
            };
            let r = k / m;
            sinc * bessel_i0(beta * libm::sqrt((1.0 - r * r).max(0.0))) / i0b
        })
        .collect()
}

/// A decimator by two: a halfband FIR evaluated every other input sample.
#[derive(Debug, Clone)]
pub struct Halfband {
    taps: Vec<f64>,
    line: Vec<f64>,
    pos: usize,
    phase: bool,
    /// Sparse ([`Halfband::sparse`]): the taps an even distance from the centre, which a
    /// halfband's are zero but for rounding (about 1e-17 of the rest), exactly zero and
    /// skipped.
    sparse: bool,
}

impl Halfband {
    pub fn new(len: usize, beta: f64) -> Halfband {
        let taps = halfband_taps(len, beta);
        let n = taps.len();
        Halfband {
            taps,
            line: vec![0.0; 2 * n],
            pos: 0,
            phase: false,
            sparse: false,
        }
    }

    /// [`Halfband::new`] with the taps an even distance from the centre (their sin(pi k /
    /// 2) is zero but for rounding) exactly zero and skipped: a little under half the work
    /// (Potato).
    pub fn sparse(len: usize, beta: f64) -> Halfband {
        let mut h = Halfband::new(len, beta);
        let m = h.taps.len() / 2;
        for (i, t) in h.taps.iter_mut().enumerate() {
            if i != m && (i as isize - m as isize) % 2 == 0 {
                *t = 0.0;
            }
        }
        h.sparse = true;
        h
    }

    /// The window's sum with the taps: in a sparse halfband only the taps an odd distance
    /// from the centre and the centre's.
    fn sum(&self, w: &[f64]) -> f64 {
        if self.sparse {
            let m = self.taps.len() / 2;
            let mut acc = 0.0;
            for i in ((1 - m % 2)..self.taps.len()).step_by(2) {
                acc += self.taps[i] * w[i];
            }
            acc + self.taps[m] * w[m]
        } else {
            let mut acc = 0.0;
            for (t, v) in self.taps.iter().zip(w) {
                acc += t * v;
            }
            acc
        }
    }

    /// Group delay in input samples.
    pub fn delay(&self) -> usize {
        self.taps.len() / 2
    }

    /// Pushes one input sample; returns an output sample every second call.
    pub fn push(&mut self, x: f64) -> Option<f64> {
        let n = self.taps.len();
        // A doubled ring buffer: the window [pos, pos + n) is always contiguous.
        self.line[self.pos] = x;
        self.line[self.pos + n] = x;
        self.pos = if self.pos == 0 { n - 1 } else { self.pos - 1 };
        self.phase = !self.phase;
        if !self.phase {
            return None;
        }
        let w = &self.line[self.pos + 1..self.pos + 1 + n];
        Some(self.sum(w))
    }

    /// Filters one sample at the input rate (every sample produces one): for
    /// interpolation, where the halfband runs at the higher rate.
    pub fn filter(&mut self, x: f64) -> f64 {
        let n = self.taps.len();
        self.line[self.pos] = x;
        self.line[self.pos + n] = x;
        let w = &self.line[self.pos..self.pos + n];
        let acc = self.sum(w);
        self.pos = if self.pos == 0 { n - 1 } else { self.pos - 1 };
        acc
    }

    /// [`Halfband::filter`] of a stuffed zero in a sparse halfband: its window then holds
    /// samples an odd distance from the newest only, which meet no tap but the centre's.
    fn filter_stuffed_zero(&mut self) -> f64 {
        let n = self.taps.len();
        self.line[self.pos] = 0.0;
        self.line[self.pos + n] = 0.0;
        let m = n / 2;
        let acc = if m % 2 == 1 {
            self.taps[m] * self.line[self.pos + m]
        } else {
            self.sum(&self.line[self.pos..self.pos + n])
        };
        self.pos = if self.pos == 0 { n - 1 } else { self.pos - 1 };
        acc
    }

    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
        self.phase = false;
    }
}

/// Decimation by 2^stages: a short halfband first, a long one last.
#[derive(Debug, Clone)]
pub struct Decimator {
    stages: Vec<Halfband>,
}

impl Decimator {
    /// `factor` is 1, 2, 4 or 8. The last stage passes 0..0.417 of its output Nyquist
    /// (20 kHz at 48 kHz) and stops from 0.583 (28 kHz) down by about 100 dB.
    pub fn new(factor: usize) -> Decimator {
        Self::with(factor, Halfband::new)
    }

    /// [`Decimator::new`] with sparse halfbands ([`Halfband::sparse`]; Potato).
    pub fn sparse(factor: usize) -> Decimator {
        Self::with(factor, Halfband::sparse)
    }

    fn with(factor: usize, make: fn(usize, f64) -> Halfband) -> Decimator {
        let stages = stages_len(factor);
        let stages = (0..stages)
            .map(|i| {
                if i + 1 == stages {
                    make(79, 10.06)
                } else {
                    make(23, 10.06)
                }
            })
            .collect();
        Decimator { stages }
    }

    pub fn push(&mut self, x: f64) -> Option<f64> {
        let mut v = x;
        for s in &mut self.stages {
            v = s.push(v)?;
        }
        Some(v)
    }

    /// Group delay in output samples: each stage's delay at its own input rate, which is
    /// 2^(stages left, itself included) times the output rate.
    pub fn delay(&self) -> f64 {
        let n = self.stages.len();
        self.stages
            .iter()
            .enumerate()
            .map(|(k, s)| s.delay() as f64 / f64::from(1u32 << (n - k)))
            .sum()
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Halfband::reset);
    }
}

/// Interpolation by 2^stages: zero-stuffing and the same halfband filters as [`Decimator`]
/// in the reverse order (long first), with the gain of two each stage needs.
#[derive(Debug, Clone)]
pub struct Interpolator {
    stages: Vec<Halfband>,
    factor: usize,
}

impl Interpolator {
    /// `factor` is 1, 2, 4 or 8.
    pub fn new(factor: usize) -> Interpolator {
        Self::with(factor, Halfband::new)
    }

    /// [`Interpolator::new`] with sparse halfbands ([`Halfband::sparse`]; Potato): each
    /// stuffed zero's output takes one tap.
    pub fn sparse(factor: usize) -> Interpolator {
        Self::with(factor, Halfband::sparse)
    }

    fn with(factor: usize, make: fn(usize, f64) -> Halfband) -> Interpolator {
        let stages = stages_len(factor);
        let stages = (0..stages)
            .map(|i| {
                if i == 0 {
                    make(79, 10.06)
                } else {
                    make(23, 10.06)
                }
            })
            .collect();
        Interpolator {
            stages,
            factor: 1 << (stages_len(factor)),
        }
    }

    pub fn factor(&self) -> usize {
        self.factor
    }

    /// One input sample to `factor` output samples.
    pub fn push(&mut self, x: f64, out: &mut [f64]) {
        let mut buf = [0.0f64; 8];
        let mut n = 1usize;
        buf[0] = x;
        for s in &mut self.stages {
            let mut next = [0.0f64; 8];
            for i in 0..n {
                next[2 * i] = s.filter(2.0 * buf[i]);
                next[2 * i + 1] = if s.sparse {
                    s.filter_stuffed_zero()
                } else {
                    s.filter(0.0)
                };
            }
            n *= 2;
            buf = next;
        }
        out[..n].copy_from_slice(&buf[..n]);
    }

    /// Group delay in input samples.
    pub fn delay(&self) -> f64 {
        self.stages
            .iter()
            .enumerate()
            .map(|(k, s)| s.delay() as f64 / f64::from(1u32 << (k + 1)))
            .sum()
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Halfband::reset);
    }
}

/// The halfband stages for an oversampling factor; any other factor than 1, 2, 4 or 8 is a
/// caller's error, not something to round.
fn stages_len(factor: usize) -> u32 {
    match factor {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        _ => panic!("oversampling factor {factor}: 1, 2, 4 or 8"),
    }
}
