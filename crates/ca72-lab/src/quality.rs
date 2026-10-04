//! A quality mode measured against No Compromises, each limit on its own path (history.md,
//! "High Fidelity: how it is measured"; `ca72-lab hifi`). Pitch: the keyboard's voltage (1 V an
//! octave) and each oscillator's timing current (its frequency follows it) within 0.01 cent
//! of No Compromises', allowing up to a sample of shift: each sample within the range No
//! Compromises' took from the sample before to the one after, widened by the limit.
//! Contours: within 1e-5 V (-120 dB of 10 V), with the same allowance of a sample. Audio: the
//! mode's render with the pitch and contour paths held at No Compromises
//! (`Voice::hold_control_quality`) within 1e-6 of full scale (-120 dBFS) of No Compromises'
//! in every sample.

use crate::worst::Probes;

/// The limits: cents, volts (the contours), full scale (the audio).
pub const CENTS: f64 = 0.01;
pub const CONTOUR_V: f64 = 1e-5;
pub const AUDIO: f64 = 1e-6;

/// Each path's largest distance from No Compromises and the sample where it is: the
/// keyboard's (cents), each oscillator's (cents), each contour's (V, filter then loudness),
/// the audio's (full scale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    pub keyboard: (f64, usize),
    /// How long the keyboard's pitch lay beyond the limit, in all and at the longest
    /// stretch, samples (a transient or a note held out of tune).
    pub keyboard_beyond: (usize, usize),
    pub oscillators: [(f64, usize); 3],
    pub contours: [(f64, usize); 2],
    pub audio: (f64, usize),
}

impl Measured {
    /// The limits crossed, described (none: within the mode's limits).
    pub fn crossed(&self, name: &str, rate: f64) -> Vec<String> {
        let at = |i: usize| i as f64 / rate;
        let mut out = Vec::new();
        if self.keyboard.0 > CENTS {
            out.push(format!(
                "{name}: the keyboard's pitch {:.3e} cent at {:.4} s",
                self.keyboard.0,
                at(self.keyboard.1)
            ));
        }
        for (k, (d, i)) in self.oscillators.iter().enumerate() {
            if *d > CENTS {
                out.push(format!(
                    "{name}: oscillator {}'s pitch {d:.3e} cent at {:.4} s",
                    k + 1,
                    at(*i)
                ));
            }
        }
        for (k, (d, i)) in self.contours.iter().enumerate() {
            if *d > CONTOUR_V {
                out.push(format!(
                    "{name}: the {} contour {d:.3e} V at {:.4} s",
                    ["filter", "loudness"][k],
                    at(*i)
                ));
            }
        }
        if self.audio.0 > AUDIO {
            out.push(format!(
                "{name}: the audio {:.3e} ({:.1} dBFS) at {:.4} s",
                self.audio.0,
                20.0 * self.audio.0.log10(),
                at(self.audio.1)
            ));
        }
        out
    }
}

/// How far each sample of `x` lies outside the range `r` took from the sample before to
/// the one after (a shift of up to a sample allowed): the largest distance and its sample.
/// Samples where the reference is not finite are skipped; a sample of `x` that is not
/// finite where the reference is, is infinitely far.
pub fn beyond(x: &[f64], r: &[f64]) -> (f64, usize) {
    let mut worst = (0.0f64, 0usize);
    for (i, &v) in x.iter().enumerate().take(r.len()) {
        let near = &r[i.saturating_sub(1)..=(i + 1).min(r.len() - 1)];
        let d = if near.iter().any(|y| !y.is_finite()) {
            0.0
        } else if !v.is_finite() {
            f64::INFINITY
        } else {
            let (lo, hi) = near
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &y| {
                    (l.min(y), h.max(y))
                });
            (lo - v).max(v - hi).max(0.0)
        };
        if d > worst.0 {
            worst = (d, i);
        }
    }
    worst
}

/// Each sample's distance of `x` outside the range `r` took a sample either side (see
/// [`beyond`]).
pub fn distances(x: &[f64], r: &[f64]) -> Vec<f64> {
    (0..x.len().min(r.len()))
        .map(|i| {
            let near = &r[i.saturating_sub(1)..=(i + 1).min(r.len() - 1)];
            if near.iter().any(|y| !y.is_finite()) {
                0.0
            } else if !x[i].is_finite() {
                f64::INFINITY
            } else {
                let (lo, hi) = near
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &y| {
                        (l.min(y), h.max(y))
                    });
                (lo - x[i]).max(x[i] - hi).max(0.0)
            }
        })
        .collect()
}

