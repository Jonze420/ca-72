//! Board 1's exponential converter: the control summer IC1, the scale divider, the CA3046
//! pair and IC3's reference loop (docs/circuit/board1.md, items 1-3), solved from the
//! circuit's own equations. Values and reference designators are oscillator 1's
//! (Figure 9-3); oscillators 2 and 3 have the same parts under other designators.
//!
//! What it computes: the exponential transistor's collector current, which charges the
//! timing capacitor, for given input voltages, ramp voltage and temperature. Included,
//! because they move the pitch by more than 0.1 cent somewhere in the instrument's range:
//! the op-amps' input bias currents, the reference transistor's base current loading the
//! scale divider, the exponential transistor's base current through R7, the transistors'
//! base and emitter resistance, R42's feedback of the tail current, the Early effect on
//! both collectors (the ramp's curvature), R20's temperature coefficient. Left out: the
//! op-amps' offset voltages and finite gain (sub-0.01-cent at DC in the Boyle model), the
//! dynamics of the control path (C2, the op-amps' bandwidth; to be measured).

use crate::devices::{Bjt, CA3046};

/// The parts of one oscillator's converter (oscillator 1's designators).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpoCircuit {
    /// R20 (1K, temperature compensating) at 25 C, and its coefficient per C.
    pub r20: f64,
    pub tc20: f64,
    /// R11 (the "range" trimpot, 0..1K rheostat) and R37 (11.8K) to -10 V.
    pub r11: f64,
    pub r37: f64,
    /// R42 (160K), tail emitter to the summing junction.
    pub r42: f64,
    /// R9 (33), R8 (100, "scale" trimpot) with its wiper at `a8` from the R19 end, R19 (1K).
    pub r9: f64,
    pub r8: f64,
    pub a8: f64,
    pub r19: f64,
    /// R7 (100): the exponential transistor's base to -5 V.
    pub r7: f64,
    /// R50 (39K): the reference current, from GND to the reference collector.
    pub r50: f64,
    /// R79 (1K): the tail's emitter to -10 V.
    pub r79: f64,
    /// The -5 V reference line, the "-4V" line (as loaded) and the -10 V rail.
    pub m5: f64,
    pub m4: f64,
    pub vn: f64,
    /// R10 (1K): IC1's non-inverting input to the -5 V line.
    pub r10: f64,
    /// Another source on IC1's non-inverting input, as a Thevenin voltage and conductance
    /// (oscillator 2's FREQUENCY network; 0 S for none).
    pub pos_v: f64,
    pub pos_g: f64,
    /// Whether R11/R37 reach the summing junction (oscillator 3's hang on its switched
    /// input node, which OSC. 3 CONTROL off moves to the -5 V line).
    pub trim_on: bool,
    /// IC1's input offset, V: the summing junction above its + input by this much.
    pub vos: f64,
    /// The 741's input bias current (both inputs), A: its input pair's collector current
    /// over their beta in the Boyle model (7.58 uA / 93.75).
    pub bias: f64,
    pub bjt: Bjt,
}

impl Default for ExpoCircuit {
    /// Oscillator 1 as drawn, trimmers at mid travel.
    fn default() -> Self {
        ExpoCircuit {
            r20: 1000.0,
            tc20: 3.5e-3,
            r11: 500.0,
            r37: 11.8e3,
            r42: 160e3,
            r9: 33.0,
            r8: 100.0,
            a8: 0.5,
            r19: 1000.0,
            r7: 100.0,
            r50: 39e3,
            r79: 1000.0,
            // The reference lines as board 1's reference circuit makes them (the lab's
            // operating point): the -5 V line 19 uV above -5 V (IC9's offset in the 741
            // macromodel), R166/R174's -3.861 V loaded by the current sources' bases.
            m5: M5_LINE,
            m4: M4_LINE,
            vn: -10.0,
            r10: 1000.0,
            pos_v: 0.0,
            pos_g: 0.0,
            trim_on: true,
            vos: SUMMER_OFFSET,
            bias: 15.16e-6 / 2.0 / 93.75,
            bjt: CA3046,
        }
    }
}

