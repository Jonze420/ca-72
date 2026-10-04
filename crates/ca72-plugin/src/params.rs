//! The plug-in's parameters: the front panel's and left hand controller's controls in their
//! dials' units, MIDI BEND RANGE, and the plug-in's own POLY, VOICES, ENTROPY and SPREAD.
//! Their ids are stable (a saved session's settings are found by them): never rename one.

use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use ca72::modulation::PITCH_WHEEL_SEMITONES;
use ca72::tuning::Range;
use ca72::voice::{ContourKnobs, OscPanel, Panel, Quality, Waveform};
use nih_plug::prelude::*;

use crate::engine::{Controls, POLY_VOICES};

/// RANGE's positions.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Footage {
    #[id = "lo"]
    #[name = "LO"]
    Lo,
    #[id = "32"]
    #[name = "32'"]
    R32,
    #[id = "16"]
    #[name = "16'"]
    R16,
    #[id = "8"]
    #[name = "8'"]
    R8,
    #[id = "4"]
    #[name = "4'"]
    R4,
    #[id = "2"]
    #[name = "2'"]
    R2,
}

impl From<Footage> for Range {
    fn from(f: Footage) -> Range {
        match f {
            Footage::Lo => Range::Lo,
            Footage::R32 => Range::R32,
            Footage::R16 => Range::R16,
            Footage::R8 => Range::R8,
            Footage::R4 => Range::R4,
            Footage::R2 => Range::R2,
        }
    }
}

/// WAVEFORM on oscillators 1 and 2.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    #[id = "triangle"]
    Triangle,
    #[id = "shark_tooth"]
    #[name = "Shark Tooth"]
    SharkTooth,
    #[id = "sawtooth"]
    Sawtooth,
    #[id = "square"]
    Square,
    #[id = "wide_rectangle"]
    #[name = "Wide Rectangle"]
    WideRectangle,
    #[id = "narrow_rectangle"]
    #[name = "Narrow Rectangle"]
    NarrowRectangle,
}

impl From<Wave> for Waveform {
    fn from(w: Wave) -> Waveform {
        match w {
            Wave::Triangle => Waveform::Triangle,
            Wave::SharkTooth => Waveform::SharkTooth,
            Wave::Sawtooth => Waveform::Sawtooth,
            Wave::Square => Waveform::Square,
            Wave::WideRectangle => Waveform::WideRectangle,
            Wave::NarrowRectangle => Waveform::NarrowRectangle,
        }
    }
}

/// WAVEFORM on oscillator 3: its second position the reverse sawtooth.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave3 {
    #[id = "triangle"]
    Triangle,
    #[id = "reverse_sawtooth"]
    #[name = "Reverse Sawtooth"]
    ReverseSawtooth,
    #[id = "sawtooth"]
    Sawtooth,
    #[id = "square"]
    Square,
    #[id = "wide_rectangle"]
    #[name = "Wide Rectangle"]
    WideRectangle,
    #[id = "narrow_rectangle"]
    #[name = "Narrow Rectangle"]
    NarrowRectangle,
}

impl From<Wave3> for Waveform {
    fn from(w: Wave3) -> Waveform {
        match w {
            Wave3::Triangle => Waveform::Triangle,
            Wave3::ReverseSawtooth => Waveform::ReverseSawtooth,
            Wave3::Sawtooth => Waveform::Sawtooth,
            Wave3::Square => Waveform::Square,
            Wave3::WideRectangle => Waveform::WideRectangle,
            Wave3::NarrowRectangle => Waveform::NarrowRectangle,
        }
    }
}

/// The NOISE switch: white (pink to the modulation) or pink (red to the modulation).
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseColor {
    #[id = "white"]
    White,
    #[id = "pink"]
    Pink,
}

#[derive(Params)]
pub struct Ca72Params {
    /// The noise generator's seed, saved with the session so that it renders the same noise
    /// again.
    #[persist = "noise_seed"]
    pub noise_seed: Arc<AtomicU64>,
    /// The editor's width in logical pixels (its height follows the panel's proportions); 0
    /// until it is resized, when it opens at a share of the screen.
    #[persist = "editor_width"]
    pub editor_width: Arc<AtomicU32>,
    /// The preset the plug-in was last set to (decisions.md R10), empty when none: saved
    /// with the session, so the bar names it again.
    #[persist = "preset"]
    pub preset: Arc<RwLock<String>>,

    /// POWER off: the host's bypass, the output faded out.
    #[id = "bypass"]
    pub bypass: BoolParam,

