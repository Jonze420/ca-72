//! One oscillator of board 1 on a bench: its frequency, its waveforms, and the factory
//! calibration of Table 5-3 (the updated page for serial numbers below 10175).

use crate::bench::{Solver, Supplies, header};
use ca72_spice::{Error, Ngspice, Plot, crossings};
use std::path::Path;

/// The oscillator's trimmers (board 1) and the front panel's octave trim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trims {
    /// R11, the "range" trimpot: ohms in circuit, 0..1000.
    pub r11: f64,
    /// R8, the "scale" trimpot: wiper position from its R19 end, 0..1.
    pub a8: f64,
    /// R20's temperature coefficient, per C.
    pub tc20: f64,
    /// Volts between adjacent range-switch taps: the octave trimpot R168's effect.
    pub octave_step: f64,
}

impl Default for Trims {
    fn default() -> Self {
        Trims {
            r11: 500.0,
            a8: 0.5,
            tc20: 3.5e-3,
            octave_step: 15.0 / 51.1,
        }
    }
}

/// The range switch's positions.
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
    /// Octaves below 2'. LO's distance below 32' is not established (board1.md B1-6): five
    /// octaves is an assumption (docs/circuit/assumptions.md A3).
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

/// Which of the board's three oscillators, with the front panel controls only it has
/// (board1.md, "Oscillators 2 and 3"; `board1-osc23.lib`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Osc {
    One,
    /// OSCILLATOR-2 FREQUENCY (R4 5K linear, +10 V to R35 5.1K to GND): the knob, 0..1
    /// (clockwise raises the pitch).
    Two {
        freq: f64,
    },
    /// OSCILLATOR-3 FREQUENCY (R5 5K linear, +10 V to R41 5.1K to GND) likewise, and OSC. 3
    /// CONTROL (SW2): on, the keyboard and the other inputs reach oscillator 3.
    Three {
        freq: f64,
        control: bool,
    },
}

/// The FREQUENCY pots at their centre.
pub const FREQ_CENTRE: f64 = 0.5;

/// Voltages at the oscillator's inputs (board 1 pins), before its input resistors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    /// Which oscillator (oscillator 1 by default).
    pub osc: Osc,
    /// Keyboard (6A): 0 V at the lowest F, [`KEY_STEP`] per key.
    pub kbd: f64,
    /// Pitch bend (4A): the pitch wheel's 25K pot between +10 V and GND, its wiper 15.3K
    /// from GND in the detent (Folkman 1973; dwg 1449): [`BEND_DETENT`].
    pub bend: f64,
    /// Tune (5A): R1 5K from +10 V over R22 5.1K to GND (Figure 9-17): +5.05..+10 V,
    /// [`TUNE_CENTRE`] at the knob's centre.
    pub tune: f64,
    /// Modulation (7A) and external control (8A): 0 V when unused.
    pub modulation: f64,
    pub ext: f64,
    pub range: Range,
    /// Rectangular width bias (22B): 0, -1.5 or -2.5 V from the waveform switch.
    pub width: f64,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            osc: Osc::One,
            kbd: 0.0,
            bend: BEND_DETENT,
            tune: TUNE_CENTRE,
            modulation: 0.0,
            ext: 0.0,
            range: Range::R4,
            width: 0.0,
        }
    }
}

/// Volts per key: the keyboard circuit's 8.48 mA source through each 10 ohm 1% resistor of
/// the 43-resistor string (dwg 1436, board 2): 1.0176 V per octave, not exactly 1 V.
pub const KEY_STEP: f64 = 8.48e-3 * 10.0;

/// The pitch wheel's output in its detent: 10 V * 15.3K / 25K.
pub const BEND_DETENT: f64 = 10.0 * 15.3 / 25.0;

/// The tune control's output at its centre: 10 V * (5.1K + 2.5K) / 10.1K.
pub const TUNE_CENTRE: f64 = 10.0 * 7.6 / 10.1;

