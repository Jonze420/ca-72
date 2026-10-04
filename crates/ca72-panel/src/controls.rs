//! The panel's controls: where each is, what it looks like, and the parameter it operates.

/// A knob's printed dial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dial {
    /// 0 to 10.
    Ten,
    /// TUNE: –2 to 2 (semitones, as printed).
    Tune,
    /// CUTOFF FREQUENCY: –4 to 4.
    Cutoff,
    /// An oscillator's FREQUENCY: –7 to 7 semitones.
    Freq,
    /// The contours' ATTACK and DECAY TIME, in time.
    Time,
    /// MODULATION MIX: 0 to 10 between OSC. 3 and NOISE.
    ModMix,
}

/// A selector's legends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Legends {
    Ranges,
    /// Oscillators 1 and 2's waveforms.
    Waves12,
    /// Oscillator 3's (a reverse sawtooth second).
    Waves3,
}

/// A rocker's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Blue,
    Red,
    Black,
    Ivory,
}

/// Where a rocker's "on" end is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orient {
    Right,
    Top,
    Bottom,
}

/// A wheel's mark: the PITCH wheel's line, the MODULATION wheel's dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Line,
    Dot,
}

/// What a control is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Knob {
        dial: Dial,
        big: bool,
    },
    /// Six positions (RANGE, WAVEFORM).
    Selector(Legends),
    /// A switch, `w` by `h` as seen.
    Rocker {
        w: f64,
        h: f64,
        colour: Colour,
        orient: Orient,
    },
    /// `detent`: centred (PITCH); `span`: its turn over its travel, in degrees.
    Wheel {
        mark: Mark,
        detent: bool,
        span: f64,
    },
}

/// The part of the drawing a control's position is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The panel, its origin at its top left corner.
    Panel,
    /// The left hand controller's face, beside the panel.
    Column,
}

/// A control of the panel, operating the parameter of the same function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Control {
    /// Its parameter's id.
    pub param: &'static str,
    /// Its name, as its tip gives it.
    pub label: &'static str,
    pub place: Place,
    /// Its centre in its place.
    pub x: f64,
    pub y: f64,
    pub kind: Kind,
}

impl Control {
    /// Its centre in the drawing.
    pub fn centre(&self) -> (f64, f64) {
        let (ox, oy) = match self.place {
            Place::Panel => (crate::art::COL, crate::art::TOP),
            Place::Column => (crate::art::LH_X, crate::art::TOP),
        };
        (ox + self.x, oy + self.y)
    }
}

const fn knob(param: &'static str, label: &'static str, x: f64, y: f64, dial: Dial) -> Control {
    Control {
        param,
        label,
        place: Place::Panel,
        x,
        y,
        kind: Kind::Knob { dial, big: false },
    }
}

const fn big(param: &'static str, label: &'static str, x: f64, y: f64) -> Control {
    Control {
        param,
        label,
        place: Place::Panel,
        x,
        y,
        kind: Kind::Knob {
            dial: Dial::Freq,
            big: true,
        },
    }
}

const fn selector(
    param: &'static str,
    label: &'static str,
    x: f64,
    y: f64,
    legends: Legends,
) -> Control {
    Control {
        param,
        label,
        place: Place::Panel,
        x,
        y,
        kind: Kind::Selector(legends),
    }
}

#[allow(clippy::too_many_arguments)]
const fn rocker(
    param: &'static str,
    label: &'static str,
    place: Place,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    colour: Colour,
    orient: Orient,
) -> Control {
    Control {
        param,
        label,
        place,
        x,
        y,
        kind: Kind::Rocker {
            w,
            h,
            colour,
            orient,
        },
    }
}

const fn switch(
    param: &'static str,
    label: &'static str,
    x: f64,
    y: f64,
    colour: Colour,
) -> Control {
    rocker(
        param,
        label,
        Place::Panel,
        x,
        y,
        108.0,
        50.0,
        colour,
        Orient::Right,
    )
}

/// The MIXER's rows.
const ROWS: [f64; 5] = [170.0, 274.0, 377.0, 481.0, 583.0];

