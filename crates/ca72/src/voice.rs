//! The instrument, first voice (v0): the parts derived and tested so far, wired as the
//! instrument wires them (docs/circuit/voice.md).
//!
//! - **Keyboard** (board 2's keyboard circuit, [`crate::keyboard`]): 44 keys; the lowest
//!   and highest held join the pitch bus to the string (lowest-note priority follows); the
//!   trigger contact is closed while any key is held (single triggering); the hold,
//!   GLIDE, and the output's load (the oscillators' and the filter's keyboard inputs). A
//!   key's pitch contact opens [`RELEASE_LEAD`] after its trigger contact (A18).
//! - **Oscillators 1, 2 and 3** (board 1, each tuned by Folkman's 1973 procedure on the
//!   model, its keys played through the keyboard circuit): oscillator 2 with its FREQUENCY
//!   control, oscillator 3 with its FREQUENCY control, OSC. 3 CONTROL (its control stage IC8)
//!   and its reverse sawtooth (Q37). Each through its WAVEFORM switch (dwg 1448: triangle,
//!   shark tooth through R030 47K and R031 10K (oscillator 3: the reverse sawtooth),
//!   sawtooth, rectangle at 0, -1.5 and -2.5 V of width bias) and the mixer's VOLUME pot
//!   (25K linear) and 33K onto the bus, as a Norton source. The external input (33K) and the
//!   noise (11K) are switched off and load the bus.
//! - **Filter** (board 4) with its control node: CUTOFF (5K linear across +/-10 V) through
//!   R55 200K, KEYBOARD CONTROL 1 and 2 through R53 300K and R54 150K, AMOUNT OF CONTOUR (5K
//!   linear from the filter contour) through R74 47K; EMPHASIS (R14, 50K reverse audio,
//!   a rheostat).
//! - **VCA** (board 4, balanced by the factory procedure) on the loudness contour.
//! - **Contours** (board 2), at half the output rate, interpolated.
//!
//! The front panel's pot tapers are generic audio and reverse audio laws (assumptions A9;
//! GLIDE is "R2 5M AUDIO" on Figure 9-17).
//! The output is the main output's voltage across a 10K load, scaled 5 V to 1.0.

use crate::a440::A440;
use crate::contour::{ContourCircuit, Contours, Controls, Panel as ContourPanel};
use crate::expo::Input;
use crate::keyboard::{Keyboard, KeyboardCircuit, Load};
use crate::modulation::{LineLoads, Modulation, mod_wheel_r, pitch_wheel_volts};
use crate::noise::{Noise, NoiseCircuit, NoiseLoads, NoiseOut};
use crate::preamp::{InLoop, Preamp, PreampCircuit};
use crate::revsaw::{RevSaw, RevSawCircuit, RevSawOut};
use crate::tuning::{
    Buses, FREQ_CENTRE, HIGH_A, LOW_A, OPEN_BUS, Osc, Range, SECOND_A, Tuning, folkman_1973_buses,
    folkman_1973_osc_buses, osc_drive_with,
};
use crate::vca::{Vca, VcaCircuit, VcaDrive};
use crate::vcf::{ExpoTable, FilterExpo, Vcf};
use crate::vco::{Vco, VcoOut};

#[cfg(test)]
mod loop_lab;

/// The WAVEFORM switch's positions. The second is the shark tooth on oscillators 1 and 2
/// and the reverse sawtooth on oscillator 3 (dwg 1448); either name selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Triangle,
    SharkTooth,
    ReverseSawtooth,
    Sawtooth,
    Square,
    WideRectangle,
    NarrowRectangle,
}

impl Waveform {
    /// The width bias the switch's second pole sets (R034, R033, R032 from ground to
    /// -10 V), V.
    fn width(self) -> f64 {
        match self {
            Waveform::WideRectangle => -1.5,
            Waveform::NarrowRectangle => -2.5,
            _ => 0.0,
        }
    }
}

/// A contour generator's three knobs, 0..1 (the panel's 0..10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourKnobs {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
}

/// One oscillator's controls: RANGE, WAVEFORM, its mixer switch and VOLUME, and FREQUENCY
/// (oscillators 2 and 3; 0..1, centred at 0.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OscPanel {
    pub range: Range,
    pub waveform: Waveform,
    pub on: bool,
    pub volume: f64,
    pub freq: f64,
}

/// How closely the model follows its circuit, against what it costs (history.md, "Three
/// quality modes"): No Compromises within 1e-7 of full scale of the model as it stands;
/// High Fidelity within -120 dBFS, 0.01 cent and a sample of it, each measured on its own
/// path (`ca72-lab hifi`); Potato as close as a hundred voices at once allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    #[default]
    NoCompromises,
    HighFidelity,
    Potato,
}

/// The front panel and left hand controller: knobs 0..1 (the panel's 0..10; CUTOFF's
/// -5..+5 as 0..1), switches as booleans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    /// Oscillators 1, 2 and 3.
    pub osc: [OscPanel; 3],
    /// OSC. 3 CONTROL: on, the keyboard and the other inputs reach oscillator 3.
    pub osc3_control: bool,
    pub cutoff: f64,
    pub emphasis: f64,
    pub contour_amount: f64,
    pub keyboard_control_1: bool,
    pub keyboard_control_2: bool,
    pub filter_contour: ContourKnobs,
    pub loudness_contour: ContourKnobs,
    /// GLIDE (5M, a rheostat) and the left hand controller's GLIDE and DECAY switches.
    pub glide: f64,
    pub glide_on: bool,
    pub decay: bool,
    /// The noise's mixer switch and VOLUME, and the NOISE switch (SW14): WHITE (false) sends
    /// white noise to the mixer and pink to the modulation, PINK (true) pink to the mixer
    /// (through R50) and red to the modulation.
    pub noise_on: bool,
    pub noise_volume: f64,
    pub noise_pink: bool,
    /// MODULATION MIX: 0 oscillator 3 alone (counterclockwise), 1 the noise alone.
    pub mod_mix: f64,
    /// OSCILLATOR MODULATION and FILTER MODULATION.
    pub osc_mod: bool,
    pub filter_mod: bool,
    /// The wheels: pitch -1..1 (0 in its detent), MODULATION 0..1 (fully forward).
    pub pitch_wheel: f64,
    pub mod_wheel: f64,
    /// EXTERNAL INPUT: its VOLUME (R9, 1M audio, ahead of the preamplifier) and its mixer
    /// switch (SW10).
    pub ext_volume: f64,
    pub ext_on: bool,
    /// A-440 (SW18).
    pub a440: bool,
    /// TUNE, -1..1 from its centre (where the factory's tuning leaves it): R1 5K between
    /// +10 V and R22 5.1K, its wiper through R21 into each oscillator (5.35: "4-6
    /// semitones" over its travel).
    pub tune: f64,
    /// The model's quality mode (not a control of the instrument's).
    pub quality: Quality,
}

/// TUNE's wiper voltage over half its travel: R1 spans 10 V less R22's 5.05 V.
pub const TUNE_HALF_SPAN: f64 = 0.5 * (10.0 - 10.0 * 5.1 / 10.1);

impl Default for Panel {
    /// Oscillator 1 alone on a sawtooth at 8'; 2 and 3 off, their FREQUENCY centred.
    fn default() -> Self {
        let osc = |on| OscPanel {
            range: Range::R8,
            waveform: Waveform::Sawtooth,
            on,
            volume: 0.8,
            freq: FREQ_CENTRE,
        };
        Panel {
            osc: [osc(true), osc(false), osc(false)],
            osc3_control: true,
            cutoff: 0.5,
            emphasis: 0.0,
            contour_amount: 0.5,
            keyboard_control_1: true,
            keyboard_control_2: false,
            filter_contour: ContourKnobs {
                attack: 0.0,
                decay: 0.4,
                sustain: 0.5,
            },
            loudness_contour: ContourKnobs {
                attack: 0.0,
                decay: 0.4,
                sustain: 0.8,
            },
            glide: 0.0,
            glide_on: false,
            decay: true,
            noise_on: false,
            noise_volume: 0.8,
            noise_pink: false,
            mod_mix: 0.0,
            osc_mod: false,
            filter_mod: false,
            pitch_wheel: 0.0,
            mod_wheel: 0.0,
            ext_volume: 0.5,
            ext_on: false,
            a440: false,
            tune: 0.0,
            quality: Quality::NoCompromises,
        }
    }
}