/// The keyboard voltage of a key: keys above the lowest F.
pub fn key_volts(keys_above_low_f: f64) -> f64 {
    keys_above_low_f * KEY_STEP
}

/// Low A and high A of the 44-key keyboard (F to C), as the tuning procedures use them.
pub const LOW_A: f64 = 4.0 * KEY_STEP;
pub const HIGH_A: f64 = 40.0 * KEY_STEP;

/// The range switches' string loads the -5 V reference toward -10 V (through R169 and
/// R168). The 1st-edition string has 10 ohm steps (S-ERR); one octave step (0.299 V) per
/// 10 ohm is about 30 mA, so the string is about 167 ohm from -5 V to -10 V. Its exact
/// values are not established (board1.md B1-6; docs/circuit/assumptions.md A3). The
/// reference's feedback holds -5 V at any such load.
pub const STRING_LOAD: f64 = 167.0;

/// A FREQUENCY pot on the front panel: 5K linear from +10 V over 5.1K to GND, the knob at
/// `pos` (0 fully counterclockwise, 1 clockwise). Clockwise raises the pitch; the pot
/// reaches the oscillators through inputs that lower it as the wiper rises (IC4's +
/// input; IC8, an inverter, into IC6's summing junction), so its clockwise end is the 5.1K
/// end. Its load, board 1's input network, is in the netlist.
fn freq_pot(name: &str, wiper: &str, pos: f64) -> String {
    let p = pos.clamp(0.0, 1.0);
    format!(
        "r{name}t vp {wiper} {}\nr{name}b {wiper} n{name}b {}\nr{name}g n{name}b 0 5.1k\n",
        (5e3 * p).max(1e-3),
        (5e3 * (1.0 - p)).max(1e-3)
    )
}

/// A netlist of one oscillator with its input resistors and its own front panel controls,
/// driven by ideal sources.
pub fn netlist(trims: &Trims, ctl: &Controls, supplies: Supplies, solver: Solver) -> String {
    let mut s = header("minimoog board 1 oscillator", supplies, solver);
    let tap = -5.0 - trims.octave_step * ctl.range.octaves_below_2();
    s.push_str(&format!(
        "xrefs vp vn m5 m5 m4 mm_refs\n\
         rstring m5 vn {STRING_LOAD}\n\
         vkbd kbd 0 {kbd}\nvbend bend 0 {bend}\nvtune tune 0 {tune}\n\
         vmod mod 0 {modv}\nvext ext 0 {ext}\nvrng rng 0 {tap}\nvwidth width 0 {width}\n",
        kbd = ctl.kbd,
        bend = ctl.bend,
        tune = ctl.tune,
        modv = ctl.modulation,
        ext = ctl.ext,
        width = ctl.width,
    ));
    let vco = |trim: &str| {
        format!(
            "x1 sum pos {trim} ramp buf saw tri rect width vp vn m5 m4 mm_vco \
             r11={} a8={} tc20={}\n",
            trims.r11, trims.a8, trims.tc20
        )
    };
    match ctl.osc {
        Osc::One => {
            // R12 bend, R21 tune, R27 keyboard, R32 modulation, R38 external, R43 range.
            s.push_str(
                "r12 bend sum 150k\nr21 tune sum 560k\nr27 kbd sum 51.1k\nr32 mod sum 51.1k\n\
                 r38 ext sum 51.1k\nr43 rng sum 15k\n",
            );
            s.push_str(&vco("sum"));
        }
        Osc::Two { freq } => {
            // R96, R88, R80, R72, R63, R52; the FREQUENCY network on IC4's + input.
            s.push_str(
                "r96 bend sum 150k\nr88 tune sum 560k\nr80 kbd sum 51.1k\nr72 mod sum 51.1k\n\
                 r63 ext sum 51.1k\nr52 rng sum 15k\nx2f freq pos vn mm_osc2freq\n",
            );
            s.push_str(&freq_pot("4", "freq", freq));
            s.push_str(&vco("sum"));
        }
        Osc::Three { freq, control } => {
            // R110, R120, R129, R136, R144 meet at 16A with the trimmer chain (R109, R108);
            // SW2 joins 16A to the summing junction (15A) or, off, to the -5 V line. R179
            // (range) and R143 (IC8) go to the summing junction directly. R171 (the reverse
            // sawtooth's input) loads the sawtooth.
            s.push_str(
                "r110 bend in3 150k\nr120 tune in3 560k\nr129 kbd in3 51.1k\nr136 mod in3 51.1k\n\
                 r144 ext in3 51.1k\nr179 rng sum 15k\n",
            );
            s.push_str(&format!(
                "rsw2 in3 {} 1m\nrsw10 fw ctl {}\nx8 fw ctl sum vp vn mm_osc3ctl\n\
                 x37 saw rev vp vn mm_revsaw\n",
                if control { "sum" } else { "m5" },
                if control { "1e12" } else { "1m" }
            ));
            s.push_str(&freq_pot("5", "fw", freq));
            s.push_str(&vco("in3"));
        }
    }
    s
}