/// Every control, in the order they are drawn.
pub const CONTROLS: [Control; 44] = [
    // CONTROLLERS.
    knob("tune", "TUNE", 229.0, 273.0, Dial::Tune),
    switch(
        "osc_mod",
        "OSCILLATOR MODULATION",
        444.0,
        269.0,
        Colour::Red,
    ),
    knob("glide", "GLIDE", 126.0, 480.0, Dial::Ten),
    knob("mod_mix", "MODULATION MIX", 332.0, 480.0, Dial::ModMix),
    // OSCILLATOR BANK.
    selector(
        "osc1_range",
        "OSCILLATOR-1 RANGE",
        617.0,
        163.0,
        Legends::Ranges,
    ),
    selector(
        "osc2_range",
        "OSCILLATOR-2 RANGE",
        617.0,
        370.0,
        Legends::Ranges,
    ),
    selector(
        "osc3_range",
        "OSCILLATOR-3 RANGE",
        617.0,
        576.0,
        Legends::Ranges,
    ),
    big("osc2_frequency", "OSCILLATOR-2 FREQUENCY", 820.0, 373.0),
    big("osc3_frequency", "OSCILLATOR-3 FREQUENCY", 820.0, 581.0),
    selector(
        "osc1_waveform",
        "OSCILLATOR-1 WAVEFORM",
        1021.0,
        163.0,
        Legends::Waves12,
    ),
    selector(
        "osc2_waveform",
        "OSCILLATOR-2 WAVEFORM",
        1021.0,
        370.0,
        Legends::Waves12,
    ),
    selector(
        "osc3_waveform",
        "OSCILLATOR-3 WAVEFORM",
        1021.0,
        576.0,
        Legends::Waves3,
    ),
    rocker(
        "osc3_control",
        "OSC. 3 CONTROL",
        Place::Panel,
        480.0,
        585.0,
        55.0,
        111.0,
        Colour::Red,
        Orient::Top,
    ),
    // MIXER: the oscillators' VOLUME at the left, EXTERNAL INPUT and NOISE at the right.
    knob(
        "osc1_volume",
        "OSCILLATOR-1 VOLUME",
        1220.0,
        ROWS[0],
        Dial::Ten,
    ),
    switch("osc1_on", "OSCILLATOR-1 ON", 1419.0, ROWS[0], Colour::Blue),
    knob(
        "ext_volume",
        "EXTERNAL INPUT VOLUME",
        1621.0,
        ROWS[1],
        Dial::Ten,
    ),
    switch("ext_on", "EXTERNAL INPUT ON", 1419.0, ROWS[1], Colour::Blue),
    knob(
        "osc2_volume",
        "OSCILLATOR-2 VOLUME",
        1220.0,
        ROWS[2],
        Dial::Ten,
    ),
    switch("osc2_on", "OSCILLATOR-2 ON", 1419.0, ROWS[2], Colour::Blue),
    knob("noise_volume", "NOISE VOLUME", 1621.0, ROWS[3], Dial::Ten),
    switch("noise_on", "NOISE ON", 1419.0, ROWS[3], Colour::Blue),
    knob(
        "osc3_volume",
        "OSCILLATOR-3 VOLUME",
        1220.0,
        ROWS[4],
        Dial::Ten,
    ),
    switch("osc3_on", "OSCILLATOR-3 ON", 1419.0, ROWS[4], Colour::Blue),
    rocker(
        "noise_type",
        "NOISE (WHITE or PINK)",
        Place::Panel,
        1766.0,
        481.0,
        44.0,
        114.0,
        Colour::Blue,
        Orient::Bottom,
    ),
    // Across MIXER and MODIFIERS: the filter's modulation and keyboard control.
    switch(
        "filter_mod",
        "FILTER MODULATION",
        1907.0,
        169.0,
        Colour::Red,
    ),
    rocker(
        "keyboard_control_1",
        "KEYBOARD CONTROL 1",
        Place::Panel,
        1907.0,
        264.0,
        108.0,
        48.0,
        Colour::Red,
        Orient::Right,
    ),
    rocker(
        "keyboard_control_2",
        "KEYBOARD CONTROL 2",
        Place::Panel,
        1907.0,
        364.0,
        108.0,
        48.0,
        Colour::Red,
        Orient::Right,
    ),
    // MODIFIERS.
    knob("cutoff", "CUTOFF FREQUENCY", 2081.0, 170.0, Dial::Cutoff),
    knob("emphasis", "EMPHASIS", 2281.0, 170.0, Dial::Ten),
    knob(
        "contour_amount",
        "AMOUNT OF CONTOUR",
        2483.0,
        170.0,
        Dial::Ten,
    ),
    knob(
        "filter_attack",
        "FILTER CONTOUR ATTACK TIME",
        2080.0,
        358.0,
        Dial::Time,
    ),
    knob(
        "filter_decay",
        "FILTER CONTOUR DECAY TIME",
        2281.0,
        358.0,
        Dial::Time,
    ),
    knob(
        "filter_sustain",
        "FILTER CONTOUR SUSTAIN LEVEL",
        2483.0,
        358.0,
        Dial::Ten,
    ),
    knob(
        "loudness_attack",
        "LOUDNESS CONTOUR ATTACK TIME",
        2080.0,
        579.0,
        Dial::Time,
    ),
    knob(
        "loudness_decay",
        "LOUDNESS CONTOUR DECAY TIME",
        2281.0,
        579.0,
        Dial::Time,
    ),
    knob(
        "loudness_sustain",
        "LOUDNESS CONTOUR SUSTAIN LEVEL",
        2483.0,
        579.0,
        Dial::Ten,
    ),
    // OUTPUT.
    knob("volume", "MAIN OUTPUT VOLUME", 2680.0, 167.0, Dial::Ten),
    rocker(
        "main_output",
        "MAIN OUTPUT",
        Place::Panel,
        2853.0,
        165.0,
        110.0,
        48.0,
        Colour::Blue,
        Orient::Right,
    ),
    rocker(
        "a440",
        "A-440",
        Place::Panel,
        2682.0,
        375.0,
        112.0,
        50.0,
        Colour::Blue,
        Orient::Right,
    ),
    // The phones' VOLUME is the plug-in's FEEDBACK (decisions.md R8): the PHONES output
    // cabled into EXTERNAL INPUT, as players overdrive the instrument.
    knob(
        "feedback",
        "FEEDBACK (the phones' VOLUME: the output into EXTERNAL INPUT)",
        2680.0,
        580.0,
        Dial::Ten,
    ),
    // The left hand controller.
    rocker(
        "glide_on",
        "GLIDE",
        Place::Column,
        crate::art::LH_ROCKER,
        crate::art::LH_ROWS[0],
        82.0,
        38.0,
        Colour::Ivory,
        Orient::Right,
    ),
    rocker(
        "decay_on",
        "DECAY",
        Place::Column,
        crate::art::LH_ROCKER,
        crate::art::LH_ROWS[1],
        82.0,
        38.0,
        Colour::Ivory,
        Orient::Right,
    ),
    Control {
        param: "pitch_wheel",
        label: "PITCH",
        place: Place::Column,
        x: crate::art::LH_WHEELS[0],
        y: crate::art::WHEEL_Y,
        kind: Kind::Wheel {
            mark: Mark::Line,
            detent: true,
            span: 70.0,
        },
    },
    Control {
        param: "mod_wheel",
        label: "MODULATION",
        place: Place::Column,
        x: crate::art::LH_WHEELS[1],
        y: crate::art::WHEEL_Y,
        kind: Kind::Wheel {
            mark: Mark::Dot,
            detent: false,
            span: 76.0,
        },
    },
];

