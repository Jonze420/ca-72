//! Sample peak and true peak (ITU-R BS.1770-4 Annex 2): the signal oversampled four
//! times with a windowed-sinc interpolator, so peaks between samples count.

use std::f64::consts::PI;

const PHASES: usize = 4;
const TAPS: usize = 12;

/// Interpolation filter taps for each of the four phases.
fn filter() -> [[f64; TAPS]; PHASES] {
    let n = PHASES * TAPS;
    let centre = (n - 1) as f64 / 2.0;
    let mut h = [[0.0; TAPS]; PHASES];
    for m in 0..n {
        let t = (m as f64 - centre) / PHASES as f64;
        let sinc = if t == 0.0 {
            1.0
        } else {
            libm::sin(PI * t) / (PI * t)
        };
        // Blackman-Harris window.
        let x = 2.0 * PI * m as f64 / (n - 1) as f64;
        let w = 0.35875 - 0.48829 * libm::cos(x) + 0.14128 * libm::cos(2.0 * x)
            - 0.01168 * libm::cos(3.0 * x);
        h[m % PHASES][m / PHASES] = sinc * w;
    }
    // Unity gain per phase.
    for p in &mut h {
        let s: f64 = p.iter().sum();
        p.iter_mut().for_each(|v| *v /= s);
    }
    h
}

pub fn sample_peak(channels: &[&[f32]]) -> f64 {
    channels
        .iter()
        .flat_map(|c| c.iter())
        .fold(0.0f64, |m, v| m.max(f64::from(v.abs())))
}

pub fn true_peak(channels: &[&[f32]]) -> f64 {
    let h = filter();
    let mut peak = sample_peak(channels);
    for c in channels {
        let x: Vec<f64> = c.iter().map(|v| f64::from(*v)).collect();
        for n in 0..x.len() + TAPS {
            for phase in &h {
                let mut y = 0.0;
                for (j, tap) in phase.iter().enumerate() {
                    if let Some(v) = n.checked_sub(j).and_then(|i| x.get(i)) {
                        y += tap * v;
                    }
                }
                peak = peak.max(y.abs());
            }
        }
    }
    peak
}

/// Decibels relative to full scale; `None` for silence.
pub fn db(x: f64) -> Option<f64> {
    (x > 0.0).then(|| 20.0 * libm::log10(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sine at a quarter of the sample rate, 45 degrees out: every sample is at 0.707 of
    /// the true peak.
    #[test]
    fn peaks_between_samples_are_found() {
        let s: Vec<f32> = (0..4800)
            .map(|i| (0.5 * libm::sin(PI / 2.0 * i as f64 + PI / 4.0)) as f32)
            .collect();
        let sp = sample_peak(&[&s]);
        let tp = true_peak(&[&s]);
        assert!((sp - 0.353_55).abs() < 1e-4, "{sp}");
        assert!((db(tp).unwrap() - db(0.5).unwrap()).abs() < 0.2, "{tp}");
    }
}