/// What a frequency run measured.
#[derive(Debug, Clone)]
pub struct Measured {
    /// Mean frequency over the measured periods, Hz.
    pub hz: f64,
    /// The periods measured (after the first ones are skipped), s.
    pub periods: Vec<f64>,
    /// The timing current at the operating point (the estimate the run was sized by), A.
    pub i_exp: f64,
    /// The transient plot, for waveform checks.
    pub plot: Plot,
}

/// Runs the oscillator long enough for `cycles` periods after start-up and measures its
/// frequency from the ramp's falling crossings of -2 V.
pub fn measure(
    spice: &Ngspice,
    work: &Path,
    trims: &Trims,
    ctl: &Controls,
    solver: Solver,
    cycles: usize,
) -> Result<Measured, Error> {
    let net = netlist(trims, ctl, Supplies::default(), solver);
    // A running oscillator has no DC equilibrium, so the bench holds the ramp at -2 V
    // through a switch that opens 1 us into the run: the operating point is then the held
    // circuit's (it converges, and slow parts such as C13's 135 ms filter start settled),
    // and the oscillator starts from it. The open switch leaks 2 pA at most.
    let held = format!(
        "{net}vhold hold 0 -2\nshold ramp hold hctl 0 swhold\n\
         vhctl hctl 0 pwl(0 1 1u 1 1.001u 0)\n.model swhold sw(vt=0.5 vh=0 ron=1m roff=1e14)\n"
    );
    let op = spice.run(&held, &["op"], &work.join("op"))?;
    let i_exp = op[0].scalar("i(v.x1.vexp)");
    if i_exp.is_nan() || i_exp <= 1e-12 {
        return Err(Error::Raw(format!(
            "timing current {i_exp} A at the operating point"
        )));
    }
    let t_est = 0.01e-6 * 3.9 / i_exp;
    let skip = 2usize;
    let tstop = t_est * (cycles + skip + 2) as f64 * 1.25;
    let tmax = t_est / solver.steps_per_period;
    let tran = format!("tran {tmax:e} {tstop:e} 0 {tmax:e}");
    let plots = spice.run(&held, &[tran.as_str()], &work.join("tran"))?;
    let plot = plots
        .into_iter()
        .next()
        .ok_or_else(|| Error::Raw("no plot".into()))?;
    let t = plot.vec("time");
    let ramp = plot.vec("ramp");
    let falls = crossings(t, ramp, -2.0, false);
    if falls.len() < skip + cycles + 1 {
        return Err(Error::Raw(format!(
            "{} falling crossings in {tstop:.3e} s (expected period {t_est:.3e} s)",
            falls.len()
        )));
    }
    let periods: Vec<f64> = falls.windows(2).skip(skip).map(|w| w[1] - w[0]).collect();
    let mean = periods.iter().sum::<f64>() / periods.len() as f64;
    Ok(Measured {
        hz: 1.0 / mean,
        periods,
        i_exp,
        plot,
    })
}

