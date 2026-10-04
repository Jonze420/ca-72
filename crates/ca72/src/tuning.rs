//! The factory tuning of an oscillator, run on the real-time model: the same procedure a
//! technician followed (Folkman, "Mini-Moog Field Service Manual", July 1973, Section VII),
//! solved with secant steps instead of a screwdriver. The playable instrument is tuned
//! this way, as the real one was, rather than given a pitch law.

use crate::expo::{Input, OSC1_INPUT_R};
use crate::vco::Vco;

/// The keyboard's volts per key: 8.48 mA through each 10 ohm 1 % resistor (dwg 1436).
pub const KEY_STEP: f64 = 8.48e-3 * 10.0;
/// The pitch wheel in its detent: its 25K pot's wiper 15.3K from GND, +10 V at the top.
pub const BEND_DETENT: f64 = 10.0 * 15.3 / 25.0;
/// TUNE at its centre: R1 5K from +10 V over R22 5.1K to GND.
pub const TUNE_CENTRE: f64 = 10.0 * 7.6 / 10.1;
/// Low A and the second A of the keyboard (keys above the lowest F).
pub const LOW_A: u32 = 4;
pub const SECOND_A: u32 = 16;
pub const HIGH_A: u32 = 40;

/// The range switch's positions, as octaves below 2'. LO's place is an assumption
/// (docs/circuit/assumptions.md A3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Lo,
    R32,
    R16,
    R8,
    R4,
    R2,
}

impl Range {
    pub fn octaves_below_2(self) -> f64 {
        match self {
            Range::R2 => 0.0,
            Range::R4 => 1.0,
            Range::R8 => 2.0,
            Range::R16 => 3.0,
            Range::R32 => 4.0,
            Range::Lo => 9.0,
        }
    }
}

/// Oscillator 1's settings the procedure adjusts, and the voltages it plays with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuning {
    /// R11 (range trimpot, ohm) and R8's wiper (scale trimpot).
    pub r11: f64,
    pub a8: f64,
    /// Volts between range taps (the octave trimpot).
    pub octave_step: f64,
    /// The TUNE control's voltage after the octave step.
    pub tune: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            r11: 500.0,
            a8: 0.5,
            octave_step: 15.0 / 51.1,
            tune: TUNE_CENTRE,
        }
    }
}

/// Oscillator 1's summing inputs for a key on a range, the key string's voltage taken as
/// the keyboard's (see [`osc1_inputs_at`]).
pub fn osc1_inputs(t: &Tuning, key: f64, range: Range) -> [Input; 6] {
    osc1_inputs_at(t, key * KEY_STEP, range)
}

/// Oscillator 1's summing inputs for a keyboard voltage on a range: bend, tune, keyboard,
/// modulation, external, range tap (Figure 9-3's R12, R21, R27, R32, R38, R43).
pub fn osc1_inputs_at(t: &Tuning, v_kbd: f64, range: Range) -> [Input; 6] {
    let v = [
        BEND_DETENT,
        t.tune,
        v_kbd,
        0.0,
        0.0,
        -5.0 - t.octave_step * range.octaves_below_2(),
    ];
    let mut out = [Input { r: 1.0, v: 0.0 }; 6];
    for i in 0..6 {
        out[i] = Input {
            r: OSC1_INPUT_R[i],
            v: v[i],
        };
    }
    out
}

/// Which of board 1's three oscillators, with the front panel controls only it has
/// (board1.md, "Oscillators 2 and 3").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Osc {
    One,
    /// OSCILLATOR-2 FREQUENCY, the knob 0..1 (clockwise raises the pitch).
    Two {
        freq: f64,
    },
    /// OSCILLATOR-3 FREQUENCY likewise, and OSC. 3 CONTROL (SW2): on, the keyboard and the
    /// other inputs reach oscillator 3.
    Three {
        freq: f64,
        control: bool,
    },
}

/// The FREQUENCY knobs at their centre.
pub const FREQ_CENTRE: f64 = 0.5;

/// A FREQUENCY pot (R4 or R5, 5K linear, from +10 V over R35 or R41 5.1K to GND; its
/// clockwise end is the 5.1K end): the wiper's open-circuit voltage and source resistance.
pub fn freq_pot(knob: f64) -> (f64, f64) {
    let k = knob.clamp(0.0, 1.0);
    let (top, bottom) = (5e3 * k, 5e3 * (1.0 - k) + 5.1e3);
    (
        10.0 * bottom / (top + bottom),
        top * bottom / (top + bottom),
    )
}