/// The input a mixer channel's VOLUME pot (25K linear, its wiper through `r_series` to the
/// bus, near GND) puts on its source, ohm.
fn channel_load(volume: f64, r_series: f64) -> f64 {
    let p = volume.clamp(0.0, 1.0);
    let bottom = 25e3 * p;
    25e3 * (1.0 - p) + bottom * r_series / (bottom + r_series)
}

/// The noise generator's loads for the panel: the mixer's channel and the modulation mix on
/// the outputs the NOISE switch gives them.
fn noise_loads(p: &Panel) -> NoiseLoads {
    let audio = channel_load(p.noise_volume, 11e3);
    let (modulation, _) = Modulation::source_loads(p.mod_mix);
    if p.noise_pink {
        NoiseLoads {
            white: f64::INFINITY,
            pink: 24e3 + audio,
            red: modulation,
        }
    } else {
        NoiseLoads {
            white: audio,
            pink: modulation,
            red: f64::INFINITY,
        }
    }
}

/// The white noise's level against oscillator 1's triangle at the output, as the factory set
/// R26: the service manual's triangle at "1 +- 3dB" (5.8) and noise at "-5 +- 3dB" (5.27;
/// Folkman: "-5dB maximum in the white position"), dB (assumptions A23).
pub const NOISE_BELOW_TRIANGLE: f64 = 6.0;

/// Both channels' VOLUME for that measurement: 10. Folkman's white noise is "-5dB maximum"
/// (the noise at its maximum), and 5.8's triangle ("1 +- 3dB") is what the model's output
/// gives at VOLUME 10 (+1.2 dB). At 10, R50 puts the pink 5 dB under the white, 2 dB below
/// Norlin's later 5.27 window for it (A23; board3.md B3-2).
pub const NOISE_CALIBRATION_VOLUME: f64 = 1.0;

/// The band the meter read the noise over: the audio band, the filter fully open (5.22's
/// CUTOFF fully clockwise, EMPHASIS 0; A23), Hz.
pub const NOISE_CALIBRATION_BAND: (f64, f64) = (20.0, 20e3);