/// The -5 V line and the "-4V" line as board 1's reference circuit makes them (the lab's
/// operating point; `vco_osc23.rs` measures them).
pub const M5_LINE: f64 = -4.999_981_03;
pub const M4_LINE: f64 = -3.867_06;

/// The control summers' (IC1, IC4, IC6) input offset in the 741 macromodel at their
/// operating point (+ input at -5 V on GND and -10 V supplies): the summing junction
/// 18.6 uV above the + input. Oscillators 1 and 2 are tuned with it and oscillator 3 with
/// OSC. 3 CONTROL on; with it off, it would move oscillator 3 by about a cent.
pub const SUMMER_OFFSET: f64 = 18.6e-6;

/// One summing input: its resistor and the voltage driving it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    pub r: f64,
    pub v: f64,
}

/// Oscillator 1's summing resistors (Figure 9-3): bend R12, tune R21, keyboard R27,
/// modulation R32, external R38, range R43.
pub const OSC1_INPUT_R: [f64; 6] = [150e3, 560e3, 51.1e3, 51.1e3, 51.1e3, 15e3];

/// The converter's solved state: the collector current and the node voltages the lab
/// compares with ngspice.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ExpoState {
    pub ic_exp: f64,
    pub ib_exp: f64,
    pub ic_ref: f64,
    pub ib_ref: f64,
    pub v_sum: f64,
    pub v_ic1: f64,
    pub vb_ref: f64,
    pub vb_exp: f64,
    pub v_sub: f64,
}

impl ExpoCircuit {
    /// The summing junction's voltage: IC1's + input, the -5 V line through R10 with any
    /// other source there, less its bias current.
    pub fn v_sum(&self) -> f64 {
        let g10 = 1.0 / self.r10;
        (self.m5 * g10 + self.pos_v * self.pos_g - self.bias) / (g10 + self.pos_g) + self.vos
    }

    /// The part of IC1's output voltage that the inputs' current `i_in`, the + input's
    /// source and the trimmer chain set, V: the converter's collector current depends on it
    /// and the ramp's voltage alone (the rest of IC1's output follows the tail's current),
    /// so one table in it serves every setting of the panel (Potato: [`ConverterTable`]).
    pub fn drive_u(&self, i_in: f64, celsius: f64) -> f64 {
        let r20 = self.r20 * (1.0 + self.tc20 * (celsius - 25.0));
        let v_sum = self.v_sum();
        let trim = if self.trim_on {
            (self.vn - v_sum) / (self.r11 + self.r37)
        } else {
            0.0
        };
        let fixed = i_in + trim - self.bias;
        let a = self.r79 / self.r42;
        v_sum - r20 * fixed - (r20 / self.r42) * ((self.vn + a * v_sum) / (1.0 + a) - v_sum)
    }

    /// The current the inputs send into the summing junction, A.
    pub fn input_current(&self, inputs: &[Input]) -> f64 {
        let v_sum = self.v_sum();
        inputs.iter().map(|i| (i.v - v_sum) / i.r).sum()
    }

    /// The exponential transistor's collector current, A, for the summing inputs, the ramp
    /// (its collector) voltage and the temperature in C.
    pub fn collector_current(&self, inputs: &[Input], v_ramp: f64, celsius: f64) -> f64 {
        self.solve(inputs, v_ramp, celsius).ic_exp
    }

    /// The converter's state for the summing inputs, ramp voltage and temperature.
    pub fn solve(&self, inputs: &[Input], v_ramp: f64, celsius: f64) -> ExpoState {
        self.solve_current(self.input_current(inputs), v_ramp, celsius, None)
    }