/// The 741's input bias current (the converter's value).
const BIAS: f64 = 15.16e-6 / 2.0 / 93.75;

/// IC8's effective input offset, V (+ input above the - input): the 741 macromodel's
/// (`ua741.lib`) at IC8's operating point, its +7.5 V common mode on +-10 V supplies through
/// its 90 dB CMRR less its output over its gain. The tuning absorbs it with OSC. 3 CONTROL
/// on; with it off, nothing does (2.7 cents without it). A test measures it in ngspice.
pub const IC8_OFFSET: f64 = 0.188e-3;

/// Oscillator 3's control stage IC8 (`board1-osc23.lib`): its output for the FREQUENCY
/// knob, with OSC. 3 CONTROL off feeding it through R180 as well as R181.
pub fn osc3_control(knob: f64, control: bool) -> f64 {
    let (r181, r180, r162, r170, r150, r155) = (51e3, 15e3, 3.01e3, 15e3, 1e3, 3.01e3);
    // The + input: R150 from +10 V over R155, less its bias current; the - input below it
    // by the offset.
    let vp = (10.0 / r150 - BIAS) / (1.0 / r150 + 1.0 / r155);
    let vi = vp - IC8_OFFSET;
    let (vw, rw) = freq_pot(knob);
    let r_in = if control {
        r181
    } else {
        r181 * r180 / (r181 + r180)
    };
    let i_in = (vw - vi) / (rw + r_in) + (10.0 - vi) / r162 - BIAS;
    vi - r170 * i_in
}

/// Everything an oscillator's converter takes from its controls: the summing inputs (up
/// to eight; unused ones are open), another source on IC1's + input (voltage, conductance)
/// and whether the frequency trimmer chain reaches the summing junction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drive {
    pub inputs: [Input; 8],
    pub pos_v: f64,
    pub pos_g: f64,
    pub trim_on: bool,
}

impl Drive {
    /// Sets the converter's + input source and trimmer chain, and returns the inputs'
    /// current into its summing junction.
    pub fn apply(&self, expo: &mut crate::expo::ExpoCircuit) -> f64 {
        expo.pos_v = self.pos_v;
        expo.pos_g = self.pos_g;
        expo.trim_on = self.trim_on;
        expo.input_current(&self.inputs)
    }
}

/// An oscillator's drive for a keyboard voltage on a range, the bend, TUNE, modulation and
/// external inputs at `t`'s resting values. Oscillators 1 and 2 sum the same inputs (R12..R43;
/// R96..R52); oscillator 3's (R110..R144, with the trimmer chain) meet at 16A, which OSC. 3
/// CONTROL joins to the summing junction or moves to the -5 V line, while its range (R179)
/// and IC8 (R143) reach the summing junction directly.
pub fn osc_drive(osc: Osc, t: &Tuning, v_kbd: f64, range: Range) -> Drive {
    osc_drive_with(osc, t, v_kbd, &Buses::RESTING, range)
}

/// The oscillators' shared input buses other than the keyboard and TUNE: the pitch wheel
/// (4A), the modulation line (7A) and the external input (8A), V.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buses {
    pub bend: f64,
    pub modulation: f64,
    pub ext: f64,
}

impl Buses {
    /// The wheel in its detent, the modulation and external buses at 0 V (the benches').
    pub const RESTING: Buses = Buses {
        bend: BEND_DETENT,
        modulation: 0.0,
        ext: 0.0,
    };
}

/// An undriven bus's voltage (the external input with its jack empty): R163 or R156 33K
/// from +10 V against the three oscillators' 51.1K inputs to their summing junctions at
/// -5 V (board1.md, "Oscillators 2 and 3", 4).
pub const OPEN_BUS: f64 = (10.0 / 33e3 - 3.0 * 5.0 / 51.1e3) / (1.0 / 33e3 + 3.0 / 51.1e3);