/// A factory tuning procedure for oscillator 1, solved: the technician adjusts one trimmer
/// at a time until the counter reads the target; here each adjustment is a secant search
/// on log frequency, and a pair of adjustments that interact is solved together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Procedure {
    /// Folkman, "Mini-Moog Field Service Manual" (July 1973), Section VII: on 2', high A at
    /// 3520 Hz with the range trimpot (R11) and low A at 440 Hz with the scale trimpot (R8);
    /// then the octave range trimmer: the second A zero-beat on 2' with the TUNE control,
    /// and the same key on 8' zero-beat with the octave trimmer.
    Folkman1973,
    /// Norlin's replacement Table 5-3 for serial numbers below 10175: on 4', high A at
    /// 1760 Hz (R11) and low A at 220 Hz (R8); then high A on 32' at 220 Hz with the octave
    /// trimpot and high A on 4' at 1760 Hz with R11, repeated.
    NorlinUpdated,
}

#[derive(Debug, Clone)]
pub struct Calibration {
    pub procedure: Procedure,
    pub trims: Trims,
    /// The TUNE control's voltage the procedure left (Folkman's octave step moves it).
    pub tune: f64,
    /// (range, key voltage, target Hz, measured Hz) of the procedure's points at the end.
    pub points: Vec<(Range, f64, f64, f64)>,
    pub runs: usize,
}

struct Tuner<'a> {
    spice: &'a Ngspice,
    work: &'a Path,
    solver: Solver,
    runs: usize,
    osc: Osc,
}

impl Tuner<'_> {
    fn hz(&mut self, tr: &Trims, tune: f64, range: Range, kbd: f64) -> Result<f64, Error> {
        self.runs += 1;
        let ctl = Controls {
            osc: self.osc,
            kbd,
            range,
            tune,
            ..Controls::default()
        };
        let dir = self.work.join(format!("run{:03}", self.runs));
        let m = measure(self.spice, &dir, tr, &ctl, self.solver, 4)?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok(m.hz)
    }

    /// Solves g(x) = log2(target) by secant steps from x0, with first step dx.
    fn secant(
        &mut self,
        mut g: impl FnMut(&mut Self, f64) -> Result<f64, Error>,
        x0: f64,
        dx: f64,
        target: f64,
        bounds: (f64, f64),
    ) -> Result<f64, Error> {
        let (mut xa, mut ea) = (x0, g(self, x0)? - target.log2());
        let mut xb = (x0 + dx).clamp(bounds.0, bounds.1);
        for _ in 0..12 {
            let eb = g(self, xb)? - target.log2();
            if eb.abs() < 1e-5 {
                return Ok(xb);
            }
            let slope = (eb - ea) / (xb - xa);
            if slope == 0.0 || !slope.is_finite() {
                return Err(Error::Raw("tuning search found no slope".into()));
            }
            let next = (xb - eb / slope).clamp(bounds.0, bounds.1);
            if next == xb {
                return Err(Error::Raw(format!(
                    "trimmer at its end ({next}) with {:.1} cents to go",
                    eb * 1200.0
                )));
            }
            (xa, ea, xb) = (xb, eb, next);
        }
        Err(Error::Raw("tuning search did not converge".into()))
    }

    /// Range (R11) at `hi` and scale (R8) at `lo` together, on `range`.
    fn range_and_scale(
        &mut self,
        tr: &mut Trims,
        tune: f64,
        range: Range,
        (lo_key, lo_hz): (f64, f64),
        (hi_key, hi_hz): (f64, f64),
    ) -> Result<(), Error> {
        for _ in 0..10 {
            let lo = self.hz(tr, tune, range, lo_key)?;
            let hi = self.hz(tr, tune, range, hi_key)?;
            let e = [(lo / lo_hz).log2(), (hi / hi_hz).log2()];
            if e[0].abs() < 1e-5 && e[1].abs() < 1e-5 {
                return Ok(());
            }
            let (d11, d8) = (10.0, 0.01);
            let t11 = Trims {
                r11: tr.r11 + d11,
                ..*tr
            };
            let t8 = Trims {
                a8: tr.a8 + d8,
                ..*tr
            };
            let j = [
                [
                    (self.hz(&t11, tune, range, lo_key)? / lo).log2() / d11,
                    (self.hz(&t8, tune, range, lo_key)? / lo).log2() / d8,
                ],
                [
                    (self.hz(&t11, tune, range, hi_key)? / hi).log2() / d11,
                    (self.hz(&t8, tune, range, hi_key)? / hi).log2() / d8,
                ],
            ];
            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
            if det.abs() < 1e-18 {
                return Err(Error::Raw(
                    "range and scale do not act independently".into(),
                ));
            }
            let dr11 = (-e[0] * j[1][1] + e[1] * j[0][1]) / det;
            let da8 = (-e[1] * j[0][0] + e[0] * j[1][0]) / det;
            let r11 = (tr.r11 + dr11).clamp(0.0, 1000.0);
            let a8 = (tr.a8 + da8).clamp(0.0, 1.0);
            if r11 == tr.r11 && a8 == tr.a8 {
                return Err(Error::Raw(format!(
                    "range and scale trimmers at their ends: R11 {r11}, R8 {a8}; \
                     {:.1} and {:.1} cents to go",
                    e[0] * 1200.0,
                    e[1] * 1200.0
                )));
            }
            tr.r11 = r11;
            tr.a8 = a8;
        }
        Err(Error::Raw("range and scale did not converge".into()))
    }
}

