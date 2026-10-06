//! The CA-72 as a VST3 and CLAP plug-in: the circuit model's voice in its real-time quality,
//! played by MIDI, its EXTERNAL INPUT the side chain; one voice, or with POLY one per note
//! (up to ten), placed across the stereo output by SPREAD.

pub mod character;
pub mod editor;
pub mod engine;
pub mod helper;
pub mod library;
pub mod params;
pub mod pool;
pub mod presets;
pub mod update;

use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug::wrapper::state::ParamValue;

use crate::editor::{Ca72Editor, Meters};
use crate::engine::{
    ALL_NOTES_OFF, ALL_SOUND_OFF, Engine, Event, MODULATION_WHEEL, RESET_CONTROLLERS,
};
use crate::helper::{Helper, MEND, SERVE};
use crate::params::Ca72Params;

pub struct Ca72 {
    params: Arc<Ca72Params>,
    engine: Engine,
    meters: Arc<Meters>,
    /// The host's sample rate (for each block's deadline).
    rate: f64,
    /// The plug-in's own thread for what the audio thread asks of it (decisions.md R23),
    /// started when the plug-in is first initialised (none if the system would not start it).
    helper: Option<Helper>,
}

impl std::fmt::Debug for Ca72 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ca72")
            .field("engine", &self.engine)
            .finish()
    }
}

impl Default for Ca72 {
    fn default() -> Self {
        Ca72::with_params(Ca72Params::default())
    }
}

impl Ca72 {
    fn with_params(params: Ca72Params) -> Ca72 {
        Ca72 {
            params: Arc::new(params),
            engine: Engine::new(),
            meters: Arc::new(Meters::default()),
            rate: 48_000.0,
            helper: None,
        }
    }
}

impl Drop for Ca72 {
    /// The helper stopped and joined first (a mend in progress stops before its next voice),
    /// then the voices' workers, as the engine drops: when the host's destroy returns, no
    /// thread of the plug-in's runs, and the host may unload it (decisions.md R23).
    fn drop(&mut self) {
        drop(self.helper.take());
    }
}

/// The side chain at sample `i`, its channels mixed to mono: a channel shorter than the block
/// (one the host did not supply may be) or a sample that is not finite counts as 0
/// (decisions.md R18).
fn mono(channels: &[&mut [f32]], i: usize) -> f32 {
    if channels.is_empty() {
        return 0.0;
    }
    channels
        .iter()
        .map(|c| c.get(i).copied().filter(|x| x.is_finite()).unwrap_or(0.0))
        .sum::<f32>()
        / channels.len() as f32
}

/// The host's MIDI as the engine takes it (any channel), if it takes it.
fn event_of(e: NoteEvent<()>) -> Option<Event> {
    Some(match e {
        NoteEvent::NoteOn { note, .. } => Event::Note {
            key: note,
            on: true,
        },
        NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => Event::Note {
            key: note,
            on: false,
        },
        NoteEvent::MidiPitchBend { value, .. } => Event::PitchBend(value * 2.0 - 1.0),
        NoteEvent::MidiCC { cc, value, .. } => match cc {
            MODULATION_WHEEL => Event::Modulation(value),
            RESET_CONTROLLERS => Event::ResetControllers,
            ALL_SOUND_OFF => Event::AllSoundOff,
            ALL_NOTES_OFF => Event::AllNotesOff,
            _ => return None,
        },
        _ => return None,
    })
}

impl Ca72 {
    fn midi(&mut self, e: NoteEvent<()>) {
        if let Some(event) = event_of(e) {
            self.engine.event(event);
        }
    }

    /// What the engine asks done off the audio thread since the last block, asked of the
    /// helper (decisions.md R23): allocates nothing and does not wait.
    fn ask_helper(&mut self) {
        let mend = if self.engine.take_mending() { MEND } else { 0 };
        let serve = if self.engine.take_serving() { SERVE } else { 0 };
        if let Some(h) = &self.helper {
            h.ask(mend | serve);
        }
    }
}

/// EXTERNAL INPUT: a side chain of as many channels as the output (mixed to mono at the jack).
const SIDE_CHAIN_STEREO: &[std::num::NonZeroU32] = &[new_nonzero_u32(2)];
const SIDE_CHAIN_MONO: &[std::num::NonZeroU32] = &[new_nonzero_u32(1)];