    // CONTROLLERS
    /// TUNE in its dial's units (printed -2..+2, -2.5..+2.5 at its ends).
    #[id = "tune"]
    pub tune: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    /// MODULATION MIX: oscillator 3 (0) to the noise (10).
    #[id = "mod_mix"]
    pub mod_mix: FloatParam,
    #[id = "osc_mod"]
    pub osc_mod: BoolParam,
    #[id = "osc3_control"]
    pub osc3_control: BoolParam,

    // OSCILLATOR BANK
    #[id = "osc1_range"]
    pub osc1_range: EnumParam<Footage>,
    #[id = "osc1_waveform"]
    pub osc1_waveform: EnumParam<Wave>,
    #[id = "osc2_range"]
    pub osc2_range: EnumParam<Footage>,
    /// FREQUENCY in its dial's units: a semitone a mark, 7 printed at 140 degrees, 7.5 at
    /// the ends of the knob's travel.
    #[id = "osc2_frequency"]
    pub osc2_frequency: FloatParam,
    #[id = "osc2_waveform"]
    pub osc2_waveform: EnumParam<Wave>,
    #[id = "osc3_range"]
    pub osc3_range: EnumParam<Footage>,
    #[id = "osc3_frequency"]
    pub osc3_frequency: FloatParam,
    #[id = "osc3_waveform"]
    pub osc3_waveform: EnumParam<Wave3>,

    // MIXER
    #[id = "osc1_on"]
    pub osc1_on: BoolParam,
    #[id = "osc1_volume"]
    pub osc1_volume: FloatParam,
    /// EXTERNAL INPUT: the side chain, through the preamplifier.
    #[id = "ext_on"]
    pub ext_on: BoolParam,
    #[id = "ext_volume"]
    pub ext_volume: FloatParam,
    #[id = "osc2_on"]
    pub osc2_on: BoolParam,
    #[id = "osc2_volume"]
    pub osc2_volume: FloatParam,
    #[id = "noise_on"]
    pub noise_on: BoolParam,
    #[id = "noise_volume"]
    pub noise_volume: FloatParam,
    #[id = "noise_type"]
    pub noise_type: EnumParam<NoiseColor>,
    #[id = "osc3_on"]
    pub osc3_on: BoolParam,
    #[id = "osc3_volume"]
    pub osc3_volume: FloatParam,

    // MODIFIERS
    #[id = "filter_mod"]
    pub filter_mod: BoolParam,
    #[id = "keyboard_control_1"]
    pub keyboard_control_1: BoolParam,
    #[id = "keyboard_control_2"]
    pub keyboard_control_2: BoolParam,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "emphasis"]
    pub emphasis: FloatParam,
    #[id = "contour_amount"]
    pub contour_amount: FloatParam,
    #[id = "filter_attack"]
    pub filter_attack: FloatParam,
    #[id = "filter_decay"]
    pub filter_decay: FloatParam,
    #[id = "filter_sustain"]
    pub filter_sustain: FloatParam,
    #[id = "loudness_attack"]
    pub loudness_attack: FloatParam,
    #[id = "loudness_decay"]
    pub loudness_decay: FloatParam,
    #[id = "loudness_sustain"]
    pub loudness_sustain: FloatParam,

    // OUTPUT
    #[id = "volume"]
    pub volume: FloatParam,
    #[id = "main_output"]
    pub main_output: BoolParam,
    #[id = "a440"]
    pub a440: BoolParam,

    // LEFT HAND CONTROLLER
    #[id = "glide_on"]
    pub glide_on: BoolParam,
    /// On, the contours decay after a key's release.
    #[id = "decay_on"]
    pub decay_on: BoolParam,
    /// The PITCH wheel, -1..1, 0 in its detent.
    #[id = "pitch_wheel"]
    pub pitch_wheel: FloatParam,
    /// The MODULATION wheel, 1 fully forward.
    #[id = "mod_wheel"]
    pub mod_wheel: FloatParam,

    // MIDI
    /// How far a MIDI keyboard's pitch bend, fully up or down, moves the PITCH wheel, in
    /// semitones: up to the wheel's whole travel.
    #[id = "midi_bend_range"]
    pub midi_bend_range: FloatParam,

    // The plug-in's own, not the instrument's (decisions.md, "POLY", "ANALOG and SPREAD").
    /// POLY: one instrument per voice (VOICES of them); off, the one with its keyboard
    /// circuit's lowest-note priority.
    #[id = "poly"]
    pub poly: BoolParam,
    /// VOICES: how many POLY plays, 2 to 10 (kept while POLY is off).
    #[id = "voices"]
    pub voices: IntParam,
    /// ENTROPY: each voice its own parts and its oscillators drifting, 0 to 100 %.
    #[id = "entropy"]
    pub entropy: FloatParam,
    /// SPREAD: POLY's voices across the stereo field, 0 to 100 %.
    #[id = "spread"]
    pub spread: FloatParam,
    /// FEEDBACK, the phones' VOLUME knob (0 to 10): the output cabled into EXTERNAL INPUT,
    /// the level sent following the knob's taper (`ca72::voice::feedback_law`).
    #[id = "feedback"]
    pub feedback: FloatParam,
    /// LOCK: the oscillators identical, the circuit as drawn, its level matched (off: each
    /// keeps a small mismatch of its own; decisions.md R9).
    #[id = "lock"]
    pub lock: BoolParam,
}