/// Folkman's procedure for oscillator 2 or 3 (July 1973: "Turn on OSCILLATOR 2 and repeat
/// the procedure -- then OSCILLATOR 3"): its FREQUENCY control at mid-position and OSC. 3
/// CONTROL on, its range trimpot for high A at 3520 Hz and its scale trimpot for low A at
/// 440 Hz on 2', with the TUNE and the octave step oscillator 1's calibration left (they
/// are shared). `osc` gives the FREQUENCY setting; `start` the trims' starting point and the
/// octave step.
pub fn calibrate_osc(
    spice: &Ngspice,
    work: &Path,
    start: Trims,
    solver: Solver,
    osc: Osc,
    tune: f64,
) -> Result<Calibration, Error> {
    let mut t = Tuner {
        spice,
        work,
        solver,
        runs: 0,
        osc,
    };
    let mut tr = start;
    t.range_and_scale(&mut tr, tune, Range::R2, (LOW_A, 440.0), (HIGH_A, 3520.0))?;
    let mut checked = Vec::new();
    for (r, k, target) in [(Range::R2, LOW_A, 440.0), (Range::R2, HIGH_A, 3520.0)] {
        let hz = t.hz(&tr, tune, r, k)?;
        checked.push((r, k, target, hz));
    }
    Ok(Calibration {
        procedure: Procedure::Folkman1973,
        trims: tr,
        tune,
        points: checked,
        runs: t.runs,
    })
}

