//! An in-place radix-2 FFT, enough for analysis frames.

use std::f64::consts::PI;

/// Transforms `re` + i`im` in place. The length must be a power of two.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        for k in 0..half {
            let ang = -2.0 * PI * k as f64 / len as f64;
            let (wr, wi) = (libm::cos(ang), libm::sin(ang));
            for start in (0..n).step_by(len) {
                let (a, b) = (start + k, start + k + half);
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// A Hann window of `n` points.
pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 - 0.5 * libm::cos(2.0 * PI * i as f64 / n as f64))
        .collect()
}

/// Power spectrum (|X|², bins 0..=n/2) of a windowed real frame.
pub fn power(frame: &[f64], window: &[f64]) -> Vec<f64> {
    let n = frame.len();
    let mut re: Vec<f64> = frame.iter().zip(window).map(|(x, w)| x * w).collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    (0..=n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sine_lands_in_its_bin() {
        let n = 64;
        let mut re: Vec<f64> = (0..n)
            .map(|i| libm::sin(2.0 * PI * 5.0 * i as f64 / n as f64))
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mag: Vec<f64> = (0..n)
            .map(|k| libm::sqrt(re[k] * re[k] + im[k] * im[k]))
            .collect();
        assert!((mag[5] - 32.0).abs() < 1e-9 && (mag[59] - 32.0).abs() < 1e-9);
        assert!(
            mag.iter()
                .enumerate()
                .all(|(k, m)| k == 5 || k == 59 || *m < 1e-9)
        );
    }
}