/// Calibrates the noise source's density (R26's job): the white noise's current into the bus
/// through its channel against oscillator 1's triangle's through its channel, both at
/// [`NOISE_CALIBRATION_VOLUME`], the noise over [`NOISE_CALIBRATION_BAND`] (the continuous
/// model's response).
fn calibrate_noise(vco: &mut Vco, rate: f64, tuning: &Tuning) -> f64 {
    let vol = NOISE_CALIBRATION_VOLUME;
    // The triangle's current through its channel, low A on 2' (440 Hz), 200 periods.
    let key = f64::from(LOW_A) * crate::tuning::KEY_STEP;
    let d = osc_drive_with(Osc::One, tuning, key, &Buses::RESTING, Range::R2);
    let i_in = d.apply(&mut vco.expo);
    vco.reset();
    let n = (200.0 * rate / 440.0) as usize;
    let mut samples = Vec::with_capacity(n);
    for k in 0..n + 1000 {
        let o = vco.tick(i_in, 0.0);
        if k >= 1000 {
            let (v, r) = waveform_source(Waveform::Triangle, &o, None);
            samples.push(channel(v, r, vol, true, 33e3).0);
        }
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let i_tri = (samples.iter().map(|i| (i - mean) * (i - mean)).sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    // The white noise at pin 5 into its channel, per unit density.
    let loads = NoiseLoads {
        white: channel_load(vol, 11e3),
        pink: Modulation::source_loads(0.0).0,
        red: f64::INFINITY,
    };
    let Ok(nz) = Noise::new(NoiseCircuit::default(), loads, rate, 0) else {
        return 0.0;
    };
    let (model, _, out) = nz.model_of();
    let (f0, f1) = NOISE_CALIBRATION_BAND;
    let points = 400usize;
    let mut power = 0.0;
    let mut prev: Option<(f64, f64)> = None;
    for k in 0..=points {
        let f = f0 * crate::ulp::pow(f1 / f0, k as f64 / points as f64);
        let z = model.response(out[0], f);
        let pw = z.0 * z.0 + z.1 * z.1;
        if let Some((fp, pp)) = prev {
            power += 0.5 * (pw + pp) * (f - fp);
        }
        prev = Some((f, pw));
    }
    // Its channel: the pin's voltage (a follower behind C18) through the pot and R48 11K.
    let (i_unit, _) = channel(libm::sqrt(power), 0.0, vol, true, 11e3);
    i_tri * crate::ulp::pow(10.0, -NOISE_BELOW_TRIANGLE / 20.0) / i_unit
}

/// An audio taper (generic: 10 % at half rotation), as a fraction of the track.
pub fn audio_taper(p: f64) -> f64 {
    (crate::ulp::pow(81.0, p.clamp(0.0, 1.0)) - 1.0) / 80.0
}

/// The ATTACK and DECAY pots: 1M audio rheostats, ohm.
fn time_pot(p: f64) -> f64 {
    1e6 * audio_taper(p)
}

/// EMPHASIS: R14, 50K reverse audio used as a rheostat, 50K at 0 and 0 at 10.
pub fn emphasis_r14(p: f64) -> f64 {
    50e3 * audio_taper(1.0 - p)
}

/// The mixer's bus as a Norton source: current and conductance.
fn channel(v: f64, r_src: f64, volume: f64, on: bool, r_series: f64) -> (f64, f64) {
    if !on {
        // Off, the switch grounds the series resistor's far end: it loads the bus only.
        return (0.0, 1.0 / r_series);
    }
    let p = volume.clamp(0.0, 1.0);
    let (up, down) = (r_src + 25e3 * (1.0 - p), (25e3 * p).max(1e-3));
    let v_w = v * down / (up + down);
    let r_w = up * down / (up + down);
    let g = 1.0 / (r_w + r_series);
    (v_w * g, g)
}

/// An oscillator's selected output as a Thevenin source (board 1's output dividers:
/// sawtooth R34/R33, triangle R28/R23 behind Q1, rectangle R22 against R13 as Q12
/// switches), the shark tooth through R030 and R031. For oscillator 3, `rev` is its reverse
/// sawtooth stage's (the second position, and the sawtooth as R171 loads it).
fn waveform_source(w: Waveform, o: &VcoOut, rev: Option<&RevSawOut>) -> (f64, f64) {
    let r_saw = 4300.0 * 4700.0 / 9000.0;
    let saw = match rev {
        Some(r) => (r.saw, r_saw * 33e3 / (r_saw + 33e3)),
        None => (o.saw, r_saw),
    };
    let tri = (o.tri, 750.0 * 8200.0 / 8950.0);
    // Rectangle: 4.7K to ground when Q12 is off (0 V), 4.7K || 6.2K when on (-4.3 V).
    let on = (-o.rect / 4.3).clamp(0.0, 1.0);
    let rect = (o.rect, 4700.0 + (4700.0 * 6200.0 / 10900.0 - 4700.0) * on);
    match (w, rev) {
        (Waveform::Triangle, _) => tri,
        (Waveform::Sawtooth, _) => saw,
        (Waveform::SharkTooth | Waveform::ReverseSawtooth, Some(r)) => (r.rev, r.r_rev),
        (Waveform::SharkTooth | Waveform::ReverseSawtooth, None) => {
            let (ga, gb) = (1.0 / (saw.1 + 47e3), 1.0 / (tri.1 + 10e3));
            ((saw.0 * ga + tri.0 * gb) / (ga + gb), 1.0 / (ga + gb))
        }
        _ => rect,
    }
}

/// GLIDE's resistance in circuit: the 5M pot (a rheostat), shorted by the GLIDE switch
/// off, ohm.
pub fn glide_r(p: f64, on: bool) -> f64 {
    if on { 5e6 * audio_taper(p) } else { 0.0 }
}

/// How long a released key's pitch contact stays closed after its trigger contact opens,
/// s (the keyboard is set up so the trigger goes off first: service manual 2.2.6; the time
/// is not documented: assumptions A18).
pub const RELEASE_LEAD: f64 = 2e-3;

/// How long a voice's trigger contact must be open before a key pressed again retriggers
/// its contours, s: Q20 holds the reset line about 12 ms after a release, until C7 has
/// drained (docs/circuit/board2.md, "Trigger section"; `tests/retrigger.rs` finds 11.5 to
/// 12 ms in every mode), and a millisecond more. POLY waits this long before a note that
/// takes a voice whose key is down (decisions.md, "POLY").
pub const RETRIGGER_GAP: f64 = 0.013;

/// The lowest key (F) as a MIDI note on 8' (A, the fifth key, is 110 Hz there).
pub const LOWEST_KEY_MIDI: i32 = 41;
pub const KEYS: i32 = 44;

/// The MIDI notes the voice plays: all 128. Those beyond the instrument's 44 keys continue
/// its key string (the plug-in's extension, not the circuit's: [`Keys::step`]).
pub const MIDI_NOTES: std::ops::RangeInclusive<i32> = 0..=127;

/// The keyboard circuit's load: the three oscillators' keyboard inputs and the filter's
/// KEYBOARD CONTROL 1 and 2 ([`Load::of`]).
fn keyboard_load(p: &Panel) -> Load {
    Load::of(3, p.keyboard_control_1, p.keyboard_control_2)
}

/// How far up FEEDBACK's knob (0..1 of its travel) sends how much of the output back
/// (0..1, [`Voice::feedback`]): a taper, as a volume pot's, so the knob's lower half is a
/// gentle onset and the loop's self-oscillation is near its top. 60 % of the travel sends
/// 30 % (the owner, 2026-10-02, found 30 % sent already very intense at 30 % of the knob);
/// a third sends 6 %, three quarters 51 %.
pub fn feedback_law(knob: f64) -> f64 {
    if knob > 0.0 {
        crate::ulp::pow(knob.min(1.0), FEEDBACK_TAPER)
    } else {
        0.0
    }
}

/// [`feedback_law`]'s exponent: ln 0.3 / ln 0.6, so that 60 % of the knob sends 30 %.
pub const FEEDBACK_TAPER: f64 = 2.356_915_448_856_724;

/// The instrument's voice: its panel and four parts. The keys' contacts, the keyboard
/// circuit and the contours (with the noise source) depend only on the keys and the panel,
/// not on the audio path, so a caller may run them on threads of their own ahead of it
/// ([`crate::threaded`]); [`Voice::tick_in`] runs them in turn.
#[derive(Debug, Clone)]
pub struct Voice {
    pub panel: Panel,
    keys: Keys,
    kbd: KeyboardPart,
    ctl: ControlPart,
    bias: BiasPart,
    audio: AudioPart,
    /// For measuring a quality mode's audio path apart (`ca72-lab hifi`): the pitch and
    /// contour paths (the keyboard circuit, the contours) held at this quality whatever
    /// the panel's.
    control_quality: Option<Quality>,
    /// The last sample's contours (filter, loudness), V.
    last_ctl: (f64, f64),
    /// FEEDBACK (the plug-in's, not the instrument's): how much of the voice's own output is
    /// patched back into its EXTERNAL INPUT jack, 0 to 1 (1: the output at its full level,
    /// before MAIN OUTPUT's VOLUME), one sample late, as players cable the output into the
    /// external input to overdrive the preamplifier and the mixer. Heard through the
    /// MIXER's EXTERNAL INPUT switch and VOLUME, as the cable is. 0, nothing is added.
    pub feedback: f64,
    /// How the preamplifier is solved inside that loop in Potato ([`InLoop`]).
    pub in_loop: InLoop,
    /// The last output sample, for FEEDBACK.
    fed_back: f64,
}

/// The keys: which are held, and which released keys' pitch contacts are still closed.
#[derive(Debug, Clone)]
pub struct Keys {
    rate: f64,
    /// Keys held (as key numbers from the lowest), in the order pressed.
    held: Vec<i32>,
    /// Released keys whose pitch contacts are still closed, and the sample they open at.
    releasing: Vec<(i32, u64)>,
    samples: u64,
    /// The string's foot as last set (it stays while no key's contact is closed).
    foot: i32,
}

/// A sample's contacts: the lowest and highest keys whose pitch contacts are closed, the
/// trigger contact, and EXT. S-TRIG (the rear jack shorted to ground). With a key beyond
/// the string's ends sounding, `keys` is that end's key alone and `foot` how many keys
/// beyond it the key is ([`Keys::step`]); 0 otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Contacts {
    pub keys: Option<(usize, usize)>,
    pub gate: bool,
    pub s_trig: bool,
    pub foot: i32,
}

/// A sample at the instrument's jacks: the EXTERNAL INPUT (as a fraction of
/// [`INPUT_VOLTS`]) and the rear panel's control inputs, V, `None` for an empty jack (its
/// normal contact in place: the oscillators' external bus held by R156 33K, the filter's
/// R51 grounded, the loudness input tied to +10 V through 33K); EXT. S-TRIG closed or open.
/// A plugged source is ideal (A5).
///
/// With them, the plug-in's ENTROPY (decisions.md; not the instrument's): each oscillator off its
/// calibration by `detune` cents and the filter's cutoff off its by `cutoff` octaves, both
/// 0 for the circuit as drawn (the samples then the same to the bit).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Jacks {
    pub ext: f64,
    pub osc: Option<f64>,
    pub filter: Option<f64>,
    pub loudness: Option<f64>,
    pub s_trig: bool,
    pub detune: [f64; 3],
    pub cutoff: f64,
}

/// The volts an octave at a 100K input to the filter's control node, as R51 (the FILTER
/// CONTROL jack's) is, measured on the model with CUTOFF at its centre: 0.97 V at -3 and
/// 1.03 V at +3, the converter's own curve (`tests/entropy.rs`). KEYBOARD CONTROL 1 and 2
/// together (100K) make the filter follow the keyboard's 1.013 V an octave about as closely.
pub const FILTER_VOLTS_PER_OCTAVE: f64 = 0.98;

/// The keyboard circuit (board 1), driven by the contacts: the pitch voltage.
#[derive(Debug, Clone)]
pub struct KeyboardPart {
    keyboard: Keyboard,
    /// The sample rate, Hz; the circuit's sub-rate (samples a tick, [`keyboard_every`]),
    /// the samples since the last tick and the outputs interpolated between.
    rate: f64,
    every: usize,
    count: usize,
    from: f64,
    to: f64,
    /// The keyboard's output's volts a key over the instrument's keys (the string's scale as
    /// its current, the amplifier and the load give it), and the keys beyond the string's
    /// ends in volts as they stand, gliding as the hold capacitor does (decisions.md, "The
    /// full MIDI range": the plug-in's extension).
    step: f64,
    beyond: f64,
}

/// The contour generators at their sub-rate, interpolated, and the noise source: what
/// depends only on the trigger contact and the panel.
#[derive(Debug, Clone)]
pub struct ControlPart {
    contours: Contours,
    contour_every: usize,
    /// The sample rate, Hz (the contours' sub-rate follows the quality mode).
    rate: f64,
    count: usize,
    env_from: (f64, f64),
    env_to: (f64, f64),
    noise: Noise,
    /// The ATTACK and DECAY pots' laws (filter, loudness).
    pots: [Memo<f64>; 4],
    /// The noise's last outputs, and the modulation mix's gain from it (its table, and its
    /// law at MODULATION MIX), for knowing when nothing hears it.
    last_noise: NoiseOut,
    mix: Modulation,
    noise_gain: Memo<f64>,
}