/// Runs a factory tuning procedure on oscillator 1, starting from `start`.
pub fn calibrate(
    spice: &Ngspice,
    work: &Path,
    start: Trims,
    solver: Solver,
    procedure: Procedure,
) -> Result<Calibration, Error> {
    let mut t = Tuner {
        spice,
        work,
        solver,
        runs: 0,
        osc: Osc::One,
    };
    let mut tr = start;
    let mut tune = TUNE_CENTRE;
    let second_a = LOW_A + 12.0 * KEY_STEP;
    let points: Vec<(Range, f64, f64)> = match procedure {
        Procedure::Folkman1973 => {
            t.range_and_scale(&mut tr, tune, Range::R2, (LOW_A, 440.0), (HIGH_A, 3520.0))?;
            // Octave range trimmer: 2' second A with TUNE, then 8' with the octave step.
            let tr0 = tr;
            tune = t.secant(
                |t, x| t.hz(&tr0, x, Range::R2, second_a).map(f64::log2),
                tune,
                0.05,
                880.0,
                (10.0 * 5.1 / 10.1, 10.0),
            )?;
            let step = t.secant(
                |t, x| {
                    let tr1 = Trims {
                        octave_step: x,
                        ..tr0
                    };
                    t.hz(&tr1, tune, Range::R8, second_a).map(f64::log2)
                },
                tr.octave_step,
                0.003,
                220.0,
                (0.2, 0.4),
            )?;
            tr.octave_step = step;
            vec![
                (Range::R2, LOW_A, 440.0),
                (Range::R2, HIGH_A, 3520.0),
                (Range::R2, second_a, 880.0),
                (Range::R8, second_a, 220.0),
            ]
        }
        Procedure::NorlinUpdated => {
            t.range_and_scale(&mut tr, tune, Range::R4, (LOW_A, 220.0), (HIGH_A, 1760.0))?;
            for _ in 0..6 {
                let tr0 = tr;
                tr.octave_step = t.secant(
                    |t, x| {
                        let tr1 = Trims {
                            octave_step: x,
                            ..tr0
                        };
                        t.hz(&tr1, tune, Range::R32, HIGH_A).map(f64::log2)
                    },
                    tr.octave_step,
                    0.003,
                    220.0,
                    (0.2, 0.4),
                )?;
                let tr0 = tr;
                tr.r11 = t.secant(
                    |t, x| {
                        let tr1 = Trims { r11: x, ..tr0 };
                        t.hz(&tr1, tune, Range::R4, HIGH_A).map(f64::log2)
                    },
                    tr.r11,
                    10.0,
                    1760.0,
                    (0.0, 1000.0),
                )?;
                let h32 = t.hz(&tr, tune, Range::R32, HIGH_A)?;
                if (h32 / 220.0).log2().abs() < 1e-5 {
                    break;
                }
            }
            vec![
                (Range::R4, LOW_A, 220.0),
                (Range::R4, HIGH_A, 1760.0),
                (Range::R32, HIGH_A, 220.0),
            ]
        }
    };
    let mut checked = Vec::new();
    for (r, k, target) in points {
        let hz = t.hz(&tr, tune, r, k)?;
        checked.push((r, k, target, hz));
    }
    Ok(Calibration {
        procedure,
        trims: tr,
        tune,
        points: checked,
        runs: t.runs,
    })
}

/// One point of a tracking sweep.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackPoint {
    pub range: String,
    /// Keys above the lowest F.
    pub key: u32,
    pub kbd_volts: f64,
    pub hz: f64,
    /// The equal-tempered target: the procedure's reference key and range, 12 keys per
    /// octave, one octave per range step.
    pub target_hz: f64,
    pub cents: f64,
    /// The timing current, A.
    pub i_exp: f64,
}

/// The keys' targets after a calibration: A = 440 Hz on 2' at low A, 12 keys per octave.
pub fn target_hz(range: Range, key: u32) -> f64 {
    let semis = key as f64 - 4.0 - 12.0 * range.octaves_below_2();
    440.0 * 2f64.powf(semis / 12.0)
}

/// Frequency of `keys` on each of `ranges`, with the calibrated trims and TUNE.
pub fn track(
    spice: &Ngspice,
    work: &Path,
    cal: &Calibration,
    ranges: &[Range],
    keys: &[u32],
    solver: Solver,
) -> Result<Vec<TrackPoint>, Error> {
    let mut out = Vec::new();
    for &range in ranges {
        for &key in keys {
            let ctl = Controls {
                kbd: key_volts(key as f64),
                range,
                tune: cal.tune,
                ..Controls::default()
            };
            let dir = work.join(format!("{range:?}-{key}"));
            let m = measure(spice, &dir, &cal.trims, &ctl, solver, 4)?;
            let _ = std::fs::remove_dir_all(&dir);
            let target = target_hz(range, key);
            out.push(TrackPoint {
                range: format!("{range:?}"),
                key,
                kbd_volts: ctl.kbd,
                hz: m.hz,
                target_hz: target,
                cents: 1200.0 * (m.hz / target).log2(),
                i_exp: m.i_exp,
            });
        }
    }
    Ok(out)
}

