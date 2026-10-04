//! Onset detection by spectral flux: the rise of the log-compressed magnitude spectrum
//! from frame to frame, picked where it peaks above a running median.

use crate::fft::{hann, power};

const N: usize = 512;
const HOP: usize = 128;

/// Onset times in seconds.
pub fn onsets(mono: &[f64], sample_rate: u32) -> Vec<f64> {
    let sr = f64::from(sample_rate);
    let w = hann(N);
    let mut prev: Option<Vec<f64>> = None;
    let mut flux = Vec::new();
    let mut frame = vec![0.0; N];
    let mut start = 0usize;
    // Frames start half a window early so the first samples are covered.
    let offset = N / 2;
    while start < mono.len() + offset {
        for (i, f) in frame.iter_mut().enumerate() {
            *f = (start + i)
                .checked_sub(offset)
                .and_then(|j| mono.get(j))
                .copied()
                .unwrap_or(0.0);
        }
        let mag: Vec<f64> = power(&frame, &w)
            .iter()
            .map(|p| libm::log1p(100.0 * libm::sqrt(*p)))
            .collect();
        let f = match &prev {
            Some(p) => mag.iter().zip(p).map(|(a, b)| (a - b).max(0.0)).sum(),
            None => 0.0,
        };
        flux.push(f);
        prev = Some(mag);
        start += HOP;
    }
    let max = flux.iter().copied().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return Vec::new();
    }
    let min_gap = (0.05 * sr / HOP as f64).ceil() as usize;
    let mut out: Vec<f64> = Vec::new();
    let mut last: Option<usize> = None;
    for t in 1..flux.len() {
        let lo = t.saturating_sub(16);
        let hi = (t + 16).min(flux.len());
        let mut win: Vec<f64> = flux[lo..hi].to_vec();
        win.sort_by(f64::total_cmp);
        let median = win[win.len() / 2];
        let is_peak = flux[t] >= flux[t - 1] && flux.get(t + 1).is_none_or(|n| flux[t] > *n);
        if is_peak && flux[t] > 1.5 * median + 0.1 * max && last.is_none_or(|l| t - l >= min_gap) {
            last = Some(t);
            // The frame is centred on `t * HOP - offset + N / 2 = t * HOP`.
            out.push((t * HOP) as f64 / sr);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_are_found_at_their_times() {
        let sr = 48_000;
        let mut s = vec![0.0; 3 * sr as usize];
        for t in [0.5, 1.25, 2.0] {
            let at = (t * f64::from(sr)) as usize;
            for (k, v) in s[at..at + 48].iter_mut().enumerate() {
                *v = 0.8 * (1.0 - k as f64 / 48.0);
            }
        }
        let found = onsets(&s, sr);
        assert_eq!(found.len(), 3, "{found:?}");
        for (f, t) in found.iter().zip([0.5, 1.25, 2.0]) {
            assert!((f - t).abs() < 0.01, "{f} vs {t}");
        }
    }
}
