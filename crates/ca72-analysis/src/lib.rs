//! Listening, for the lab and the tests: measurements of a rendered signal (taken from
//! the analysis library of the DAW the model was developed in, without its
//! device-emulation measurements).
//!
//! - loudness per ITU-R BS.1770-4 / EBU R 128 ([`loudness`]),
//! - sample and true peak ([`peak`]),
//! - a spectrum summary ([`spectrum`]),
//! - onsets ([`onsets`]),
//! - a pitch track ([`pitch`]),
//! - A/B comparison of two renders ([`compare`]).
//!
//! Transcendental functions come from `libm`, so results are the same on every platform.

pub mod compare;
pub mod fft;
pub mod loudness;
pub mod onsets;
pub mod peak;
pub mod pitch;
pub mod spectrum;
pub mod wav;

use serde::Serialize;

/// Everything [`analyze`] measures.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Analysis {
    pub sample_rate: u32,
    pub channels: usize,
    pub seconds: f64,
    pub loudness: loudness::Loudness,
    pub sample_peak_db: Option<f64>,
    pub true_peak_db: Option<f64>,
    pub rms_db: Option<f64>,
    pub spectrum: spectrum::Spectrum,
    /// Onset times, seconds (at most 500).
    pub onsets: Vec<f64>,
    pub pitch: pitch::Pitch,
}

fn round(x: f64, digits: i32) -> f64 {
    let m = libm::pow(10.0, f64::from(digits));
    libm::round(x * m) / m
}

/// Measures a signal given as channels of equal length.
pub fn analyze(channels: &[Vec<f32>], sample_rate: u32) -> Analysis {
    let refs: Vec<&[f32]> = channels.iter().map(Vec::as_slice).collect();
    let n = channels.first().map_or(0, Vec::len);
    let mono: Vec<f64> = (0..n)
        .map(|i| {
            channels.iter().map(|c| f64::from(c[i])).sum::<f64>() / channels.len().max(1) as f64
        })
        .collect();
    let mean_square = if n == 0 || channels.is_empty() {
        0.0
    } else {
        channels
            .iter()
            .flat_map(|c| c.iter())
            .map(|v| f64::from(*v) * f64::from(*v))
            .sum::<f64>()
            / (n * channels.len()) as f64
    };
    let r2 = |x: Option<f64>| x.map(|v| round(v, 2));
    let l = loudness::loudness(&refs, sample_rate);
    let mut sp = spectrum::spectrum(&mono, sample_rate);
    sp.centroid_hz = r2(sp.centroid_hz);
    sp.peak_hz = r2(sp.peak_hz);
    for b in &mut sp.bands {
        b.db = r2(b.db);
    }
    let mut pitch = pitch::pitch(&mono, sample_rate);
    pitch.median_hz = r2(pitch.median_hz);
    for p in &mut pitch.track {
        p.0 = round(p.0, 3);
        p.1 = r2(p.1);
        p.2 = round(p.2, 3);
    }
    let mut onsets: Vec<f64> = onsets::onsets(&mono, sample_rate)
        .into_iter()
        .map(|t| round(t, 3))
        .collect();
    onsets.truncate(500);
    Analysis {
        sample_rate,
        channels: channels.len(),
        seconds: round(n as f64 / f64::from(sample_rate.max(1)), 3),
        loudness: loudness::Loudness {
            integrated: r2(l.integrated),
            momentary_max: r2(l.momentary_max),
            short_term_max: r2(l.short_term_max),
            range: r2(l.range),
        },
        sample_peak_db: r2(peak::db(peak::sample_peak(&refs))),
        true_peak_db: r2(peak::db(peak::true_peak(&refs))),
        rms_db: r2((mean_square > 0.0).then(|| 10.0 * libm::log10(mean_square))),
        spectrum: sp,
        onsets,
        pitch,
    }
}
