//! A/B comparison of two renders, sample by sample.

use crate::loudness::loudness;
use crate::peak::db;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Comparison {
    pub identical: bool,
    pub frames: (usize, usize),
    pub sample_rates: (u32, u32),
    /// Largest absolute sample difference, and in dBFS.
    pub max_difference: f64,
    pub max_difference_db: Option<f64>,
    /// RMS of the difference signal, dBFS.
    pub difference_rms_db: Option<f64>,
    /// First and last differing frame (over the common length).
    pub first_difference: Option<usize>,
    pub last_difference: Option<usize>,
    /// Pearson correlation of the mono mixes over the common length.
    pub correlation: Option<f64>,
    pub loudness: (Option<f64>, Option<f64>),
}

fn refs(x: &[Vec<f32>]) -> Vec<&[f32]> {
    x.iter().map(Vec::as_slice).collect()
}

/// Compares `a` and `b`, each a list of channels.
pub fn compare(a: &[Vec<f32>], sr_a: u32, b: &[Vec<f32>], sr_b: u32) -> Comparison {
    let len = |c: &[Vec<f32>]| c.first().map_or(0, Vec::len);
    let (na, nb) = (len(a), len(b));
    let n = na.min(nb);
    let channels = a.len().min(b.len());
    let (mut max, mut sum2, mut count) = (0.0f64, 0.0f64, 0usize);
    let (mut first, mut last) = (None, None);
    for i in 0..n {
        for c in 0..channels {
            let d = f64::from(a[c][i]) - f64::from(b[c][i]);
            if d != 0.0 {
                first.get_or_insert(i);
                last = Some(i);
            }
            max = max.max(d.abs());
            sum2 += d * d;
            count += 1;
        }
    }
    let mono = |x: &[Vec<f32>], i: usize| -> f64 {
        x.iter().map(|c| f64::from(c[i])).sum::<f64>() / x.len().max(1) as f64
    };
    let correlation = (n > 1 && channels > 0).then(|| {
        let (ma, mb) = (
            (0..n).map(|i| mono(a, i)).sum::<f64>() / n as f64,
            (0..n).map(|i| mono(b, i)).sum::<f64>() / n as f64,
        );
        let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
        for i in 0..n {
            let (x, y) = (mono(a, i) - ma, mono(b, i) - mb);
            sab += x * y;
            saa += x * x;
            sbb += y * y;
        }
        if saa > 0.0 && sbb > 0.0 {
            sab / libm::sqrt(saa * sbb)
        } else if saa == 0.0 && sbb == 0.0 {
            1.0
        } else {
            0.0
        }
    });
    let r = |x: Option<f64>, d: f64| x.map(|v| libm::round(v * d) / d);
    Comparison {
        identical: na == nb && sr_a == sr_b && a.len() == b.len() && first.is_none(),
        frames: (na, nb),
        sample_rates: (sr_a, sr_b),
        max_difference: max,
        max_difference_db: db(max),
        difference_rms_db: (count > 0)
            .then(|| libm::sqrt(sum2 / count as f64))
            .and_then(db),
        first_difference: first,
        last_difference: last,
        correlation: r(correlation, 1e6),
        loudness: (
            r(loudness(&refs(a), sr_a).integrated, 100.0),
            r(loudness(&refs(b), sr_b).integrated, 100.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences_are_located() {
        let a = vec![vec![0.5f32; 10_000], vec![0.25f32; 10_000]];
        let same = compare(&a, 48_000, &a, 48_000);
        assert!(same.identical && same.first_difference.is_none());
        let mut b = a.clone();
        b[1][1000] = 0.0;
        b[0][7000] = 0.75;
        let c = compare(&a, 48_000, &b, 48_000);
        assert!(!c.identical);
        assert_eq!(
            (c.first_difference, c.last_difference),
            (Some(1000), Some(7000))
        );
        assert_eq!(c.max_difference, 0.25);
    }
}