/// A knob law's last input and value: taken again only when its knob moves
/// (performance: their powers were a few percent of Potato's time). The same value either
/// way.
#[derive(Debug, Clone, Copy)]
struct Memo<T: Copy>(Option<(u64, T)>);

impl<T: Copy> Memo<T> {
    const NEW: Memo<T> = Memo(None);

    fn get(&mut self, x: f64, law: impl Fn(f64) -> T) -> T {
        match self.0 {
            Some((k, v)) if k == x.to_bits() => v,
            _ => {
                let v = law(x);
                self.0 = Some((x.to_bits(), v));
                v
            }
        }
    }
}

/// The VCA's bias ([`Vca::control`]) on the loudness contour and EXT. LOUDNESS. It reads
/// nothing of the signal path, so on threads it runs on one of its own, fed the contour
/// by the contours' as they go, ahead of the audio path (performance); the back's copy
/// of the VCA runs the signal path on it.
#[derive(Debug, Clone)]
pub struct BiasPart {
    vca: Vca,
    /// EXT. LOUDNESS's normal contact: the voltage and resistance J3 ties the input to.
    j3_normal: (f64, f64),
    /// The VCA's full bias solve's interval with the jack empty, samples.
    vca_every: usize,
}

/// A sample of [`ControlPart`]'s outputs: the filter and loudness contours, V, and the
/// noise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControlOut {
    pub env_f: f64,
    pub env_l: f64,
    pub noise: NoiseOut,
}

/// The audio path: the external input's preamplifier, then the front (modulation,
/// oscillators, mixer, the filter's control node), then the back (filter, VCA, the A-440).
/// Each depends only on what comes before it, so on threads they run side by side, each a
/// few samples behind the one before (`threaded.rs`), doing what [`AudioPart::tick`] does.
#[derive(Debug, Clone)]
pub struct AudioPart {
    pub(crate) pre: PreampPart,
    pub(crate) front: FrontPart,
    pub(crate) back: BackPart,
    /// The last sample's keyboard voltage, the mixer bus's Norton current, A, and the
    /// filter's output, V.
    last: (f64, f64, f64),
}

/// The external input's preamplifier and the OVERLOAD lamp's driver.
#[derive(Debug, Clone)]
pub struct PreampPart {
    preamp: Preamp,
    /// The OVERLOAD lamp (0 off, 1 fully lit), its brightest since it was last read
    /// ([`AudioPart::take_overload_peak`]), and the preamplifier's output, V.
    overload: f64,
    overload_peak: f64,
    ext_amp: f64,
    /// EXT. INPUT VOLUME's law.
    volume: Memo<(f64, f64)>,
    /// With FEEDBACK on (`accurate`), in Potato, the preamplifier as `in_loop` has it
    /// (decisions.md R8, R11): Potato's own (its paths fitted to the circuit's gain)
    /// oscillates low and dark inside the loop where the circuit does not.
    pub accurate: bool,
    pub in_loop: InLoop,
}

/// The modulation line, the oscillators, the mixer and the filter's control node.
#[derive(Debug, Clone)]
pub struct FrontPart {
    rate: f64,
    /// Each oscillator's tuning (TUNE and the octave step shared) and its oscillator.
    tunings: [Tuning; 3],
    vcos: [Vco; 3],
    revsaw: RevSaw,
    expo: FilterExpo,
    table: std::sync::Arc<ExpoTable>,
    modulation: Modulation,
    /// The filter's control node's voltage and current (the last sample's), and the
    /// modulation line's.
    filter_node: f64,
    i0: f64,
    line: f64,
    /// Oscillator 3's switch output, the last sample's (the modulation mix's input).
    last_osc3: f64,
    /// Each oscillator's timing current (the converter's output), the last sample's: its
    /// frequency follows it (a probe of the pitch path).
    last_i: [f64; 3],
    /// The MODULATION wheel's law.
    wheel: Memo<f64>,
}

/// What the front gives the back each sample: the mixer bus's Norton current (A) and
/// conductance (S), and the filter's control current (A).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrontOut {
    pub i_bus: f64,
    pub g_bus: f64,
    pub i0: f64,
}

/// The filter, the VCA and the A-440.
#[derive(Debug, Clone)]
pub struct BackPart {
    vcf: Vcf,
    /// The VCA's signal path ([`Vca::signal`]) on the bias [`ControlPart`] works out.
    vca: Vca,
    a440: A440,
    /// EMPHASIS's law.
    emphasis: Memo<f64>,
}

/// The external input's scale: the jack's voltage for a sample of 1.0, the same as the
/// output's (so a render fed back to the input behaves as the instrument does; A25).
pub const INPUT_VOLTS: f64 = 5.0;

/// R9 (EXTERNAL INPUT VOLUME, 1M audio) at `volume`: its divider's ratio and source
/// resistance, ohm.
fn ext_volume(volume: f64) -> (f64, f64) {
    let t = audio_taper(volume.clamp(0.0, 1.0));
    (t, 1e6 * t * (1.0 - t))
}

