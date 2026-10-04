//! Board 4's A-440 reference oscillator (circuit No. 13, `board4-a440.lib`;
//! docs/circuit/board4.md) in real time: its waveform derived from the circuit in ngspice
//! ([`crate::a440_table`]: the steady state's harmonics and its start from switch-on),
//! played at 440 Hz, the pitch the factory trims it to (5.7; as modelled the trim falls
//! short, B4-7).
//!
//! Its output (Q4's emitter) drives R40 and C8 into the VCA ([`crate::vca::Vca::a440`]).
//! SW18 switches its rail: off, the output is at 0 V.

use crate::a440_table as t;

/// The pitch, Hz.
pub const HZ: f64 = 440.0;
/// Points in the steady state's period table.
const N: usize = 2048;

/// The oscillator.
#[derive(Debug, Clone)]
pub struct A440 {
    /// One period of the steady state less its mean, `N + 1` points (the last repeats the
    /// first).
    wave: Vec<f64>,
    phase: f64,
    step: f64,
    since_on: f64,
    dt: f64,
    on: bool,
}

impl A440 {
    pub fn new(rate: f64) -> A440 {
        let wave = (0..=N)
            .map(|i| {
                let x = i as f64 / N as f64;
                t::AMP
                    .iter()
                    .zip(&t::PHASE)
                    .enumerate()
                    .map(|(k, (&a, &p))| {
                        a * crate::ulp::cos(2.0 * core::f64::consts::PI * (k + 1) as f64 * x + p)
                    })
                    .sum::<f64>()
            })
            .collect();
        A440 {
            wave,
            phase: 0.0,
            step: HZ / rate,
            since_on: 0.0,
            dt: 1.0 / rate,
            on: false,
        }
    }

    /// Whether SW18 is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// One sample of the output (V), with SW18 `on`.
    pub fn tick(&mut self, on: bool) -> f64 {
        if !on {
            self.on = false;
            return 0.0;
        }
        if !self.on {
            self.on = true;
            self.since_on = 0.0;
            self.phase = 0.0;
        }
        let pos = self.since_on / t::START_STEP;
        let last = t::START_DC.len() - 1;
        let (dc, amp) = if pos < last as f64 {
            let i = pos as usize;
            let f = pos - i as f64;
            (
                t::START_DC[i] + (t::START_DC[i + 1] - t::START_DC[i]) * f,
                t::START_AMP[i] + (t::START_AMP[i + 1] - t::START_AMP[i]) * f,
            )
        } else {
            // The start's table ends in the steady state; join it without a step.
            (t::DC, 1.0)
        };
        let x = self.phase * N as f64;
        let i = (x as usize).min(N - 1);
        let f = x - i as f64;
        let w = self.wave[i] + (self.wave[i + 1] - self.wave[i]) * f;
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.since_on += self.dt;
        dc + amp * w
    }
}
