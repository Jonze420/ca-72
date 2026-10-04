//! Pitch tracking with YIN (de Cheveigné and Kawahara, 2002): the cumulative mean
//! normalised difference function, computed through an FFT autocorrelation.

use crate::fft::fft;
use serde::Serialize;

const W: usize = 2048;
const HOP: usize = 1024;
const THRESHOLD: f64 = 0.15;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pitch {
    /// Median fundamental of the voiced frames.
    pub median_hz: Option<f64>,
    /// The nearest note and its offset in cents.
    pub note: Option<String>,
    /// (seconds, Hz or `None` when unvoiced, confidence 0..1), at most 200 points.
    pub track: Vec<(f64, Option<f64>, f64)>,
}

fn note_name(hz: f64) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let midi = 69.0 + 12.0 * libm::log2(hz / 440.0);
    let n = midi.round();
    let cents = ((midi - n) * 100.0).round() as i64;
    let octave = (n as i64).div_euclid(12) - 1;
    format!(
        "{}{octave} {cents:+} cents",
        NAMES[(n as i64).rem_euclid(12) as usize]
    )
}

fn frame_pitch(x: &[f64], sr: f64) -> (Option<f64>, f64) {
    // r(tau) = sum_{j<W} x[j] x[j + tau], via FFT of size 4W.
    let n = 4 * W;
    let mut ar = vec![0.0; n];
    let mut ai = vec![0.0; n];
    let mut br = vec![0.0; n];
    let mut bi = vec![0.0; n];
    ar[..W].copy_from_slice(&x[..W]);
    br[..2 * W].copy_from_slice(&x[..2 * W]);
    fft(&mut ar, &mut ai);
    fft(&mut br, &mut bi);
    // conj(A) * B, then inverse via the forward transform of the conjugate.
    let mut cr: Vec<f64> = (0..n).map(|k| ar[k] * br[k] + ai[k] * bi[k]).collect();
    let mut ci: Vec<f64> = (0..n).map(|k| -(ar[k] * bi[k] - ai[k] * br[k])).collect();
    fft(&mut cr, &mut ci);
    let r: Vec<f64> = cr.iter().map(|v| v / n as f64).collect();
    let mut prefix = vec![0.0; 2 * W + 1];
    for (i, v) in x[..2 * W].iter().enumerate() {
        prefix[i + 1] = prefix[i] + v * v;
    }
    let e0 = prefix[W];
    let max_tau = W;
    let mut d = vec![0.0; max_tau];
    for tau in 1..max_tau {
        let e = prefix[tau + W] - prefix[tau];
        d[tau] = e0 + e - 2.0 * r[tau];
    }
    // Cumulative mean normalised difference.
    let mut dn = vec![1.0; max_tau];
    let mut sum = 0.0;
    for tau in 1..max_tau {
        sum += d[tau];
        dn[tau] = if sum > 0.0 {
            d[tau] * tau as f64 / sum
        } else {
            1.0
        };
    }
    let min_tau = (sr / 2000.0) as usize;
    let mut tau = None;
    let mut t = min_tau.max(2);
    while t < max_tau - 1 {
        if dn[t] < THRESHOLD {
            while t + 1 < max_tau - 1 && dn[t + 1] < dn[t] {
                t += 1;
            }
            tau = Some(t);
            break;
        }
        t += 1;
    }
    let Some(t) = tau else {
        let best = dn[min_tau.max(2)..].iter().copied().fold(1.0f64, f64::min);
        return (None, (1.0 - best).clamp(0.0, 1.0));
    };
    let (a, b, c) = (dn[t - 1], dn[t], dn[t + 1]);
    let den = a - 2.0 * b + c;
    let off = if den.abs() > 1e-12 {
        0.5 * (a - c) / den
    } else {
        0.0
    };
    (Some(sr / (t as f64 + off)), (1.0 - b).clamp(0.0, 1.0))
}

pub fn pitch(mono: &[f64], sample_rate: u32) -> Pitch {
    let sr = f64::from(sample_rate);
    let mut track = Vec::new();
    let mut start = 0;
    while start + 2 * W <= mono.len() {
        let (f, c) = frame_pitch(&mono[start..start + 2 * W], sr);
        track.push(((start + W / 2) as f64 / sr, f, c));
        start += HOP;
    }
    let mut voiced: Vec<f64> = track.iter().filter_map(|(_, f, _)| *f).collect();
    voiced.sort_by(f64::total_cmp);
    let median_hz = (!voiced.is_empty()).then(|| voiced[voiced.len() / 2]);
    if track.len() > 200 {
        let step = track.len().div_ceil(200);
        track = track.into_iter().step_by(step).collect();
    }
    Pitch {
        median_hz,
        note: median_hz.map(note_name),
        track,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn a_sine_and_a_saw_have_their_pitch() {
        let sr = 48_000;
        let sine: Vec<f64> = (0..48_000)
            .map(|i| 0.5 * libm::sin(2.0 * PI * 440.0 * i as f64 / 48_000.0))
            .collect();
        let p = pitch(&sine, sr);
        assert!(
            (p.median_hz.unwrap() - 440.0).abs() < 0.5,
            "{:?}",
            p.median_hz
        );
        assert_eq!(p.note.as_deref(), Some("A4 +0 cents"));
        let saw: Vec<f64> = (0..48_000)
            .map(|i| {
                let ph = (220.0 * i as f64 / 48_000.0).fract();
                0.5 * (2.0 * ph - 1.0)
            })
            .collect();
        let p = pitch(&saw, sr);
        assert!(
            (p.median_hz.unwrap() - 220.0).abs() < 0.5,
            "{:?}",
            p.median_hz
        );
        assert_eq!(pitch(&vec![0.0; 48_000], sr).median_hz, None);
    }
}