/// [`osc_drive`] with the buses given.
pub fn osc_drive_with(osc: Osc, t: &Tuning, v_kbd: f64, buses: &Buses, range: Range) -> Drive {
    let open = Input {
        r: f64::INFINITY,
        v: 0.0,
    };
    let mut inputs = [open; 8];
    let tap = -5.0 - t.octave_step * range.octaves_below_2();
    let summed = [buses.bend, t.tune, v_kbd, buses.modulation, buses.ext];
    let mut d = Drive {
        inputs,
        pos_v: 0.0,
        pos_g: 0.0,
        trim_on: true,
    };
    let joined = !matches!(osc, Osc::Three { control: false, .. });
    if joined {
        for (i, &v) in summed.iter().enumerate() {
            inputs[i] = Input {
                r: OSC1_INPUT_R[i],
                v,
            };
        }
    }
    inputs[5] = Input {
        r: OSC1_INPUT_R[5],
        v: tap,
    };
    match osc {
        Osc::One => {}
        Osc::Two { freq } => {
            // R95 220K from the wiper and R87 91K to -10 V on IC4's + input.
            let (vw, rw) = freq_pot(freq);
            let (g1, g2) = (1.0 / (rw + 220e3), 1.0 / 91e3);
            d.pos_g = g1 + g2;
            d.pos_v = (vw * g1 - 10.0 * g2) / d.pos_g;
        }
        Osc::Three { freq, control } => {
            inputs[6] = Input {
                r: 51.1e3,
                v: osc3_control(freq, control),
            };
            d.trim_on = control;
        }
    }
    d.inputs = inputs;
    d
}

/// The frequency of a real-time oscillator held at a steady input: the mean period between
/// its ramp's resets (exact times), after the first two.
pub fn measure_hz(vco: &mut Vco, sample_rate: f64, i_in: f64, cycles: usize) -> f64 {
    vco.reset();
    let mut times = Vec::with_capacity(cycles + 3);
    let mut seen = 0u64;
    // The period is unknown at first: run until enough resets, with a generous limit.
    let limit = (sample_rate * 40.0) as usize;
    let mut n = 0usize;
    while times.len() < cycles + 3 && n < limit {
        vco.tick(i_in, 0.0);
        let (t, count) = vco.last_reset();
        if count != seen {
            seen = count;
            times.push(t);
        }
        n += 1;
    }
    if times.len() < 4 {
        return 0.0;
    }
    let used = &times[2..];
    let period = (used[used.len() - 1] - used[0]) / (used.len() - 1) as f64;
    sample_rate / period
}

struct Tuner<'a> {
    sample_rate: f64,
    plays: usize,
    /// The keyboard's voltage for a key held.
    volts: &'a dyn Fn(u32) -> f64,
    osc: Osc,
    buses: Buses,
}

impl Tuner<'_> {
    fn hz(&mut self, vco: &mut Vco, t: &Tuning, key: u32, range: Range) -> f64 {
        self.plays += 1;
        vco.expo.r11 = t.r11;
        vco.expo.a8 = t.a8;
        let i_in =
            osc_drive_with(self.osc, t, (self.volts)(key), &self.buses, range).apply(&mut vco.expo);
        measure_hz(vco, self.sample_rate, i_in, 6)
    }

    /// The range and scale trimmers together: on 2', high A at 3520 Hz and low A at
    /// 440 Hz (Newton's method on the two, as a technician iterates them).
    fn range_and_scale(&mut self, vco: &mut Vco, t: &mut Tuning) {
        for _ in 0..10 {
            let lo = self.hz(vco, t, LOW_A, Range::R2);
            let hi = self.hz(vco, t, HIGH_A, Range::R2);
            let e = [crate::ulp::log2(lo / 440.0), crate::ulp::log2(hi / 3520.0)];
            if e[0].abs() < 2e-6 && e[1].abs() < 2e-6 {
                break;
            }
            let (d11, d8) = (5.0, 0.005);
            let t11 = Tuning {
                r11: t.r11 + d11,
                ..*t
            };
            let t8 = Tuning {
                a8: t.a8 + d8,
                ..*t
            };
            let j00 = crate::ulp::log2(self.hz(vco, &t11, LOW_A, Range::R2) / lo) / d11;
            let j01 = crate::ulp::log2(self.hz(vco, &t8, LOW_A, Range::R2) / lo) / d8;
            let j10 = crate::ulp::log2(self.hz(vco, &t11, HIGH_A, Range::R2) / hi) / d11;
            let j11 = crate::ulp::log2(self.hz(vco, &t8, HIGH_A, Range::R2) / hi) / d8;
            let det = j00 * j11 - j01 * j10;
            if det.abs() < 1e-18 {
                break;
            }
            t.r11 = (t.r11 + (-e[0] * j11 + e[1] * j01) / det).clamp(0.0, 1000.0);
            t.a8 = (t.a8 + (-e[1] * j00 + e[0] * j10) / det).clamp(0.0, 1.0);
        }
    }

    /// Solves hz(set(base, x)) = target for x by secant steps from x0.
    #[allow(clippy::too_many_arguments)]
    fn secant(
        &mut self,
        vco: &mut Vco,
        base: Tuning,
        set: fn(&mut Tuning, f64),
        x0: f64,
        dx: f64,
        key: u32,
        range: Range,
        target: f64,
    ) -> f64 {
        let mut ta = base;
        set(&mut ta, x0);
        let (mut xa, mut ea) = (x0, crate::ulp::log2(self.hz(vco, &ta, key, range) / target));
        let mut xb = x0 + dx;
        for _ in 0..12 {
            let mut tb = base;
            set(&mut tb, xb);
            let eb = crate::ulp::log2(self.hz(vco, &tb, key, range) / target);
            if eb.abs() < 2e-6 || eb == ea {
                return xb;
            }
            let next = xb - eb * (xb - xa) / (eb - ea);
            (xa, ea, xb) = (xb, eb, next);
        }
        xb
    }
}