/// The exponential converter's collector current in ngspice, with the ramp held at `v_ramp`
/// by a voltage source (the loop broken), for the real-time converter to be checked against.
pub fn expo_current(
    spice: &Ngspice,
    work: &Path,
    trims: &Trims,
    ctl: &Controls,
    v_ramp: f64,
    solver: Solver,
) -> Result<f64, Error> {
    let net = netlist(trims, ctl, Supplies::default(), solver);
    let op = spice.run(&format!("{net}vhold ramp 0 {v_ramp}\n"), &["op"], work)?;
    Ok(op[0].scalar("i(v.x1.vexp)"))
}

/// The tap voltage a range switch position gives with these trims.
pub fn range_tap(trims: &Trims, range: Range) -> f64 {
    -5.0 - trims.octave_step * range.octaves_below_2()
}

/// What the real-time core needs from one note: the period, the timing current at two
/// ramp voltages (the Early effect makes it linear in between), and the ramp's extremes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CorePoint {
    pub range: String,
    pub key: u32,
    pub hz: f64,
    /// Timing current with the ramp held at -0.2 V and at -3.8 V, A.
    pub i_hi: f64,
    pub i_lo: f64,
    /// The ramp's lowest and highest voltage over the last periods.
    pub ramp_min: f64,
    pub ramp_max: f64,
}

pub fn core_points(
    spice: &Ngspice,
    work: &Path,
    trims: &Trims,
    tune: f64,
    notes: &[(Range, u32)],
    solver: Solver,
) -> Result<Vec<CorePoint>, Error> {
    let mut out = Vec::new();
    for &(range, key) in notes {
        let ctl = Controls {
            kbd: key_volts(f64::from(key)),
            range,
            tune,
            ..Controls::default()
        };
        let dir = work.join(format!("{range:?}-{key}"));
        let i_hi = expo_current(spice, &dir.join("hi"), trims, &ctl, -0.2, solver)?;
        let i_lo = expo_current(spice, &dir.join("lo"), trims, &ctl, -3.8, solver)?;
        let m = measure(spice, &dir.join("run"), trims, &ctl, solver, 4)?;
        let t = m.plot.vec("time");
        let r = m.plot.vec("ramp");
        let t0 = t[t.len() - 1] - 2.0 / m.hz;
        let (lo, hi) = t
            .iter()
            .zip(r)
            .filter(|(t, _)| **t >= t0)
            .fold((f64::MAX, f64::MIN), |(lo, hi), (_, &v)| {
                (lo.min(v), hi.max(v))
            });
        let _ = std::fs::remove_dir_all(&dir);
        out.push(CorePoint {
            range: format!("{range:?}"),
            key,
            hz: m.hz,
            i_hi,
            i_lo,
            ramp_min: lo,
            ramp_max: hi,
        });
    }
    Ok(out)
}

/// DC transfer of the wave shapers: the ramp held by a source and swept from `from` to
/// `to`; ngspice continues each point from the last, so the rectangular comparator's
/// hysteresis follows the sweep's direction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ShaperSweep {
    pub width: f64,
    pub ramp: Vec<f64>,
    pub buf: Vec<f64>,
    pub saw: Vec<f64>,
    pub tri: Vec<f64>,
    pub rect: Vec<f64>,
}

pub fn shaper_sweep(
    spice: &Ngspice,
    work: &Path,
    width: f64,
    from: f64,
    to: f64,
    step: f64,
    solver: Solver,
) -> Result<ShaperSweep, Error> {
    let ctl = Controls {
        width,
        ..Controls::default()
    };
    let net = netlist(&Trims::default(), &ctl, Supplies::default(), solver);
    let step = if to < from { -step.abs() } else { step.abs() };
    let plots = spice.run(
        &format!("{net}vhold ramp 0 {from}\n"),
        &[&format!("dc vhold {from} {to} {step}")],
        work,
    )?;
    let p = &plots[0];
    Ok(ShaperSweep {
        width,
        ramp: p.vec("ramp").to_vec(),
        buf: p.vec("buf").to_vec(),
        saw: p.vec("saw").to_vec(),
        tri: p.vec("tri").to_vec(),
        rect: p.vec("rect").to_vec(),
    })
}