const fn layout(channels: u32, name: &'static str) -> AudioIOLayout {
    AudioIOLayout {
        main_input_channels: None,
        main_output_channels: Some(new_nonzero_u32(channels)),
        aux_input_ports: if channels == 1 {
            SIDE_CHAIN_MONO
        } else {
            SIDE_CHAIN_STEREO
        },
        names: PortNames {
            layout: Some(name),
            main_output: Some("Output"),
            aux_inputs: &["External Input"],
            ..PortNames::const_default()
        },
        ..AudioIOLayout::const_default()
    }
}

impl Plugin for Ca72 {
    const NAME: &'static str = "CA-72";
    const VENDOR: &'static str = "Idle Foundry";
    const URL: &'static str = "https://github.com/idlefoundry/ca-72";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[layout(2, "Stereo"), layout(1, "Mono")];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    /// None: nih-plug's background thread, shared by every instance, holds a plug-in alive
    /// while it runs one of its tasks, past the host's destroy. The plug-in's own helper does
    /// that work instead (decisions.md R23).
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    /// A session saved before ENTROPY (decisions.md R14) holds ANALOG's `analog`, which no
    /// parameter takes: it opens with ENTROPY at its default, not at what the instance had
    /// (R18).
    fn filter_state(state: &mut PluginState) {
        state.params.remove("analog");
        if !state.params.contains_key("entropy") {
            let entropy = Ca72Params::default().entropy.default_plain_value();
            state
                .params
                .insert("entropy".to_owned(), ParamValue::F32(entropy));
        }
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(Ca72Editor::new(
            self.params.clone(),
            self.meters.clone(),
        )))
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set(&self.params.controls());
        self.engine
            .prepare(f64::from(config.sample_rate), self.params.seed());
        let block = config.max_buffer_size.max(1) as usize;
        let rate = f64::from(config.sample_rate);
        self.rate = rate;
        if self.helper.is_none() {
            self.helper = Helper::start(self.engine.spares(), self.engine.crew()).ok();
        }
        // POLY's workers, audio threads for the host's largest block (decisions.md R11), held
        // only while POLY is on: started here if it is, else on the helper thread when it is
        // switched on (R21, R23). A host loading a state into the running instance comes here
        // again with the audio thread waiting: running workers are kept then (R18).
        let period = std::time::Duration::from_secs_f64(block as f64 / rate);
        self.engine
            .start_workers(crate::engine::default_workers(), Some(period));
        true
    }

    /// POLY's workers stopped and given back to the process's budget (decisions.md R21): the
    /// host initialises the plug-in again before it next plays.
    fn deactivate(&mut self) {
        self.engine.stop_workers();
    }

    fn reset(&mut self) {
        self.engine.release_all();
        self.engine.event(Event::ResetControllers);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.engine.set(&self.params.controls());
        let len = buffer.samples();
        // The voices' deadline: a share of the block's period from now (decisions.md R11);
        // none rendering offline, where every voice is waited for (R18).
        let period = len as f64 / self.rate;
        let deadline = (context.process_mode() != ProcessMode::Offline).then(|| {
            std::time::Instant::now()
                + std::time::Duration::from_secs_f64(period * crate::engine::DEADLINE)
        });
        self.engine.set_deadline(deadline);
        let side = aux.inputs.first().map(|b| b.as_slice_immutable());
        let out = buffer.as_slice();
        let mut next = context.next_event();
        // The samples between one event and the next, a run at a time (the engine plays
        // POLY's voices one after another over a run: decisions.md R11), the side chain mixed
        // to mono.
        const RUN: usize = 128;
        let (mut ext, mut l, mut r) = ([0.0f32; RUN], [0.0f32; RUN], [0.0f32; RUN]);
        let mut i = 0;
        while i < len {
            while let Some(e) = next {
                if e.timing() as usize > i {
                    break;
                }
                self.midi(e);
                next = context.next_event();
            }
            let end = next
                .map_or(len, |e| e.timing() as usize)
                .min(len)
                .min(i + RUN);
            let n = end - i;
            let ext = match side {
                Some(s) if !s.is_empty() => {
                    for (k, x) in ext[..n].iter_mut().enumerate() {
                        *x = mono(s, i + k);
                    }
                    &ext[..n]
                }
                _ => &[][..],
            };
            // Left and right (SPREAD's voices placed); on a mono output, the two together.
            match &mut out[..] {
                [left, right, ..] => self
                    .engine
                    .render(ext, &mut left[i..end], &mut right[i..end]),
                [only] => {
                    self.engine.render(ext, &mut l[..n], &mut r[..n]);
                    for k in 0..n {
                        only[i + k] = 0.5 * (l[k] + r[k]);
                    }
                }
                [] => self.engine.render(ext, &mut l[..n], &mut r[..n]),
            }
            i = end;
        }
        while let Some(e) = next {
            self.midi(e);
            next = context.next_event();
        }
        let lamp = self.engine.end_block(len);
        self.meters.publish(lamp, self.engine.midi_wheels());
        self.ask_helper();
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Ca72 {
    const CLAP_ID: &'static str = "com.idlefoundry.ca-72";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A synthesizer modelled from its circuit, one voice or up to ten");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Mono,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Ca72 {
    const VST3_CLASS_ID: [u8; 16] = *b"IdleFoundryCA72!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

#[cfg(test)]
mod tests {
    use super::*;
    use ca72::modulation::PITCH_WHEEL_SEMITONES;

    #[test]
    fn the_hosts_midi_reaches_the_wheels_on_any_channel() {
        let mut p = Ca72::default();
        p.midi(NoteEvent::MidiPitchBend {
            timing: 0,
            channel: 3,
            value: 0.0,
        });
        p.midi(NoteEvent::MidiCC {
            timing: 0,
            channel: 9,
            cc: MODULATION_WHEEL,
            value: 1.0,
        });
        let (bend, modulation) = p.engine.wheels();
        assert!((bend + 2.0 / PITCH_WHEEL_SEMITONES).abs() < 1e-9, "{bend}");
        assert_eq!(modulation, 1.0);
        p.midi(NoteEvent::MidiCC {
            timing: 0,
            channel: 0,
            cc: RESET_CONTROLLERS,
            value: 0.0,
        });
        assert_eq!(p.engine.wheels(), (0.0, 0.0));
    }

    /// All Sound Off silences, All Notes Off releases (decisions.md R18).
    #[test]
    fn all_sound_off_and_all_notes_off_are_told_apart() {
        let cc = |cc| {
            event_of(NoteEvent::MidiCC {
                timing: 0,
                channel: 0,
                cc,
                value: 0.0,
            })
        };
        assert_eq!(cc(ALL_SOUND_OFF), Some(Event::AllSoundOff));
        assert_eq!(cc(ALL_NOTES_OFF), Some(Event::AllNotesOff));
        assert_eq!(cc(7), None);
    }

    /// The side chain's channels mixed, a channel shorter than the block (one the host did
    /// not supply may be) and a sample that is not finite taken as 0 (R18).
    #[test]
    fn the_side_chain_is_read_within_its_channels_and_finite() {
        let (mut a, mut b) = ([1.0, f32::NAN, 2.0, f32::INFINITY], [3.0f32]);
        let side: [&mut [f32]; 2] = [&mut a, &mut b];
        let mixed: Vec<f32> = (0..4).map(|i| mono(&side, i)).collect();
        assert_eq!(mixed, [2.0, 0.0, 1.0, 0.0]);
    }

    /// A session saved before ENTROPY, with ANALOG's amount, opens with ENTROPY at its
    /// default, not at what the instance had (R14, R18); one saved since keeps its own.
    #[test]
    fn an_old_session_opens_with_entropy_at_its_default() {
        let state = |params: &[(&str, f32)]| PluginState {
            version: String::new(),
            params: params
                .iter()
                .map(|&(k, v)| (k.to_owned(), ParamValue::F32(v)))
                .collect(),
            fields: Default::default(),
        };
        let entropy = |s: &PluginState| match s.params.get("entropy") {
            Some(ParamValue::F32(v)) => Some(*v),
            _ => None,
        };
        let mut old = state(&[("analog", 70.0), ("cutoff", 3.0)]);
        Ca72::filter_state(&mut old);
        assert_eq!(entropy(&old), Some(0.0));
        assert!(!old.params.contains_key("analog"));
        let mut new = state(&[("entropy", 40.0)]);
        Ca72::filter_state(&mut new);
        assert_eq!(entropy(&new), Some(40.0));
    }

    /// The plug-in tests that use the process's budget of workers, one at a time.
    static BUDGET_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Init;
    impl InitContext<Ca72> for Init {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn execute(&self, _: ()) {}
        fn set_latency_samples(&self, _: u32) {}
        fn set_current_voice_capacity(&self, _: u32) {}
    }

    /// A plug-in, POLY as `poly`, initialised as a host does at 48 kHz in blocks of 256.
    fn initialised(poly: bool) -> Ca72 {
        let config = BufferConfig {
            sample_rate: 48_000.0,
            min_buffer_size: None,
            max_buffer_size: 256,
            process_mode: ProcessMode::Realtime,
        };
        let mut p = Ca72::with_params(Ca72Params {
            poly: BoolParam::new("Poly", poly),
            ..Ca72Params::default()
        });
        assert!(p.initialize(&Ca72::AUDIO_IO_LAYOUTS[0], &config, &mut Init));
        p
    }

    /// Waits up to 10 s for `f` (bounded by what happens, not by time: it fails only if the
    /// helper never gets there).
    fn until(what: &str, mut f: impl FnMut() -> bool) {
        let t = std::time::Instant::now();
        while !f() {
            assert!(t.elapsed() < std::time::Duration::from_secs(10), "{what}");
            std::thread::sleep(std::time::Duration::from_micros(500));
        }
    }

    /// The plug-in's workers through its life, out of the process's budget (decisions.md
    /// R21): initialised with POLY off, it holds none; POLY switched on, its helper (R23),
    /// asked as at the end of a block, starts this machine's share, taken at a block's start;
    /// deactivated, they are back in the budget; initialised with POLY on, it holds them at
    /// once, and gives them back when dropped.
    #[test]
    fn the_workers_follow_poly_through_the_plug_ins_life() {
        let _one = BUDGET_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let held = || crate::pool::BUDGET.held();
        // (A machine of fewer than 5 processors has none to give: R11. CI's have 3.)
        let most = crate::engine::default_workers();
        let mut p = initialised(false);
        assert_eq!((p.engine.workers(), held()), (0, 0));
        // (POLY as `process` reads it at a block's start.)
        let c = crate::engine::Controls {
            poly: true,
            ..p.params.controls()
        };
        p.engine.set(&c);
        p.engine.set_deadline(None);
        p.ask_helper();
        until("no workers from the helper", || {
            p.engine.set_deadline(None);
            p.engine.workers() == most
        });
        assert_eq!(held(), most);
        p.deactivate();
        assert_eq!((p.engine.workers(), held()), (0, 0));
        let q = initialised(true);
        assert_eq!((q.engine.workers(), held()), (most, most));
        drop(q);
        assert_eq!(held(), 0);
    }

    /// The crash R23 fixes: a plug-in dropped (the host's destroy) while its helper is in a
    /// round of mending and serving waits for it, and once the drop returns no thread of the
    /// plug-in's runs: the helper has ended and the voices' workers have stopped, their share
    /// back in the budget, so the host may unload the plug-in.
    #[test]
    fn a_plug_in_dropped_while_its_helper_works_leaves_no_thread_running() {
        let _one = BUDGET_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let held = || crate::pool::BUDGET.held();
        let p = initialised(true);
        assert_eq!(held(), p.engine.workers());
        let watch = p.helper.as_ref().map(Helper::watch).unwrap();
        watch.hold(true);
        if let Some(h) = &p.helper {
            h.ask(MEND | SERVE);
        }
        until("the helper's round never began", || watch.held());
        let (tx, rx) = std::sync::mpsc::channel();
        let w = watch.clone();
        let dropper = std::thread::spawn(move || {
            drop(p);
            tx.send((w.ended(), held())).unwrap();
        });
        // (The drop must not return while the round runs: given a moment to, it has not.)
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(200))
                .is_err(),
            "the drop returned while the helper's round ran"
        );
        watch.hold(false);
        let (ended, workers) = rx.recv().unwrap();
        assert!(ended, "the drop returned before the helper ended");
        assert_eq!(workers, 0, "the drop returned before the workers stopped");
        dropper.join().unwrap();
    }
}

#[allow(unsafe_code)]
mod export {
    use super::Ca72;
    use nih_plug::prelude::*;

    nih_export_clap!(Ca72);
    nih_export_vst3!(Ca72);
}