/// The distances ([`Measured`]) of a mode's render from No Compromises': `nc` and `mode`
/// the two renders' probes, `held` the mode's audio with the pitch and contour paths held at
/// No Compromises, `nc_audio` No Compromises' audio.
pub fn measure(nc: &[Probes], mode: &[Probes], nc_audio: &[f64], held: &[f64]) -> Measured {
    let col = |r: &[Probes], k: usize| r.iter().map(|p| p[k]).collect::<Vec<f64>>();
    let cents = |x: Vec<f64>| -> Vec<f64> {
        x.into_iter()
            .map(|i| if i > 0.0 { 1200.0 * i.log2() } else { f64::NAN })
            .collect()
    };
    let (kd, ki) = beyond(&col(mode, 0), &col(nc, 0));
    let (mut total, mut run, mut longest) = (0, 0, 0);
    for d in distances(&col(mode, 0), &col(nc, 0)) {
        if d * 1200.0 > CENTS {
            total += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let osc = |k: usize| beyond(&cents(col(mode, k)), &cents(col(nc, k)));
    let contour = |k: usize| beyond(&col(mode, k), &col(nc, k));
    let mut audio = (0.0f64, 0usize);
    for (i, (a, b)) in held.iter().zip(nc_audio).enumerate() {
        let d = if a.is_finite() {
            (a - b).abs()
        } else {
            f64::INFINITY
        };
        if d > audio.0 {
            audio = (d, i);
        }
    }
    Measured {
        keyboard: (kd * 1200.0, ki),
        keyboard_beyond: (total, longest),
        oscillators: [osc(1), osc(2), osc(3)],
        contours: [contour(4), contour(5)],
        audio,
    }
}

/// The audio compared allowing for a delay (a mode whose resamplers differ is late by a
/// few samples): the whole-sample lag within `max_lag` at which `mode` best matches `nc`
/// (the cross-correlation's peak), and the largest difference after shifting it back.
pub fn delayed_difference(mode: &[f64], nc: &[f64], max_lag: usize) -> (isize, f64) {
    let n = mode.len().min(nc.len());
    let m = max_lag as isize;
    let at = |x: &[f64], i: isize| {
        if i >= 0 && (i as usize) < n {
            x[i as usize]
        } else {
            0.0
        }
    };
    let lag = (-m..=m)
        .map(|l| {
            let c: f64 = (0..n as isize).map(|i| at(mode, i + l) * at(nc, i)).sum();
            (l, c)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |x| x.0);
    let d = (m..n as isize - m)
        .map(|i| (at(mode, i + lag) - at(nc, i)).abs())
        .fold(0.0f64, f64::max);
    (lag, d)
}

/// The audio's spectra compared, which a delay does not change: in windows of 4096 samples
/// (half overlapping, Hann), the difference of the magnitude spectra relative to No
/// Compromises', dB, where No Compromises' window is above -60 dBFS: the median window's
/// and the worst's (at its first sample).
pub fn spectral_difference(mode: &[f64], nc: &[f64]) -> Option<(f64, (f64, usize))> {
    const N: usize = 4096;
    let w = ca72_analysis::fft::hann(N);
    // A full-scale sine's windowed power, summed over the bins (for the -60 dBFS floor).
    let full: f64 = w.iter().map(|x| x * x).sum::<f64>() * N as f64 / 4.0;
    let mut out = Vec::new();
    let mut at = 0;
    while at + N <= mode.len().min(nc.len()) {
        let pn = ca72_analysis::fft::power(&nc[at..at + N], &w);
        let pm = ca72_analysis::fft::power(&mode[at..at + N], &w);
        let sig: f64 = pn.iter().sum();
        if sig > full * 1e-6 {
            let diff: f64 = pn
                .iter()
                .zip(&pm)
                .map(|(a, b)| (a.sqrt() - b.sqrt()).powi(2))
                .sum();
            out.push((10.0 * (diff.max(1e-300) / sig).log10(), at));
        }
        at += N / 2;
    }
    if out.is_empty() {
        return None;
    }
    let worst = out
        .iter()
        .copied()
        .fold((f64::NEG_INFINITY, 0), |a, b| if b.0 > a.0 { b } else { a });
    let mut db: Vec<f64> = out.iter().map(|x| x.0).collect();
    db.sort_by(f64::total_cmp);
    Some((db[db.len() / 2], worst))
}

#[cfg(test)]
mod tests {
    use super::beyond;

    /// A step that comes a fraction of a sample sooner or later lies within the range the
    /// reference takes a sample either side: no distance. A step of the wrong height, or
    /// a sample not finite, is caught.
    #[test]
    fn a_shift_within_a_sample_passes_and_a_wrong_value_does_not() {
        let r: Vec<f64> = (0..10).map(|i| if i < 5 { 0.0 } else { 1.0 }).collect();
        let mut x = r.clone();
        x[4] = 0.4; // the step begun within the sample before
        x[5] = 0.9;
        assert_eq!(beyond(&x, &r), (0.0, 0));
        let mut y = r.clone();
        y[7] = 1.0 + 1e-3;
        let (d, i) = beyond(&y, &r);
        assert!((d - 1e-3).abs() < 1e-12 && i == 7, "{d} at {i}");
        let mut z = r.clone();
        z[2] = f64::NAN;
        assert_eq!(beyond(&z, &r), (f64::INFINITY, 2));
    }
}