    /// The converter's state for the inputs' total current into the summing junction,
    /// starting the iteration from `guess` (the previous sample's current) when given.
    pub fn solve_current(
        &self,
        i_in: f64,
        v_ramp: f64,
        celsius: f64,
        guess: Option<f64>,
    ) -> ExpoState {
        let b = &self.bjt;
        let bt = b.at(celsius);
        let vt = bt.vt;
        let r20 = self.r20 * (1.0 + self.tc20 * (celsius - 25.0));
        let v_sum = self.v_sum();
        // IC3 holds the reference collector at the -4 V line; its + input takes its bias.
        let ic_ref = -self.m4 / self.r50 - self.bias;
        // The divider's Thevenin equivalent at R8's wiper.
        let r_top = self.r9 + self.r8 * (1.0 - self.a8);
        let r_bot = self.r8 * self.a8 + self.r19;
        let k = r_bot / (r_top + r_bot);
        let r_th = r_top * r_bot / (r_top + r_bot);
        let trim = if self.trim_on {
            (self.vn - v_sum) / (self.r11 + self.r37)
        } else {
            0.0
        };
        let fixed = i_in + trim - self.bias;
        let mut st = ExpoState {
            ic_exp: guess.unwrap_or(ic_ref),
            ic_ref,
            v_sum,
            vb_ref: self.m5,
            vb_exp: self.m5,
            ..ExpoState::default()
        };
        let mut converged = false;
        for _ in 0..16 {
            let x = st.ic_exp;
            let ib_ref = bt.base_current(ic_ref, st.vb_ref - self.m4, b.vaf);
            let ib_exp = bt.base_current(x, st.vb_exp - v_ramp, b.vaf);
            let ie_ref = ic_ref + ib_ref;
            let ie_exp = x + ib_exp;
            let ic_tail = ie_exp + ie_ref;
            // The tail's collector sits at the pair's emitters, about -5.7 V.
            let ie_tail = ic_tail + bt.base_current(ic_tail, -9.1 + 5.7, b.vaf);
            // R79 carries the tail's emitter current and R42's current (sum -> tail node).
            let a = self.r79 / self.r42;
            let v_sub = (self.vn + self.r79 * ie_tail + a * v_sum) / (1.0 + a);
            let sum = fixed + (v_sub - v_sum) / self.r42;
            let v_ic1 = v_sum - r20 * sum;
            let vb_ref = self.m5 + (v_ic1 - self.m5) * k - r_th * ib_ref;
            let vb_exp = self.m5 - self.r7 * ib_exp;
            // Intrinsic base-emitter voltages differ by the external bases' difference less
            // the drops in RB and RE.
            let dvbe =
                (vb_exp - b.rb * ib_exp - b.re * ie_exp) - (vb_ref - b.rb * ib_ref - b.re * ie_ref);
            // Early effect: Ic grows as 1 - Vbc/VAF (Gummel-Poon's q1, VAR infinite).
            let vbc_exp = vb_exp - b.rb * ib_exp - v_ramp;
            let vbc_ref = vb_ref - b.rb * ib_ref - self.m4;
            let next = ic_ref * crate::ulp::exp(dvbe / vt) * (1.0 - vbc_exp / b.vaf)
                / (1.0 - vbc_ref / b.vaf);
            let done = (next / x - 1.0).abs() < 1e-11;
            st = ExpoState {
                ic_exp: next,
                ib_exp,
                ic_ref,
                ib_ref,
                v_sum,
                v_ic1,
                vb_ref,
                vb_exp,
                v_sub,
            };
            if done {
                converged = true;
                break;
            }
        }
        if !converged {
            crate::unconverged::note(crate::unconverged::Solver::Converter);
        }
        st
    }
}

/// Potato's converter: the exponential transistor's collector current's logarithm
/// against [`ExpoCircuit::drive_u`], with the ramp at 0 V and at -4 V (the timing current's
/// two points), on 4096 points by the full solve, for one converter's trims and temperature;
/// cubic between them. It spans collector currents from 1e-11 to 2e-3 A (past the audio
/// band: above about 1e-2 A the model's converter runs away); outside it, or for other
/// trims, the full solve.
#[derive(Debug, Clone)]
pub struct ConverterTable {
    u0: f64,
    du: f64,
    top: Vec<f64>,
    bottom: Vec<f64>,
    /// The trims and temperature it was made for (R11, R8's wiper, C).
    key: [u64; 3],
}

impl ConverterTable {
    const N: usize = 4096;