/// Folkman's 1973 procedure on oscillator 1: on 2', high A at 3520 Hz with R11 and low A
/// at 440 Hz with R8 (solved together); then the second A on 2' at 880 Hz with TUNE, and on
/// 8' at 220 Hz with the octave step. Returns the tuning and how many notes were played.
pub fn folkman_1973(vco: &mut Vco, sample_rate: f64, start: Tuning) -> (Tuning, usize) {
    folkman_1973_keys(vco, sample_rate, start, &|k| f64::from(k) * KEY_STEP)
}

/// [`folkman_1973`] with the keys played through a keyboard whose voltage for a key is
/// `volts` (board 2's keyboard circuit, as the procedure is run on the instrument).
pub fn folkman_1973_keys(
    vco: &mut Vco,
    sample_rate: f64,
    start: Tuning,
    volts: &dyn Fn(u32) -> f64,
) -> (Tuning, usize) {
    folkman_1973_buses(vco, sample_rate, start, volts, &Buses::RESTING)
}

/// [`folkman_1973_keys`] with the buses as the instrument leaves them during the procedure
/// (the modulation off, the external input's jack empty).
pub fn folkman_1973_buses(
    vco: &mut Vco,
    sample_rate: f64,
    start: Tuning,
    volts: &dyn Fn(u32) -> f64,
    buses: &Buses,
) -> (Tuning, usize) {
    let mut tu = Tuner {
        sample_rate,
        plays: 0,
        volts,
        osc: Osc::One,
        buses: *buses,
    };
    let mut t = start;
    tu.range_and_scale(vco, &mut t);
    t.tune = tu.secant(
        vco,
        t,
        |t, x| t.tune = x,
        t.tune,
        0.02,
        SECOND_A,
        Range::R2,
        880.0,
    );
    t.octave_step = tu.secant(
        vco,
        t,
        |t, x| t.octave_step = x,
        t.octave_step,
        0.002,
        SECOND_A,
        Range::R8,
        220.0,
    );
    vco.expo.r11 = t.r11;
    vco.expo.a8 = t.a8;
    (t, tu.plays)
}

/// Folkman's procedure repeated for oscillator 2 or 3 ("Turn on OSCILLATOR 2 and repeat the
/// procedure -- then OSCILLATOR 3"): FREQUENCY at mid-position and OSC. 3 CONTROL on, the
/// range and scale trimmers on 2' as for oscillator 1, with oscillator 1's TUNE and octave
/// step in `start` (they are shared). `osc` gives the oscillator; its FREQUENCY and switch
/// are set as the procedure says.
pub fn folkman_1973_osc(
    vco: &mut Vco,
    sample_rate: f64,
    start: Tuning,
    osc: Osc,
    volts: &dyn Fn(u32) -> f64,
) -> (Tuning, usize) {
    folkman_1973_osc_buses(vco, sample_rate, start, osc, volts, &Buses::RESTING)
}

/// [`folkman_1973_osc`] with the buses given.
pub fn folkman_1973_osc_buses(
    vco: &mut Vco,
    sample_rate: f64,
    start: Tuning,
    osc: Osc,
    volts: &dyn Fn(u32) -> f64,
    buses: &Buses,
) -> (Tuning, usize) {
    let osc = match osc {
        Osc::One => Osc::One,
        Osc::Two { .. } => Osc::Two { freq: FREQ_CENTRE },
        Osc::Three { .. } => Osc::Three {
            freq: FREQ_CENTRE,
            control: true,
        },
    };
    let mut tu = Tuner {
        sample_rate,
        plays: 0,
        volts,
        osc,
        buses: *buses,
    };
    let mut t = start;
    tu.range_and_scale(vco, &mut t);
    vco.expo.r11 = t.r11;
    vco.expo.a8 = t.a8;
    (t, tu.plays)
}