impl Voice {
    /// A voice at `rate` Hz (48 kHz is what the models are tested at): tunes the three
    /// oscillators and balances the VCA as the factory did, and settles every state.
    pub fn new(rate: f64, panel: Panel) -> Voice {
        let mut keyboard = Keyboard::new(KeyboardCircuit::default(), rate);
        keyboard.set_load(keyboard_load(&panel));
        // The factory tuning plays its keys through the keyboard circuit, settled (the
        // keys it uses, each settled once).
        let played: Vec<(u32, f64)> = [LOW_A, SECOND_A, HIGH_A]
            .iter()
            .map(|&k| {
                let v = keyboard
                    .static_out(k as usize)
                    .unwrap_or(f64::from(k) * crate::tuning::KEY_STEP);
                (k, v)
            })
            .collect();
        let volts = |k: u32| {
            played
                .iter()
                .find(|&&(key, _)| key == k)
                .map_or(f64::from(k) * crate::tuning::KEY_STEP, |&(_, v)| v)
        };
        let os = oversampling(Quality::NoCompromises).0;
        let mut vcos = [Vco::new(rate, os), Vco::new(rate, os), Vco::new(rate, os)];
        // Tuned as the instrument stands for the procedure: the wheel in its detent, the
        // modulation off (its bus grounded, A21), the external input's jack empty.
        let buses = Buses {
            ext: OPEN_BUS,
            ..Buses::RESTING
        };
        let (t1, _) = folkman_1973_buses(&mut vcos[0], rate, Tuning::default(), &volts, &buses);
        let two = Osc::Two { freq: FREQ_CENTRE };
        let three = Osc::Three {
            freq: FREQ_CENTRE,
            control: true,
        };
        let (t2, _) = folkman_1973_osc_buses(&mut vcos[1], rate, t1, two, &volts, &buses);
        let (t3, _) = folkman_1973_osc_buses(&mut vcos[2], rate, t1, three, &volts, &buses);
        let tunings = [t1, t2, t3];
        for (v, t) in vcos.iter_mut().zip(&tunings) {
            v.expo.r11 = t.r11;
            v.expo.a8 = t.a8;
        }
        let revsaw = RevSaw::new(RevSawCircuit::default(), rate)
            .expect("the reverse sawtooth's operating point");
        let density = calibrate_noise(&mut vcos[0].clone(), rate, &t1);
        for v in &mut vcos {
            v.prepare(oversampling(Quality::Potato).0);
        }
        let mut noise = Noise::new(
            NoiseCircuit::default(),
            noise_loads(&panel),
            rate,
            0x4D6F_6F67,
        )
        .expect("the noise generator's operating point");
        noise.set_density(density);
        let modulation = Modulation::new().expect("the modulation mix's operating points");
        let contour_every = contour_every(Quality::NoCompromises);
        let mut vca = Vca::new(VcaCircuit::default().calibrated(), rate, 0.0);
        vca.prepare_potato();
        let mut ctl = ControlPart {
            contours: Contours::new(ContourCircuit::default(), rate / contour_every as f64),
            contour_every,
            rate,
            count: 0,
            env_from: (0.0, 0.0),
            env_to: (0.0, 0.0),
            noise,
            pots: [Memo::NEW; 4],
            last_noise: NoiseOut::default(),
            mix: modulation.clone(),
            noise_gain: Memo::NEW,
        };
        let bias = BiasPart {
            vca: vca.clone(),
            j3_normal: (vca.circuit.ext, vca.circuit.r_j3),
            vca_every: Vca::new(VcaCircuit::default(), rate, 0.0).control_every,
        };
        let cp = contour_panel(&panel, false, &mut ctl.pots);
        ctl.contours.settle(&cp);
        let o = ctl.contours.tick(false, &cp);
        ctl.env_from = (o.filter, o.loudness);
        ctl.env_to = ctl.env_from;
        Voice {
            panel,
            keys: Keys {
                rate,
                // Room for every key: a key press must not allocate on the audio thread.
                held: Vec::with_capacity(128),
                releasing: Vec::with_capacity(128),
                samples: 0,
                foot: 0,
            },
            kbd: KeyboardPart {
                to: keyboard.out(),
                from: keyboard.out(),
                keyboard,
                rate,
                every: 1,
                count: 0,
                step: (volts(HIGH_A) - volts(LOW_A)) / f64::from(HIGH_A - LOW_A),
                beyond: 0.0,
            },
            ctl,
            bias,
            audio: AudioPart {
                pre: PreampPart {
                    preamp: Preamp::new(
                        PreampCircuit::default(),
                        rate,
                        ext_volume(panel.ext_volume).1,
                    )
                    .expect("the preamplifier's operating point"),
                    overload: 0.0,
                    overload_peak: 0.0,
                    ext_amp: 0.0,
                    volume: Memo::NEW,
                    accurate: false,
                    in_loop: InLoop::default(),
                },
                front: FrontPart {
                    rate,
                    tunings,
                    vcos,
                    revsaw,
                    expo: crate::filter_cal::FACTORY.expo(),
                    table: ExpoTable::shared(crate::filter_cal::FACTORY.expo(), 25.0),
                    modulation,
                    filter_node: 0.0,
                    i0: 0.0,
                    line: 0.0,
                    last_osc3: 0.0,
                    last_i: [0.0; 3],
                    wheel: Memo::NEW,
                },
                back: BackPart {
                    vcf: {
                        // The filter as the factory calibrated it (Folkman's procedure:
                        // filter_cal).
                        let mut f = Vcf::new(rate, oversampling(Quality::NoCompromises).1);
                        f.circuit = crate::filter_cal::FACTORY.circuit();
                        f.prepare(oversampling(Quality::Potato).1);
                        f
                    },
                    vca,
                    a440: A440::new(rate),
                    emphasis: Memo::NEW,
                },
                last: (0.0, 0.0, 0.0),
            },
            control_quality: None,
            last_ctl: (0.0, 0.0),
            feedback: 0.0,
            in_loop: InLoop::default(),
            fed_back: 0.0,
        }
    }

    /// A voice at `rate` Hz built once per rate and copied: its factory calibration (the
    /// oscillators' tuning, the tables, the noise's level) depends only on the circuit and
    /// the rate, and takes seconds. The panel is the default; set it after.
    pub fn prototype(rate: f64) -> Voice {
        use std::sync::{Mutex, OnceLock};
        static BUILT: OnceLock<Mutex<Vec<(u64, Voice)>>> = OnceLock::new();
        let cache = BUILT.get_or_init(|| Mutex::new(Vec::new()));
        let key = rate.to_bits();
        let cached = cache
            .lock()
            .ok()
            .and_then(|c| c.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()));
        let mut v = match cached {
            Some(v) => v,
            None => {
                let v = Voice::new(rate, Panel::default());
                if let Ok(mut c) = cache.lock() {
                    c.push((key, v.clone()));
                }
                v
            }
        };
        // A clone keeps the key lists' contents, not their room: a key press must not
        // allocate on the audio thread.
        v.keys.reserve();
        v
    }

    /// Seeds the noise generator (each device its own stream; a render repeats).
    pub fn set_seed(&mut self, seed: u64) {
        self.ctl.noise.reseed(seed);
    }

    pub fn rate(&self) -> f64 {
        self.audio.front.rate
    }

    /// The contours' latest values (filter, loudness), V, and whether a key is held.
    pub fn envelopes(&self) -> (f64, f64, bool) {
        (
            self.ctl.env_to.0,
            self.ctl.env_to.1,
            !self.keys.held.is_empty(),
        )
    }

    /// The last sample's keyboard voltage, V, the mixer bus's Norton current, A, and the
    /// filter's output, V.
    pub fn probe(&self) -> (f64, f64, f64) {
        self.audio.last
    }

    /// The last sample's pitch path: the keyboard's voltage, V, and each oscillator's
    /// timing current, A.
    pub fn probe_pitch(&self) -> (f64, [f64; 3]) {
        (self.audio.last.0, self.audio.front.last_i)
    }

    /// Oscillator `n`'s (0, 1, 2) latest reset: its time in samples and how many it has
    /// made (a probe of its frequency at any pitch: the resets' spacing).
    pub fn probe_resets(&self, n: usize) -> (f64, u64) {
        self.audio.front.vcos[n].last_reset()
    }

    /// The last sample's contours (filter, loudness), V.
    pub fn probe_contours(&self) -> (f64, f64) {
        self.last_ctl
    }

    /// Holds the pitch and contour paths (the keyboard circuit, the contours) at a quality
    /// whatever the panel's, or lets them follow it again (`None`): for measuring a
    /// quality mode's audio path apart (`ca72-lab hifi`).
    pub fn hold_control_quality(&mut self, q: Option<Quality>) {
        self.control_quality = q;
    }

    /// Each oscillator's tuning (the factory procedure's result).
    pub fn tunings(&self) -> [Tuning; 3] {
        self.audio.front.tunings
    }

    /// The last sample's filter current (the ladder's I0, A), its control node's voltage and
    /// the modulation line's voltage, V.
    pub fn filter_probe(&self) -> (f64, f64, f64) {
        (
            self.audio.front.i0,
            self.audio.front.filter_node,
            self.audio.front.line,
        )
    }

    /// The noise source's calibrated density, V per root hertz.
    pub fn noise_density(&self) -> f64 {
        self.ctl.noise.density()
    }

    /// The reverse sawtooth stage (for its counter of failed steps).
    pub fn revsaw(&self) -> &RevSaw {
        &self.audio.front.revsaw
    }

    /// The keyboard circuit (for its counters of failed steps).
    pub fn keyboard(&self) -> &Keyboard {
        &self.kbd.keyboard
    }

    /// A MIDI note (0 to 127) pressed or released: beyond the 44 keys the string continues
    /// ([`Keys::step`]).
    pub fn note(&mut self, midi: i32, on: bool) {
        self.keys.note(midi, on);
    }

    /// One output sample.
    pub fn tick(&mut self) -> f64 {
        self.tick_in(0.0)
    }

    /// The external preamplifier (for profiles).
    pub fn preamp(&self) -> &crate::preamp::Preamp {
        &self.audio.pre.preamp
    }

    /// Failed solves in the external preamplifier (tests require none).
    pub fn preamp_failed(&self) -> usize {
        self.audio.pre.preamp.failed
    }

    /// The external preamplifier's output, the last sample's (V; for tests).
    pub fn ext_probe(&self) -> f64 {
        self.audio.pre.ext_amp
    }

    /// The OVERLOAD lamp: 0 off, 1 fully lit (it lights while the external input's
    /// preamplifier is driven near its limits, and holds a moment).
    pub fn overload(&self) -> f64 {
        self.audio.pre.overload
    }

    /// The OVERLOAD lamp's brightest since the last call (for a display that reads it now
    /// and then and should not miss a flash).
    pub fn take_overload_peak(&mut self) -> f64 {
        self.audio.take_overload_peak()
    }

    /// The voice's parts, for running them on threads of their own
    /// ([`crate::threaded`]).
    pub fn into_parts(self) -> (Panel, Keys, KeyboardPart, ControlPart, BiasPart, AudioPart) {
        (
            self.panel, self.keys, self.kbd, self.ctl, self.bias, self.audio,
        )
    }

    /// One output sample with `ext` at the EXTERNAL INPUT jack (as a fraction of
    /// [`INPUT_VOLTS`]).
    pub fn tick_in(&mut self, ext: f64) -> f64 {
        self.tick_jacks(&Jacks {
            ext,
            ..Jacks::default()
        })
    }

    /// One output sample with these signals at the jacks (and, with [`Voice::feedback`],
    /// the last output sample added at EXTERNAL INPUT).
    pub fn tick_jacks(&mut self, j: &Jacks) -> f64 {
        self.audio.pre.accurate = self.feedback > 0.0;
        self.audio.pre.in_loop = self.in_loop;
        let y = if self.feedback > 0.0 {
            self.tick_at(&Jacks {
                ext: j.ext + self.feedback * self.fed_back,
                ..*j
            })
        } else {
            self.tick_at(j)
        };
        self.fed_back = y;
        y
    }

    fn tick_at(&mut self, j: &Jacks) -> f64 {
        let p = self.panel;
        let mut c = self.keys.step();
        c.s_trig = j.s_trig;
        // (The pitch and contour paths at another quality, when it is held.)
        let pc = match self.control_quality {
            Some(quality) => Panel { quality, ..p },
            None => p,
        };
        let v_kbd = self.kbd.tick(&pc, c);
        let ctl = self.ctl.tick(&pc, c);
        self.last_ctl = (ctl.env_f, ctl.env_l);
        let d = self.bias.tick(ctl.env_l, j.loudness, p.quality);
        self.audio.tick(&p, j, v_kbd, &ctl, &d)
    }
}

