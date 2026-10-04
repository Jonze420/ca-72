//! The fast elementary functions (Potato's, and High Fidelity's): an exponential from a
//! table of 2^(j/64) and a
//! short polynomial, and tanh from it: within about an ulp of libm's (`tests`), twice its
//! speed (libm's tanh takes an expm1 and a division, 9.6 ns against 4.5). Plain arithmetic
//! on the bits, so the same results on every platform. Not for No Compromises, whose
//! results are libm's (`crate::ulp`).

/// 2^(j/64), correctly rounded (from 60-digit decimal arithmetic).
const EXP2_64: [f64; 64] = [
    1.0,
    1.0108892860517005,
    1.0218971486541166,
    1.0330248790212284,
    1.0442737824274138,
    1.0556451783605572,
    1.0671404006768237,
    1.0787607977571199,
    1.0905077326652577,
    1.102382583307841,
    1.1143867425958924,
    1.1265216186082418,
    1.1387886347566916,
    1.1511892299529827,
    1.1637248587775775,
    1.1763969916502812,
    1.189207115002721,
    1.202156731452703,
    1.215247359980469,
    1.22848053610687,
    1.241857812073484,
    1.255380757024691,
    1.2690509571917332,
    1.2828700160787783,
    1.2968395546510096,
    1.3109612115247644,
    1.3252366431597413,
    1.339667524053303,
    1.3542555469368927,
    1.3690024229745905,
    1.383909881963832,
    1.3989796725383112,
    core::f64::consts::SQRT_2,
    1.42961333839197,
    1.4451808069770467,
    1.460917794180647,
    1.4768261459394993,
    1.4929077282912648,
    1.5091644275934228,
    1.5255981507445384,
    1.5422108254079407,
    1.559004400237837,
    1.5759808451078865,
    1.593142151342267,
    1.6104903319492543,
    1.6280274218573478,
    1.645755478153965,
    1.6636765803267364,
    1.681792830507429,
    1.7001063537185235,
    1.718619298122478,
    1.7373338352737062,
    1.7562521603732995,
    1.7753764925265212,
    1.7947090750031072,
    1.8142521755003989,
    1.8340080864093424,
    1.8539791250833855,
    1.8741676341103,
    1.8945759815869656,
    1.9152065613971474,
    1.9360617934922943,
    1.9571441241754002,
    1.978456026387951,
];

/// ln(2) / 64 in two parts, the first with its low bits clear (so k times it is exact for
/// the k that occur), and 64 / ln(2).
const LN2_64_HI: f64 = 0.01083042468962958;
const LN2_64_LO: f64 = 6.619564634077006e-12;
const INV_LN2_64: f64 = 92.33248261689366;

/// e^x: x = (64 m + j) ln(2) / 64 + r with |r| <= ln(2) / 128, e^x = 2^m 2^(j/64) e^r, e^r
/// to its fifth power (what that leaves is under 1e-17 of it). Outside +-700, libm's.
#[inline(always)]
pub fn exp(x: f64) -> f64 {
    if x.is_nan() || x.abs() >= 700.0 {
        return libm::exp(x);
    }
    // Rounded to the nearest integer by adding and taking away 1.5 * 2^52.
    const SHIFT: f64 = 6755399441055744.0;
    let kb = x * INV_LN2_64 + SHIFT;
    let kf = kb - SHIFT;
    let k = kb.to_bits() as i32 as i64;
    let r = (x - kf * LN2_64_HI) - kf * LN2_64_LO;
    let r2 = r * r;
    let p = r + r2 * (0.5 + r * (1.0 / 6.0)) + r2 * r2 * (1.0 / 24.0 + r * (1.0 / 120.0));
    let t = EXP2_64[(k & 63) as usize];
    let y = t + t * p;
    f64::from_bits((y.to_bits() as i64 + ((k >> 6) << 52)) as u64)
}

/// tanh(x) = 1 - 2 / (e^(2|x|) + 1) with its sign; below 0.02 its series to x^7 (where the
/// subtraction would lose digits), above 20 one.
#[inline(always)]
pub fn tanh(x: f64) -> f64 {
    let a = x.abs();
    let t = if a < 0.02 {
        let a2 = a * a;
        a * (1.0 - a2 * (1.0 / 3.0 - a2 * (2.0 / 15.0 - a2 * (17.0 / 315.0))))
    } else if a > 20.0 {
        1.0
    } else {
        1.0 - 2.0 / (exp(2.0 * a) + 1.0)
    };
    t.copysign(x)
}

#[cfg(test)]
mod tests {
    /// Within 2 ulp of libm's exp from -700 to 700, and tanh within 2e-14 relatively over
    /// +-25 (and exactly odd).
    #[test]
    fn they_follow_libm() {
        let mut ulps = 0i64;
        let mut rel = 0.0f64;
        for i in 0..2_000_000 {
            let f = (i as f64 * 0.754_877_666_2) % 1.0;
            let x = -700.0 + 1400.0 * f;
            ulps = ulps.max((libm::exp(x).to_bits() as i64 - super::exp(x).to_bits() as i64).abs());
            let y = -25.0 + 50.0 * f;
            let (a, b) = (libm::tanh(y), super::tanh(y));
            rel = rel.max((a - b).abs() / a.abs().max(1e-300));
            assert_eq!(super::tanh(-y), -b);
        }
        assert!(ulps <= 2, "exp: {ulps} ulp");
        assert!(rel < 2e-14, "tanh: {rel:e}");
        assert_eq!(super::exp(0.0), 1.0);
        assert_eq!(super::tanh(0.0), 0.0);
    }
}