/// The triangle shaper's transfer (Q2, Q1, unloaded) on a uniform grid of the buffer's
/// voltage, from a DC sweep: what `ca72::tables::TRI` holds.
pub const TRI_BUF_MIN: f64 = -4.6;
pub const TRI_BUF_STEP: f64 = 0.01;
pub const TRI_POINTS: usize = 891;

pub fn triangle_table(spice: &Ngspice, work: &Path, solver: Solver) -> Result<Vec<f64>, Error> {
    let s = shaper_sweep(spice, work, 0.0, 0.3, -4.3, 0.0005, solver)?;
    // The sweep runs down the ramp, so the buffer decreases: reverse for interpolation.
    let (mut b, mut t): (Vec<f64>, Vec<f64>) = (s.buf.clone(), s.tri.clone());
    b.reverse();
    t.reverse();
    let mut out = Vec::with_capacity(TRI_POINTS);
    for i in 0..TRI_POINTS {
        let x = TRI_BUF_MIN + TRI_BUF_STEP * i as f64;
        let j = b.partition_point(|&v| v < x).clamp(1, b.len() - 1);
        let a = ((x - b[j - 1]) / (b[j] - b[j - 1])).clamp(0.0, 1.0);
        out.push(t[j - 1] + (t[j] - t[j - 1]) * a);
    }
    Ok(out)
}

/// The Rust source of `ca72::tables`: the triangle shaper's table and the filter's
/// gain recovery amplifier's table.
pub fn tables_source(tri: &[f64], out: &crate::vcf::OutputStage, ngspice: &str) -> String {
    let mut s = format!(
        "//! Tables derived from the circuit reference by `ca72-lab tables` (do not edit), with\n\
         //! {ngspice}. The lab test `tables_match_the_circuit` regenerates them and compares.\n\n"
    );
    let mut array =
        |name: &str, doc: &str, min: f64, step: f64, v: &[f64], fmt: &dyn Fn(f64) -> String| {
            s.push_str(&format!(
                "/// {doc}\npub const {name}_MIN: f64 = {min:?};\n"
            ));
            s.push_str(&format!("pub const {name}_STEP: f64 = {step:?};\n"));
            s.push_str(&format!(
                "#[rustfmt::skip]\npub const {name}: [f64; {}] = [\n",
                v.len()
            ));
            for chunk in v.chunks(8) {
                s.push_str("   ");
                for x in chunk {
                    s.push_str(&format!(" {},", fmt(*x)));
                }
                s.push('\n');
            }
            s.push_str("];\n\n");
        };
    array(
        "TRI",
        "The triangle shaper's DC transfer (Q2, Q1; unloaded; 25 C) on the buffer's voltage \
         (board1-vco.lib).",
        TRI_BUF_MIN,
        TRI_BUF_STEP,
        tri,
        &|x| format!("{x:.6}"),
    );
    array(
        "OUT_DI",
        "The filter's gain recovery amplifier (Q8, Q6, Q7, Q5; 25 C): the change of R3's \
         current, A, against the differential voltage at the followers' bases \
         (board4-vcf.lib).",
        crate::vcf::OUT_W_MIN,
        crate::vcf::OUT_W_STEP,
        &out.di,
        &|x| format!("{x:.9e}"),
    );
    array(
        "OUT_DIB",
        "The same amplifier's base currents: Q8's less Q6's change, A, against the same \
         voltage.",
        crate::vcf::OUT_W_MIN,
        crate::vcf::OUT_W_STEP,
        &out.dib,
        &|x| format!("{x:.9e}"),
    );
    s
}