/// The contour generators' settings from the panel, EXT. S-TRIG open or closed.
fn contour_panel(p: &Panel, s_trig: bool, pots: &mut [Memo<f64>; 4]) -> ContourPanel {
    let [a0, d0, a1, d1] = pots;
    let k = |c: ContourKnobs, a: &mut Memo<f64>, d: &mut Memo<f64>| Controls {
        attack: a.get(c.attack, time_pot),
        decay: d.get(c.decay, time_pot),
        sustain: c.sustain,
    };
    ContourPanel {
        filter: k(p.filter_contour, a0, d0),
        loudness: k(p.loudness_contour, a1, d1),
        decay_on: p.decay,
        s_trig,
    }
}

impl Keys {
    /// A MIDI note (0 to 127) pressed or released: the 44 keys F to C (41 to 84) are the
    /// instrument's, the rest continue its key string ([`Keys::step`]).
    pub fn note(&mut self, midi: i32, on: bool) {
        if !MIDI_NOTES.contains(&midi) {
            return;
        }
        let k = midi - LOWEST_KEY_MIDI;
        if on {
            if !self.held.contains(&k) {
                self.held.push(k);
            }
            self.releasing.retain(|&(x, _)| x != k);
        } else if self.held.contains(&k) {
            self.held.retain(|&x| x != k);
            let until = self.samples + (RELEASE_LEAD * self.rate).round() as u64;
            self.releasing.push((k, until));
        }
    }

    /// The contacts for the next sample (the lowest and highest keys whose pitch contacts
    /// are closed: held, or released within the lead; and the trigger contact), and a
    /// sample on.
    ///
    /// Keys beyond the string's ends (the plug-in's extension: decisions.md, "The full MIDI range"): the
    /// trigger contact closes for them as for the rest. While the lowest key whose contact
    /// is closed is below F, it sounds alone: the circuit plays F, and the keys between F
    /// and it are added to the keyboard's output ([`KeyboardPart::tick`]); while every such
    /// key is above C, the lowest of them likewise, from C. Otherwise the keys above C are
    /// left out, as the instrument's lowest-note priority leaves them silent, and the
    /// contacts are the instrument's own.
    pub fn step(&mut self) -> Contacts {
        let now = self.samples;
        self.releasing.retain(|&(_, until)| until > now);
        let keys = || {
            self.held
                .iter()
                .copied()
                .chain(self.releasing.iter().map(|&(k, _)| k))
        };
        let on = || keys().filter(|k| (0..KEYS).contains(k));
        let (keys, foot) = match (keys().min(), on().min().zip(on().max())) {
            (Some(lowest), _) if lowest < 0 => (Some((0, 0)), lowest),
            (_, Some((lo, hi))) => (Some((lo as usize, hi as usize)), 0),
            (Some(lowest), None) => {
                let top = (KEYS - 1) as usize;
                (Some((top, top)), lowest - (KEYS - 1))
            }
            (None, _) => (None, self.foot),
        };
        self.foot = foot;
        let c = Contacts {
            keys,
            gate: !self.held.is_empty(),
            s_trig: false,
            foot,
        };
        self.samples += 1;
        c
    }

    /// Room for every key, so a key press does not allocate (a clone keeps the lists'
    /// contents, not their room).
    pub fn reserve(&mut self) {
        self.held.reserve(128);
        self.releasing.reserve(128);
    }

    /// Whether a key is held.
    pub fn held(&self) -> bool {
        !self.held.is_empty()
    }
}

