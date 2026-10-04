//! Loudness per ITU-R BS.1770-4 and EBU R 128: K-weighting, mean square over 400 ms
//! blocks (momentary, 75% overlap) and 3 s windows (short-term), absolute gate at
//! -70 LUFS and relative gate at -10 LU for the integrated value; loudness range per
//! EBU Tech 3342.

use serde::Serialize;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
}

impl Biquad {
    fn run(&self, x: &[f64]) -> Vec<f64> {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        x.iter()
            .map(|&v| {
                let y = self.b[0] * v + self.b[1] * x1 + self.b[2] * x2
                    - self.a[0] * y1
                    - self.a[1] * y2;
                x2 = x1;
                x1 = v;
                y2 = y1;
                y1 = y;
                y
            })
            .collect()
    }
}

/// The two K-weighting stages for a sample rate (the BS.1770 coefficients, derived for
/// any rate as in libebur128).
fn k_filter(sr: f64) -> [Biquad; 2] {
    let (f0, g, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_236_955_419_6,
    );
    let k = libm::tan(PI * f0 / sr);
    let vh = libm::pow(10.0, g / 20.0);
    let vb = libm::pow(vh, 0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = libm::tan(PI * f0 / sr);
    let d = 1.0 + k / q + k * k;
    let hp = Biquad {
        b: [1.0, -2.0, 1.0],
        a: [2.0 * (k * k - 1.0) / d, (1.0 - k / q + k * k) / d],
    };
    [shelf, hp]
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Loudness {
    /// Integrated loudness, LUFS; `None` for silence or less than 400 ms.
    pub integrated: Option<f64>,
    pub momentary_max: Option<f64>,
    pub short_term_max: Option<f64>,
    /// Loudness range, LU.
    pub range: Option<f64>,
}

fn lufs(z: f64) -> f64 {
    -0.691 + 10.0 * libm::log10(z)
}

/// Mean squares of K-weighted channels summed, over windows of `w` frames every `hop`.
fn blocks(prefix: &[Vec<f64>], w: usize, hop: usize) -> Vec<f64> {
    let n = prefix[0].len() - 1;
    if n < w {
        return Vec::new();
    }
    (0..=(n - w) / hop)
        .map(|i| {
            let s = i * hop;
            prefix.iter().map(|p| (p[s + w] - p[s]) / w as f64).sum()
        })
        .collect()
}

fn gated_mean(z: &[f64], relative: f64) -> Option<f64> {
    let abs: Vec<f64> = z
        .iter()
        .copied()
        .filter(|&z| z > 0.0 && lufs(z) > -70.0)
        .collect();
    if abs.is_empty() {
        return None;
    }
    let threshold = lufs(abs.iter().sum::<f64>() / abs.len() as f64) + relative;
    let rel: Vec<f64> = abs.into_iter().filter(|&z| lufs(z) > threshold).collect();
    (!rel.is_empty()).then(|| rel.iter().sum::<f64>() / rel.len() as f64)
}

pub fn loudness(channels: &[&[f32]], sample_rate: u32) -> Loudness {
    let sr = f64::from(sample_rate);
    let [shelf, hp] = k_filter(sr);
    let prefix: Vec<Vec<f64>> = channels
        .iter()
        .map(|c| {
            let x: Vec<f64> = c.iter().map(|v| f64::from(*v)).collect();
            let y = hp.run(&shelf.run(&x));
            let mut p = Vec::with_capacity(y.len() + 1);
            let mut acc = 0.0;
            p.push(0.0);
            for v in y {
                acc += v * v;
                p.push(acc);
            }
            p
        })
        .collect();
    if prefix.is_empty() {
        return Loudness {
            integrated: None,
            momentary_max: None,
            short_term_max: None,
            range: None,
        };
    }
    let hop = (sr * 0.1).round() as usize;
    let momentary = blocks(&prefix, (sr * 0.4).round() as usize, hop);
    let short = blocks(&prefix, (sr * 3.0).round() as usize, hop);
    let max = |v: &[f64]| {
        v.iter()
            .copied()
            .filter(|z| *z > 0.0)
            .fold(None, |m: Option<f64>, z| Some(m.map_or(z, |m| m.max(z))))
            .map(lufs)
    };
    let range = {
        let abs: Vec<f64> = short
            .iter()
            .copied()
            .filter(|&z| z > 0.0 && lufs(z) > -70.0)
            .collect();
        if abs.is_empty() {
            None
        } else {
            let threshold = lufs(abs.iter().sum::<f64>() / abs.len() as f64) - 20.0;
            let mut l: Vec<f64> = abs
                .into_iter()
                .map(lufs)
                .filter(|&l| l > threshold)
                .collect();
            l.sort_by(f64::total_cmp);
            (!l.is_empty()).then(|| {
                let at = |p: f64| l[((l.len() - 1) as f64 * p).round() as usize];
                at(0.95) - at(0.10)
            })
        }
    };
    Loudness {
        integrated: gated_mean(&momentary, -10.0).map(lufs),
        momentary_max: max(&momentary),
        short_term_max: max(&short),
        range,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, amp: f64, secs: f64, sr: u32) -> Vec<f32> {
        (0..(secs * f64::from(sr)) as usize)
            .map(|i| (amp * libm::sin(2.0 * PI * hz * i as f64 / f64::from(sr))) as f32)
            .collect()
    }

    /// EBU Tech 3341 case 1: a 1 kHz stereo sine at -23 dBFS reads -23.0 LUFS.
    #[test]
    fn a_1khz_sine_at_minus_23_dbfs_is_minus_23_lufs() {
        for sr in [44_100, 48_000, 96_000] {
            let s = sine(1000.0, libm::pow(10.0, -23.0 / 20.0), 20.0, sr);
            let l = loudness(&[&s, &s], sr);
            let i = l.integrated.unwrap();
            assert!((i + 23.0).abs() < 0.1, "{sr} Hz: {i}");
            assert!((l.short_term_max.unwrap() + 23.0).abs() < 0.1);
            assert!(l.range.unwrap() < 0.1);
            // One channel is 3 dB quieter.
            let mono = loudness(&[&s], sr).integrated.unwrap();
            assert!((mono + 26.01).abs() < 0.1, "{mono}");
        }
    }

    /// EBU Tech 3341 case 3: -36, -23, -36 dBFS for 10, 60, 10 s gates to -23 LUFS.
    #[test]
    fn the_relative_gate_ignores_quiet_passages() {
        let sr = 48_000;
        let mut s = sine(1000.0, libm::pow(10.0, -36.0 / 20.0), 10.0, sr);
        s.extend(sine(1000.0, libm::pow(10.0, -23.0 / 20.0), 60.0, sr));
        s.extend(sine(1000.0, libm::pow(10.0, -36.0 / 20.0), 10.0, sr));
        let i = loudness(&[&s, &s], sr).integrated.unwrap();
        assert!((i + 23.0).abs() < 0.1, "{i}");
        assert_eq!(loudness(&[&vec![0.0; 48_000]], sr).integrated, None);
    }
}