/// An amount, 0 to 100 %.
fn percent(name: &str) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: 0.0,
            max: 100.0,
        },
    )
    .with_unit(" %")
    .with_value_to_string(formatters::v2s_f32_rounded(0))
}

/// A knob over `lo..hi` in its dial's units.
fn dial(name: &str, lo: f32, hi: f32, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: lo, max: hi })
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

/// A 0 to 10 knob.
fn ten(name: &str, default: f32) -> FloatParam {
    dial(name, 0.0, 10.0, default)
}

/// A fresh seed for a new instance's noise generator.
fn fresh_seed() -> u64 {
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

impl Default for Ca72Params {
    /// The panel as the instrument's default patch has it: oscillator 1 alone on a sawtooth
    /// at 8', the filter half open, MAIN OUTPUT at 10.
    fn default() -> Self {
        Ca72Params {
            noise_seed: Arc::new(AtomicU64::new(fresh_seed())),
            editor_width: Arc::new(AtomicU32::new(0)),
            preset: Arc::new(RwLock::new(String::new())),
            bypass: BoolParam::new("Bypass", false).make_bypass(),
            tune: dial("Tune", -2.5, 2.5, 0.0),
            glide: ten("Glide", 0.0),
            mod_mix: ten("Modulation Mix", 0.0),
            osc_mod: BoolParam::new("Oscillator Modulation", false),
            osc3_control: BoolParam::new("Osc 3 Control", true),
            osc1_range: EnumParam::new("Osc 1 Range", Footage::R8),
            osc1_waveform: EnumParam::new("Osc 1 Waveform", Wave::Sawtooth),
            osc2_range: EnumParam::new("Osc 2 Range", Footage::R8),
            osc2_frequency: dial("Osc 2 Frequency", -7.5, 7.5, 0.0),
            osc2_waveform: EnumParam::new("Osc 2 Waveform", Wave::Sawtooth),
            osc3_range: EnumParam::new("Osc 3 Range", Footage::R8),
            osc3_frequency: dial("Osc 3 Frequency", -7.5, 7.5, 0.0),
            osc3_waveform: EnumParam::new("Osc 3 Waveform", Wave3::Sawtooth),
            osc1_on: BoolParam::new("Osc 1 On", true),
            osc1_volume: ten("Osc 1 Volume", 8.0),
            ext_on: BoolParam::new("External Input On", false),
            ext_volume: ten("External Input Volume", 5.0),
            osc2_on: BoolParam::new("Osc 2 On", false),
            osc2_volume: ten("Osc 2 Volume", 8.0),
            noise_on: BoolParam::new("Noise On", false),
            noise_volume: ten("Noise Volume", 8.0),
            noise_type: EnumParam::new("Noise Color", NoiseColor::White),
            osc3_on: BoolParam::new("Osc 3 On", false),
            osc3_volume: ten("Osc 3 Volume", 8.0),
            filter_mod: BoolParam::new("Filter Modulation", false),
            keyboard_control_1: BoolParam::new("Keyboard Control 1", true),
            keyboard_control_2: BoolParam::new("Keyboard Control 2", false),
            cutoff: dial("Cutoff Frequency", -5.0, 5.0, 0.0),
            emphasis: ten("Emphasis", 0.0),
            contour_amount: ten("Amount of Contour", 5.0),
            filter_attack: ten("Filter Attack Time", 0.0),
            filter_decay: ten("Filter Decay Time", 4.0),
            filter_sustain: ten("Filter Sustain Level", 5.0),
            loudness_attack: ten("Loudness Attack Time", 0.0),
            loudness_decay: ten("Loudness Decay Time", 4.0),
            loudness_sustain: ten("Loudness Sustain Level", 8.0),
            volume: ten("Volume", 10.0),
            main_output: BoolParam::new("Main Output", true),
            a440: BoolParam::new("A-440", false),
            glide_on: BoolParam::new("Glide Switch", false),
            decay_on: BoolParam::new("Decay Switch", true),
            pitch_wheel: dial("Pitch Wheel", -1.0, 1.0, 0.0),
            mod_wheel: dial("Modulation Wheel", 0.0, 1.0, 0.0),
            midi_bend_range: dial("MIDI Bend Range", 0.0, PITCH_WHEEL_SEMITONES as f32, 2.0)
                .with_unit(" semitones"),
            poly: BoolParam::new("Poly", false),
            voices: IntParam::new(
                "Voices",
                POLY_VOICES.1 as i32,
                IntRange::Linear {
                    min: POLY_VOICES.0 as i32,
                    max: POLY_VOICES.2 as i32,
                },
            ),
            entropy: percent("Entropy"),
            spread: percent("Spread"),
            feedback: ten("Feedback", 0.0),
            lock: BoolParam::new("Lock (oscillators identical)", false),
        }
    }
}

impl std::fmt::Debug for Ca72Params {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ca72Params")
            .field("noise_seed", &self.seed())
            .field("controls", &self.controls())
            .finish()
    }
}