impl KeyboardPart {
    /// One sample of the keyboard circuit's output, V, for the contacts, GLIDE and load (at
    /// the circuit's sub-rate the contacts are read at its ticks and the output
    /// interpolated between them).
    pub fn tick(&mut self, p: &Panel, c: Contacts) -> f64 {
        let _fast = crate::ulp::FastScope::new(p.quality != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        if self.count.is_multiple_of(self.every) {
            // The sub-rate as the quality mode has it, changed at a tick.
            let every = keyboard_every(p.quality);
            if every != self.every {
                self.every = every;
                self.count = 0;
                self.keyboard.set_rate(self.rate / every as f64);
            }
            self.keyboard.set_quality(p.quality);
            self.keyboard.set_glide(glide_r(p.glide, p.glide_on));
            self.keyboard.set_load(keyboard_load(p));
            self.from = self.to;
            self.to = self.keyboard.tick(c.keys, c.gate);
            if c.foot != 0 || self.beyond != 0.0 {
                self.to += self.beyond(p, c);
            }
        }
        self.count += 1;
        let v_kbd = if self.every == 1 {
            self.to
        } else {
            let a = ((self.count - 1) % self.every + 1) as f64 / self.every as f64;
            self.from + (self.to - self.from) * a
        };
        laps.lap(crate::prof::Part::Keyboard);
        v_kbd
    }

    /// The keys beyond the string's ends, in volts at the keyboard's output, a tick on
    /// (decisions.md, "The full MIDI range": the plug-in's extension, not the circuit's). The circuit plays the
    /// end key, and this adds the keys between it and the key sounding at the output's own
    /// volts a key, so the scale continues. While the trigger contact is closed it moves as
    /// the circuit's output does, through R61 and GLIDE onto the hold capacitor C6; while it
    /// is open, it holds, as C6 does.
    fn beyond(&mut self, p: &Panel, c: Contacts) -> f64 {
        let target = self.step * f64::from(c.foot);
        if c.gate {
            let k = &self.keyboard.circuit;
            let tau = (k.r61 + glide_r(p.glide, p.glide_on)) * k.c6;
            let h = self.every as f64 / self.rate;
            let a = crate::ulp::exp(-h / tau);
            self.beyond = target + (self.beyond - target) * a;
            if (self.beyond - target).abs() < 1e-12 {
                self.beyond = target;
            }
        }
        self.beyond
    }
}

/// The keyboard circuit's sub-rate (samples a tick) in each quality mode: in Potato an
/// eighth of the sample rate, a step of 167 us at 48 kHz, its output interpolated between
/// (glides and key changes lag by up to a tick).
fn keyboard_every(q: Quality) -> usize {
    match q {
        Quality::NoCompromises | Quality::HighFidelity => 1,
        Quality::Potato => 8,
    }
}

/// The oscillators' and the filter's oversampling in each quality mode: in Potato the
/// oscillators at the sample rate and the filter at twice it (at the sample rate its
/// self-oscillation strays).
fn oversampling(q: Quality) -> (usize, usize) {
    match q {
        Quality::NoCompromises | Quality::HighFidelity => (4, 4),
        Quality::Potato => (1, 2),
    }
}

/// The contours' sub-rate (samples a tick) in each quality mode: in Potato a quarter of the
/// others'.
fn contour_every(q: Quality) -> usize {
    match q {
        Quality::NoCompromises | Quality::HighFidelity => 2,
        Quality::Potato => 8,
    }
}

impl ControlPart {
    /// One sample of the contours (at their sub-rate, interpolated: a block's delay) and
    /// the noise (its loads follow the panel), for the trigger contact and EXT. S-TRIG.
    pub fn tick(&mut self, p: &Panel, c: Contacts) -> ControlOut {
        let _fast = crate::ulp::FastScope::new(p.quality != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        if self.count.is_multiple_of(self.contour_every) {
            // The sub-rate as the quality mode has it, changed at a sub-sample's start.
            let every = contour_every(p.quality);
            if every != self.contour_every {
                self.contour_every = every;
                self.count = 0;
                self.contours.set_step(every as f64 / self.rate);
            }
            self.env_from = self.env_to;
            let cp = contour_panel(p, c.s_trig, &mut self.pots);
            self.contours.set_quality(p.quality);
            let o = self.contours.tick(c.gate, &cp);
            self.env_to = (o.filter, o.loudness);
        }
        laps.lap(crate::prof::Part::Contours);
        self.count += 1;
        let a = ((self.count - 1) % self.contour_every + 1) as f64 / self.contour_every as f64;
        let env_f = self.env_from.0 + (self.env_to.0 - self.env_from.0) * a;
        let env_l = self.env_from.1 + (self.env_to.1 - self.env_from.1) * a;
        // Board 3: the noise. Not computed while nothing hears it: its mixer channel off and
        // the modulation mix taking none of it (its gain exactly 0) or neither modulation
        // switch on; it resumes where it stopped (decisions.md R11). The front then takes
        // its last outputs, which reach nothing: the channel switched off adds no current,
        // and nothing times them in the modulation mix is zero.
        let heard = p.noise_on
            || ((p.osc_mod || p.filter_mod)
                && self
                    .noise_gain
                    .get(p.mod_mix, |m| self.mix.mix(m).gain_noise)
                    != 0.0);
        let noise = if heard {
            self.noise.set_quality(p.quality);
            self.noise.set_loads(noise_loads(p));
            self.last_noise = self.noise.tick();
            self.last_noise
        } else {
            self.last_noise
        };
        laps.lap(crate::prof::Part::Noise);
        ControlOut {
            env_f,
            env_l,
            noise,
        }
    }
}

impl BiasPart {
    /// The VCA's bias as it stands (before the next sample).
    pub fn drive(&self) -> VcaDrive {
        self.vca.drive()
    }

    /// One sample of the VCA's bias on the loudness contour `env_l` (V) and EXT. LOUDNESS
    /// (`None` empty), in quality mode `q`.
    pub fn tick(&mut self, env_l: f64, loudness: Option<f64>, q: Quality) -> VcaDrive {
        let _fast = crate::ulp::FastScope::new(q != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        // EXT. LOUDNESS (J3) from its jack's source, or its normal contact's 33K to +10 V.
        let (ext_l, r_j3) = match loudness {
            Some(v) => (v, 0.0),
            None => self.j3_normal,
        };
        if (self.vca.circuit.ext, self.vca.circuit.r_j3) != (ext_l, r_j3) {
            self.vca.circuit.ext = ext_l;
            self.vca.circuit.r_j3 = r_j3;
        }
        // With the jack plugged its tail can move at audio rate, which the VCA's own refresh
        // (it watches the contour) does not see: the bias is solved every sample
        // (vca_realtime.rs).
        // (In Potato from a table instead: `Vca::control`.)
        self.vca.potato_on = q == Quality::Potato;
        self.vca.control_every = if loudness.is_some() && q != Quality::Potato {
            1
        } else {
            self.vca_every
        };
        let d = self.vca.control(env_l);
        laps.lap(crate::prof::Part::VcaBias);
        d
    }
}

impl AudioPart {
    /// The OVERLOAD lamp after the last sample (0 off, 1 fully lit).
    pub fn overload(&self) -> f64 {
        self.pre.overload
    }

    /// The OVERLOAD lamp's brightest since the last call.
    pub fn take_overload_peak(&mut self) -> f64 {
        std::mem::take(&mut self.pre.overload_peak)
    }