    /// The table for `expo` (its trims as tuned) at `celsius`.
    pub fn new(expo: &ExpoCircuit, celsius: f64) -> ConverterTable {
        let r20 = expo.r20 * (1.0 + expo.tc20 * (celsius - 25.0));
        // drive_u falls by R20 an ampere of input: the input for a drive.
        let u_at_zero = expo.drive_u(0.0, celsius);
        let i_for = |u: f64| (u_at_zero - u) / r20;
        let ln_ic = |u: f64, v_ramp: f64| {
            libm::log(expo.solve_current(i_for(u), v_ramp, celsius, None).ic_exp)
        };
        // The drives for the range's currents: bracketed from ln(ic)'s slope (nearly linear
        // in the drive), then bisected on the solve.
        let (ua, ub) = (u_at_zero, expo.drive_u(1e-5, celsius));
        let slope = (ln_ic(ub, 0.0) - ln_ic(ua, 0.0)) / (ub - ua);
        let u_for = |ic: f64| {
            let target = libm::log(ic);
            let guess = ua + (target - ln_ic(ua, 0.0)) / slope;
            let span = (2.0 / slope).abs();
            let (mut lo, mut hi) = (guess - span, guess + span);
            // (Where ln(ic) grows with the drive, below the target is low.)
            let rising = slope > 0.0;
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                let y = ln_ic(mid, 0.0);
                if (y < target) == rising {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        };
        let (u_lo, u_hi) = {
            let (a, b) = (u_for(1e-11), u_for(2e-3));
            (a.min(b), a.max(b))
        };
        let du = (u_hi - u_lo) / (Self::N - 1) as f64;
        let at = |k: usize| u_lo + du * k as f64;
        ConverterTable {
            u0: u_lo,
            du,
            top: (0..Self::N).map(|k| ln_ic(at(k), 0.0)).collect(),
            bottom: (0..Self::N).map(|k| ln_ic(at(k), -4.0)).collect(),
            key: [expo.r11.to_bits(), expo.a8.to_bits(), celsius.to_bits()],
        }
    }

    /// The collector currents with the ramp at 0 V and -4 V for this converter at `u`
    /// ([`ExpoCircuit::drive_u`]); `None` outside the table or for other trims.
    pub fn lookup(&self, expo: &ExpoCircuit, celsius: f64, u: f64) -> Option<(f64, f64)> {
        if self.key != [expo.r11.to_bits(), expo.a8.to_bits(), celsius.to_bits()] {
            return None;
        }
        let f = (u - self.u0) / self.du;
        if !(1.0..(Self::N - 3) as f64).contains(&f) {
            return None;
        }
        let i = f as usize;
        let t = f - i as f64;
        // Catmull-Rom through the four points about it.
        let cubic = |y: &[f64]| {
            let (p0, p1, p2, p3) = (y[i - 1], y[i], y[i + 1], y[i + 2]);
            p1 + 0.5
                * t
                * (p2 - p0
                    + t * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + t * (3.0 * (p1 - p2) + p3 - p0)))
        };
        // (Potato's exponential: the table is Potato's.)
        Some((
            crate::fast::exp(cubic(&self.top)),
            crate::fast::exp(cubic(&self.bottom)),
        ))
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    /// Potato's converter table against the full solve at drives between its points, over
    /// the audio range and past it, and for other settings of the + input and trimmer
    /// (which only move the drive): within 0.001 cent.
    #[test]
    fn the_converter_table_matches_the_solve() {
        let expo = ExpoCircuit::default();
        let t = ConverterTable::new(&expo, 25.0);
        let mut worst = 0.0f64;
        let mut seen = 0;
        for (pos_v, pos_g, trim_on) in [(0.0, 0.0, true), (-2.0, 1e-4, true), (0.0, 0.0, false)] {
            let e = ExpoCircuit {
                pos_v,
                pos_g,
                trim_on,
                ..expo
            };
            for k in 0..2000 {
                let i_in = -2e-4 + 4e-4 * (k as f64 + 0.37) / 2000.0;
                let u = e.drive_u(i_in, 25.0);
                let Some((top, bottom)) = t.lookup(&e, 25.0, u) else {
                    continue;
                };
                seen += 1;
                let a = e.solve_current(i_in, 0.0, 25.0, None).ic_exp;
                let b = e.solve_current(i_in, -4.0, 25.0, None).ic_exp;
                let cents = |x: f64, y: f64| 1200.0 * (x / y).log2().abs();
                worst = worst.max(cents(top, a)).max(cents(bottom, b));
            }
        }
        assert!(seen > 2000, "{seen} drives in the table");
        assert!(worst < 1e-3, "{worst} cent");
    }
}