impl Ca72Params {
    pub fn seed(&self) -> u64 {
        self.noise_seed.load(Ordering::Relaxed)
    }

    /// The engine's controls from the parameters (dial units in, the voice's 0..1 out).
    pub fn controls(&self) -> Controls {
        let ten = |p: &FloatParam| f64::from(p.value()) / 10.0;
        let freq = |p: &FloatParam| (f64::from(p.value()) + 7.5) / 15.0;
        let osc = [
            OscPanel {
                range: self.osc1_range.value().into(),
                waveform: self.osc1_waveform.value().into(),
                on: self.osc1_on.value(),
                volume: ten(&self.osc1_volume),
                freq: ca72::tuning::FREQ_CENTRE,
            },
            OscPanel {
                range: self.osc2_range.value().into(),
                waveform: self.osc2_waveform.value().into(),
                on: self.osc2_on.value(),
                volume: ten(&self.osc2_volume),
                freq: freq(&self.osc2_frequency),
            },
            OscPanel {
                range: self.osc3_range.value().into(),
                waveform: self.osc3_waveform.value().into(),
                on: self.osc3_on.value(),
                volume: ten(&self.osc3_volume),
                freq: freq(&self.osc3_frequency),
            },
        ];
        let panel = Panel {
            osc,
            osc3_control: self.osc3_control.value(),
            cutoff: (f64::from(self.cutoff.value()) + 5.0) / 10.0,
            emphasis: ten(&self.emphasis),
            contour_amount: ten(&self.contour_amount),
            keyboard_control_1: self.keyboard_control_1.value(),
            keyboard_control_2: self.keyboard_control_2.value(),
            filter_contour: ContourKnobs {
                attack: ten(&self.filter_attack),
                decay: ten(&self.filter_decay),
                sustain: ten(&self.filter_sustain),
            },
            loudness_contour: ContourKnobs {
                attack: ten(&self.loudness_attack),
                decay: ten(&self.loudness_decay),
                sustain: ten(&self.loudness_sustain),
            },
            glide: ten(&self.glide),
            glide_on: self.glide_on.value(),
            decay: self.decay_on.value(),
            noise_on: self.noise_on.value(),
            noise_volume: ten(&self.noise_volume),
            noise_pink: self.noise_type.value() == NoiseColor::Pink,
            mod_mix: ten(&self.mod_mix),
            osc_mod: self.osc_mod.value(),
            filter_mod: self.filter_mod.value(),
            pitch_wheel: f64::from(self.pitch_wheel.value()),
            mod_wheel: f64::from(self.mod_wheel.value()),
            ext_volume: ten(&self.ext_volume),
            ext_on: self.ext_on.value(),
            a440: self.a440.value(),
            tune: f64::from(self.tune.value()) / 2.5,
            quality: Quality::default(),
        };
        Controls {
            panel,
            volume: ten(&self.volume),
            main_output: self.main_output.value(),
            bend_range: f64::from(self.midi_bend_range.value()),
            power: !self.bypass.value(),
            poly: self.poly.value(),
            voices: usize::try_from(self.voices.value()).unwrap_or(POLY_VOICES.1),
            entropy: f64::from(self.entropy.value()) / 100.0,
            spread: f64::from(self.spread.value()) / 100.0,
            // The knob's travel through its taper (decisions.md R8).
            feedback: ca72::voice::feedback_law(f64::from(self.feedback.value()) / 10.0),
            lock: self.lock.value(),
        }
    }
}
