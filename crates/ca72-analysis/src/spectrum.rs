//! A spectrum summary: Welch-averaged power spectrum of the mono mix, reported as octave
//! band levels, the spectral centroid and the strongest frequency.

use crate::fft::{hann, power};
use serde::Serialize;

const N: usize = 8192;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Band {
    /// Centre frequency.
    pub hz: f64,
    /// RMS level of the band, dBFS; `None` if silent.
    pub db: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Spectrum {
    pub bands: Vec<Band>,
    pub centroid_hz: Option<f64>,
    pub peak_hz: Option<f64>,
}

pub const OCTAVES: [f64; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub fn spectrum(mono: &[f64], sample_rate: u32) -> Spectrum {
    let sr = f64::from(sample_rate);
    let w = hann(N);
    let norm: f64 = w.iter().map(|v| v * v).sum::<f64>() * N as f64;
    let mut acc = vec![0.0; N / 2 + 1];
    let mut frames = 0;
    let mut start = 0;
    let mut frame = vec![0.0; N];
    loop {
        for (i, f) in frame.iter_mut().enumerate() {
            *f = mono.get(start + i).copied().unwrap_or(0.0);
        }
        for (a, p) in acc.iter_mut().zip(power(&frame, &w)) {
            *a += p;
        }
        frames += 1;
        start += N / 2;
        if start + N / 2 >= mono.len().max(1) {
            break;
        }
    }
    // One-sided power, scaled so bins sum to the mean square.
    let p: Vec<f64> = acc
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let one_sided = if k == 0 || k == N / 2 { 1.0 } else { 2.0 };
            a / frames as f64 / norm * one_sided
        })
        .collect();
    let bin = sr / N as f64;
    let bands = OCTAVES
        .iter()
        .map(|&c| {
            let (lo, hi) = (c / std::f64::consts::SQRT_2, c * std::f64::consts::SQRT_2);
            let e: f64 = p
                .iter()
                .enumerate()
                .filter(|(k, _)| {
                    let f = *k as f64 * bin;
                    f >= lo && f < hi
                })
                .map(|(_, v)| v)
                .sum();
            Band {
                hz: c,
                db: (e > 1e-20).then(|| 10.0 * libm::log10(e)),
            }
        })
        .collect();
    let total: f64 = p.iter().sum();
    let centroid_hz = (total > 1e-20).then(|| {
        p.iter()
            .enumerate()
            .map(|(k, v)| k as f64 * bin * v)
            .sum::<f64>()
            / total
    });
    let peak_hz = (total > 1e-20).then(|| {
        let k = p
            .iter()
            .enumerate()
            .skip(1)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(1, |(k, _)| k);
        // Parabolic interpolation on the log power around the peak.
        let (a, b, c) = (
            libm::log(p[k - 1].max(1e-30)),
            libm::log(p[k].max(1e-30)),
            libm::log(p.get(k + 1).copied().unwrap_or(1e-30).max(1e-30)),
        );
        let d = a - 2.0 * b + c;
        let off = if d.abs() > 1e-12 {
            0.5 * (a - c) / d
        } else {
            0.0
        };
        (k as f64 + off) * bin
    });
    Spectrum {
        bands,
        centroid_hz,
        peak_hz,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn a_1khz_sine_is_one_band_and_one_peak() {
        let s: Vec<f64> = (0..96_000)
            .map(|i| 0.5 * libm::sin(2.0 * PI * 1000.0 * i as f64 / 48_000.0))
            .collect();
        let sp = spectrum(&s, 48_000);
        assert!(
            (sp.peak_hz.unwrap() - 1000.0).abs() < 1.0,
            "{:?}",
            sp.peak_hz
        );
        let band = |hz: f64| sp.bands.iter().find(|b| b.hz == hz).unwrap().db.unwrap();
        // RMS of a 0.5 sine: -9.03 dBFS.
        assert!((band(1000.0) + 9.03).abs() < 0.2, "{}", band(1000.0));
        assert!(band(125.0) < -60.0 && band(8000.0) < -60.0);
        assert!((sp.centroid_hz.unwrap() - 1000.0).abs() < 20.0);
    }
}