    /// One output sample (see [`Voice::tick_jacks`]) on the contours `c` and the VCA's bias
    /// `d`: the preamplifier, the front, the back.
    pub fn tick(&mut self, p: &Panel, j: &Jacks, v_kbd: f64, c: &ControlOut, d: &VcaDrive) -> f64 {
        let i_pre = self.pre.tick(p, j.ext);
        let f = self.front.tick(p, j, v_kbd, c, i_pre);
        let (y, out) = self.back.tick(p, &f, d);
        self.last = (v_kbd, f.i_bus, y);
        out
    }
}

impl PreampPart {
    /// The external input's preamplifier over one sample with `ext` at the jack (a fraction
    /// of [`INPUT_VOLTS`]): the current SW10 and R46 put into the mixer's bus, A (none
    /// while SW10 is off, when the preamplifier does not run).
    pub fn tick(&mut self, p: &Panel, ext: f64) -> f64 {
        let _fast = crate::ulp::FastScope::new(p.quality != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        self.preamp
            .set_quality(match (self.accurate, p.quality, self.in_loop) {
                (true, Quality::Potato, InLoop::Circuit) => Quality::HighFidelity,
                _ => p.quality,
            });
        self.preamp.set_delayed(
            self.accurate && p.quality == Quality::Potato && self.in_loop == InLoop::Delayed,
        );
        // The external input: R9's divider into the preamplifier, which runs while SW10 is
        // on; R46 loads the bus either way (the front counts its conductance).
        let i = if p.ext_on {
            let (t, r) = self.volume.get(p.ext_volume, ext_volume);
            self.preamp.set_source_resistance(r);
            let o = self.preamp.tick(ext * INPUT_VOLTS * t);
            self.overload = o.lamp;
            self.overload_peak = self.overload_peak.max(o.lamp);
            self.ext_amp = o.amp;
            o.i_bus
        } else {
            self.overload = 0.0;
            0.0
        };
        laps.lap(crate::prof::Part::Preamp);
        i
    }

    /// The OVERLOAD lamp after the last sample, and its brightest since the last call to
    /// this (for the threaded voice, which reads them from the preamplifier's thread).
    pub fn lamp(&mut self) -> (f64, f64) {
        (self.overload, std::mem::take(&mut self.overload_peak))
    }
}

impl FrontPart {
    /// The front over one sample: the keyboard's voltage `v_kbd`, the contours and noise
    /// `c`, the preamplifier's current into the bus `i_pre`.
    pub fn tick(
        &mut self,
        p: &Panel,
        j: &Jacks,
        v_kbd: f64,
        c: &ControlOut,
        i_pre: f64,
    ) -> FrontOut {
        let _fast = crate::ulp::FastScope::new(p.quality != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        let p = *p;
        let (env_f, nz) = (c.env_f, c.noise);
        let mix = self.modulation.mix(p.mod_mix);
        let (noise_to_mod, noise_to_mixer) = if p.noise_pink {
            (nz.red, (nz.pink, 24e3))
        } else {
            (nz.pink, (nz.white, 0.0))
        };
        // The modulation line: the amplifier's output (from the last sample's oscillator 3,
        // computed below: a sample's delay in a path flat to 100 kHz) through R57 against the
        // wheel and the line's loads.
        let amp = mix.offset + mix.gain_noise * noise_to_mod + mix.gain_osc3 * self.last_osc3;
        let line_loads = LineLoads {
            osc_mod: p.osc_mod,
            filter_mod: p.filter_mod,
            filter_node: self.filter_node,
        };
        let r_wheel = self.wheel.get(p.mod_wheel, mod_wheel_r);
        let line = Modulation::line(amp, mix.r_out, r_wheel, &line_loads);
        self.line = line;
        let buses = Buses {
            bend: pitch_wheel_volts(p.pitch_wheel),
            // OSCILLATOR MODULATION off grounds the bus (A21).
            modulation: if p.osc_mod { line } else { 0.0 },
            // The oscillators' external control input (its jack's source holds the bus).
            ext: j.osc.unwrap_or(OPEN_BUS),
        };
        // The oscillators and the mixer: each channel a Norton source onto the bus, the
        // external input switched off loads it. Oscillator 3's switch output also feeds the
        // modulation mix through R23: its node is loaded by both.
        laps.lap(crate::prof::Part::Modulation);
        let mut i_bus = 0.0;
        if p.ext_on {
            i_bus += i_pre;
        }
        let mut g_bus = 1.0 / 33e3;
        let (_, mod_load_b) = Modulation::source_loads(p.mod_mix);
        let mut osc3_node = 0.0;
        for n in 0..3 {
            let op = p.osc[n];
            // An oscillator nothing hears is not computed: switched off in the mixer (its
            // channel's 33K then only loads the bus) and, for oscillator 3, not feeding a
            // modulation switch that is on. It resumes where it stopped (a free-running
            // oscillator's phase is arbitrary; performance).
            if !(op.on || (n == 2 && (p.osc_mod || p.filter_mod))) {
                g_bus += 1.0 / 33e3;
                continue;
            }
            let osc = match n {
                0 => Osc::One,
                1 => Osc::Two { freq: op.freq },
                _ => Osc::Three {
                    freq: op.freq,
                    control: p.osc3_control,
                },
            };
            let mut tuning = self.tunings[n];
            tuning.tune += p.tune.clamp(-1.0, 1.0) * TUNE_HALF_SPAN;
            let mut d = osc_drive_with(osc, &tuning, v_kbd, &buses, op.range);
            if j.detune[n] != 0.0 {
                // ENTROPY (the plug-in's): the oscillator off its calibration, as an offset at
                // its range tap (R43), which the octave trimmer set to an octave a step.
                d.inputs[5].v += j.detune[n] / 1200.0 * tuning.octave_step;
            }
            let i_in = d.apply(&mut self.vcos[n].expo);
            self.vcos[n].set_oversample(oversampling(p.quality).0);
            self.vcos[n].table_on = p.quality == Quality::Potato;
            let o = self.vcos[n].tick(i_in, op.waveform.width());
            self.last_i[n] = self.vcos[n].timing_current();
            // Oscillator 3's reverse sawtooth stage runs while its sawtooth or reverse
            // sawtooth is selected (R171 loads the sawtooth).
            let second = matches!(
                op.waveform,
                Waveform::Sawtooth | Waveform::SharkTooth | Waveform::ReverseSawtooth
            );
            let rev = if n == 2 && second {
                self.revsaw.set_quality(p.quality);
                Some(self.revsaw.tick(o.saw))
            } else {
                None
            };
            let (v_src, r_src) = waveform_source(op.waveform, &o, rev.as_ref());
            let (i, g) = if n == 2 {
                // The switch's output node under its two loads.
                let z_ch = channel_load(op.volume, 33e3);
                let z = z_ch * mod_load_b / (z_ch + mod_load_b);
                osc3_node = v_src * z / (r_src + z);
                channel(osc3_node, 0.0, op.volume, op.on, 33e3)
            } else {
                channel(v_src, r_src, op.volume, op.on, 33e3)
            };
            i_bus += i;
            g_bus += g;
        }
        laps.lap(crate::prof::Part::Oscillators);
        self.last_osc3 = osc3_node;
        // The noise's channel (R48 11K).
        let (i, g) = channel(
            noise_to_mixer.0,
            noise_to_mixer.1,
            p.noise_volume,
            p.noise_on,
            11e3,
        );
        i_bus += i;
        g_bus += g;
        // The filter's control node: CUTOFF, AMOUNT OF CONTOUR, KEYBOARD CONTROL 1 and 2,
        // R52 (the modulation line with FILTER MODULATION on, else grounded) and R51 (the
        // external control's jack, empty: grounded).
        let pos = p.cutoff.clamp(0.0, 1.0);
        let r_cut = 200e3 + 5e3 * pos * (1.0 - pos);
        let amt = p.contour_amount.clamp(0.0, 1.0);
        let open = Input {
            r: f64::INFINITY,
            v: 0.0,
        };
        let ins = [
            Input {
                r: r_cut,
                v: -10.0 + 20.0 * pos,
            },
            Input {
                r: 47e3 + 5e3 * amt * (1.0 - amt),
                v: env_f * amt,
            },
            if p.keyboard_control_1 {
                Input { r: 300e3, v: v_kbd }
            } else {
                open
            },
            if p.keyboard_control_2 {
                Input { r: 150e3, v: v_kbd }
            } else {
                open
            },
            Input {
                r: 33e3,
                v: if p.filter_mod { line } else { 0.0 },
            },
            Input {
                r: 100e3,
                v: if j.cutoff == 0.0 {
                    j.filter.unwrap_or(0.0)
                } else {
                    // ENTROPY (the plug-in's): the cutoff off its calibration, as volts at R51.
                    j.filter.unwrap_or(0.0) + j.cutoff * FILTER_VOLTS_PER_OCTAVE
                },
            },
        ];
        let (i_node, g_node) = self.expo.node(&ins);
        let (i0, v_node) = self.table.current_and_node(i_node, g_node);
        self.i0 = i0;
        self.filter_node = v_node;
        laps.lap(crate::prof::Part::ControlNode);
        FrontOut { i_bus, g_bus, i0 }
    }
}

impl BackPart {
    /// The back over one sample: the front's `f` and the VCA's bias `d` (from
    /// [`ControlPart`]). The filter's output, V, and the output sample.
    pub fn tick(&mut self, p: &Panel, f: &FrontOut, d: &VcaDrive) -> (f64, f64) {
        let _fast = crate::ulp::FastScope::new(p.quality != Quality::NoCompromises);
        let mut laps = crate::prof::Laps::start();
        self.vcf.circuit.r14 = self.emphasis.get(p.emphasis, emphasis_r14);
        self.vcf.set_oversample(oversampling(p.quality).1);
        self.vcf.set_quality(p.quality);
        let y = self.vcf.tick(f.i_bus, f.g_bus, f.i0);
        laps.lap(crate::prof::Part::Filter);
        self.vca.a440 = self.a440.tick(p.a440);
        self.vca.plain_signal = p.quality != Quality::NoCompromises;
        let out = self.vca.signal(y, d);
        laps.lap(crate::prof::Part::Vca);
        (y, out / 5.0)
    }
}