/// The index of the control operating `param`.
pub fn index(param: &str) -> Option<usize> {
    CONTROLS.iter().position(|c| c.param == param)
}

/// FEEDBACK's tip while it is silent ([`feedback_silent`]).
pub const FEEDBACK_SILENT: &str =
    "FEEDBACK (silent: switch EXTERNAL INPUT on in the MIXER and turn its VOLUME up)";

/// Whether FEEDBACK is silent at these values (normalized, by index in [`CONTROLS`]): it is
/// heard through the MIXER's EXTERNAL INPUT, so nothing goes back while that is switched off
/// or its VOLUME is at 0. Its knob is then dimmed, still turnable, and its tip says why (the
/// owner, 2026-10-02).
pub fn feedback_silent(values: &[f64; CONTROLS.len()]) -> bool {
    let at = |param| index(param).map_or(0.0, |i| values[i]);
    !(at("ext_on") >= 0.5 && at("ext_volume") > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_is_silent_while_external_input_is_closed() {
        let mut v = [0.5; CONTROLS.len()];
        let (on, volume) = (
            index("ext_on").expect("a control"),
            index("ext_volume").expect("a control"),
        );
        assert!(!feedback_silent(&v));
        v[on] = 0.0;
        assert!(feedback_silent(&v));
        v[on] = 1.0;
        v[volume] = 0.0;
        assert!(feedback_silent(&v));
        v[volume] = 0.01;
        assert!(!feedback_silent(&v));
    }
}
