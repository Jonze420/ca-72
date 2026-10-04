//! Oversampling factors the resamplers do not support are refused, not rounded.

use ca72::resample::{Decimator, Interpolator};

#[test]
fn supported_factors_build() {
    for f in [1, 2, 4, 8] {
        assert_eq!(Interpolator::new(f).factor(), f);
        let _ = Decimator::new(f);
    }
}

#[test]
#[should_panic(expected = "oversampling factor 16")]
fn decimator_refuses_sixteen() {
    let _ = Decimator::new(16);
}

#[test]
#[should_panic(expected = "oversampling factor 3")]
fn interpolator_refuses_three() {
    let _ = Interpolator::new(3);
}

/// Sparse halfbands (Potato's) against the full ones with the same taps zeroed: the same
/// outputs to rounding, at every factor, decimating and interpolating a noisy signal.
#[test]
fn sparse_halfbands_match_the_full_ones() {
    use ca72::resample::{Decimator, Interpolator};
    let mut seed = 1u64;
    let mut noise = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let x: Vec<f64> = (0..4000).map(|_| noise()).collect();
    for f in [2, 4, 8] {
        let (mut a, mut b) = (Decimator::new(f), Decimator::sparse(f));
        let mut worst = 0.0f64;
        for &v in &x {
            if let (Some(p), Some(q)) = (a.push(v), b.push(v)) {
                worst = worst.max((p - q).abs());
            }
        }
        let (mut a, mut b) = (Interpolator::new(f), Interpolator::sparse(f));
        let (mut pa, mut pb) = ([0.0; 8], [0.0; 8]);
        for &v in &x {
            a.push(v, &mut pa);
            b.push(v, &mut pb);
            for k in 0..f {
                worst = worst.max((pa[k] - pb[k]).abs());
            }
        }
        // (The full ones' near-zero taps are about 1e-17 of the rest.)
        assert!(worst < 1e-14, "factor {f}: {worst:e}");
    }
}
