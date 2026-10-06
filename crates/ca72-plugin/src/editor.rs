//! The editor: the front panel (`ca72-panel`) in a window of its own (baseview), its frames
//! shown with softbuffer, and under it the plug-in's own controls (`ca72_panel::strip`: POLY,
//! VOICES with what plays in real time here, ENTROPY and SPREAD). Every control operates its
//! parameter through the host, a drag one gesture (one undo step in hosts that keep them);
//! the POWER switch is the bypass; the OVERLOAD lamp and the wheels show what the audio
//! thread reports. The grip at the panel's bottom right corner resizes the window. Under the strip the presets' bar, and their drawer over
//! the panel (`crate::presets`, decisions.md R10), which takes the keyboard while open; in the
//! drawer the update check (`crate::update`, R27).

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use baseview::{
    Event, EventStatus, MouseButton, MouseEvent, ScrollDelta, Size, Window, WindowEvent,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};
use ca72_panel::art::{self, POWER};
use ca72_panel::controls::{FEEDBACK_SILENT, feedback_silent};
use ca72_panel::presets::{
    BAR_END, DRAWER_H, DRAWER_TOP, DrawerRenderer, DrawerTarget, ROW_H, bar_hit, drawer_hit,
    overlay,
};
use ca72_panel::strip::{self, Amount, STRIP_H, StripRenderer, StripScene, StripTarget};
use ca72_panel::{CONTROLS, Kind, Renderer, Scene, Target, interact};
use keyboard_types::{Key, KeyState, KeyboardEvent, Modifiers};
use nih_plug::prelude::*;

use crate::library::Library;
use crate::params::Ca72Params;
use crate::presets::Browser;
use crate::update::Update;

/// The share of the usable screen the editor opens at until it is resized.
const SCREEN_SHARE: f64 = 0.8;
/// The editor's width at first where the screen's size is unknown, in logical pixels.
pub const DEFAULT_WIDTH: u32 = 1380;
/// How small and large the grip makes it.
const MIN_WIDTH: u32 = 860;
const MAX_WIDTH: u32 = 3438;

/// Two presses within this long are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// A mouse wheel's turns are one gesture until it rests this long.
const SCROLL_REST: Duration = Duration::from_millis(400);
/// A trackpad's travel that makes a notch, in pixels.
const NOTCH: f64 = 40.0;

/// The editor's height at `width`: the panel in its proportions, and the strip (with the
/// presets' selector) under it.
pub fn height_for(width: u32) -> u32 {
    (f64::from(width) * (art::H + STRIP_H) / art::W).round() as u32
}

/// What the presets' drawer adds to the window's height at `width` while it is open below
/// the bar.
pub fn drawer_height(width: u32) -> u32 {
    (f64::from(width) * DRAWER_H / art::W).round() as u32
}

fn clamp_width(width: u32) -> u32 {
    width.clamp(MIN_WIDTH, MAX_WIDTH)
}

/// The width the editor opens at before it is resized: its share of the usable screen
/// (logical pixels), its height kept within the same share.
fn fitted_width(screen: Option<(f64, f64)>) -> u32 {
    screen.map_or(DEFAULT_WIDTH, |(w, h)| {
        let w = (w * SCREEN_SHARE).min(h * SCREEN_SHARE * art::W / (art::H + STRIP_H));
        clamp_width(w.round() as u32)
    })
}

/// What the audio thread reports to the editor after each block.
#[derive(Debug)]
pub struct Meters {
    overload: AtomicU32,
    bend: AtomicU32,
    modulation: AtomicU32,
}

impl Default for Meters {
    fn default() -> Self {
        Meters {
            overload: AtomicU32::new(0.0f32.to_bits()),
            bend: AtomicU32::new(0.5f32.to_bits()),
            modulation: AtomicU32::new(0.0f32.to_bits()),
        }
    }
}

impl Meters {
    /// The OVERLOAD lamp (0..1) and a MIDI keyboard's wheels as the panel draws them.
    pub fn publish(&self, overload: f32, (bend, modulation): (f32, f32)) {
        self.overload.store(overload.to_bits(), Ordering::Relaxed);
        self.bend.store(bend.to_bits(), Ordering::Relaxed);
        self.modulation
            .store(modulation.to_bits(), Ordering::Relaxed);
    }

    fn load(a: &AtomicU32) -> f64 {
        let v = f64::from(f32::from_bits(a.load(Ordering::Relaxed)));
        if v.is_finite() { v } else { 0.0 }
    }
}

/// A parameter as the editor operates it, whatever its type.
trait Operated {
    fn value(&self) -> f32;
    fn default(&self) -> f32;
    fn text(&self, normalized: f32) -> String;
    fn begin(&self, s: &ParamSetter<'_>);
    fn set(&self, s: &ParamSetter<'_>, normalized: f32);
    fn end(&self, s: &ParamSetter<'_>);
}

impl<P: Param> Operated for P {
    fn value(&self) -> f32 {
        self.unmodulated_normalized_value()
    }
    fn default(&self) -> f32 {
        self.default_normalized_value()
    }
    fn text(&self, normalized: f32) -> String {
        self.normalized_value_to_string(normalized, true)
    }
    fn begin(&self, s: &ParamSetter<'_>) {
        s.begin_set_parameter(self);
    }
    fn set(&self, s: &ParamSetter<'_>, normalized: f32) {
        s.set_parameter_normalized(self, normalized);
    }
    fn end(&self, s: &ParamSetter<'_>) {
        s.end_set_parameter(self);
    }
}

/// The parameter a control of the panel operates, by its id.
fn param<'a>(p: &'a Ca72Params, id: &str) -> Option<&'a dyn Operated> {
    Some(match id {
        "tune" => &p.tune,
        "glide" => &p.glide,
        "mod_mix" => &p.mod_mix,
        "osc_mod" => &p.osc_mod,
        "osc3_control" => &p.osc3_control,
        "osc1_range" => &p.osc1_range,
        "osc1_waveform" => &p.osc1_waveform,
        "osc2_range" => &p.osc2_range,
        "osc2_frequency" => &p.osc2_frequency,
        "osc2_waveform" => &p.osc2_waveform,
        "osc3_range" => &p.osc3_range,
        "osc3_frequency" => &p.osc3_frequency,
        "osc3_waveform" => &p.osc3_waveform,
        "osc1_on" => &p.osc1_on,
        "osc1_volume" => &p.osc1_volume,
        "ext_on" => &p.ext_on,
        "ext_volume" => &p.ext_volume,
        "osc2_on" => &p.osc2_on,
        "osc2_volume" => &p.osc2_volume,
        "noise_on" => &p.noise_on,
        "noise_volume" => &p.noise_volume,
        "noise_type" => &p.noise_type,
        "osc3_on" => &p.osc3_on,
        "osc3_volume" => &p.osc3_volume,
        "filter_mod" => &p.filter_mod,
        "keyboard_control_1" => &p.keyboard_control_1,
        "keyboard_control_2" => &p.keyboard_control_2,
        "cutoff" => &p.cutoff,
        "emphasis" => &p.emphasis,
        "contour_amount" => &p.contour_amount,
        "filter_attack" => &p.filter_attack,
        "filter_decay" => &p.filter_decay,
        "filter_sustain" => &p.filter_sustain,
        "loudness_attack" => &p.loudness_attack,
        "loudness_decay" => &p.loudness_decay,
        "loudness_sustain" => &p.loudness_sustain,
        "volume" => &p.volume,
        "feedback" => &p.feedback,
        "main_output" => &p.main_output,
        "a440" => &p.a440,
        "glide_on" => &p.glide_on,
        "decay_on" => &p.decay_on,
        "pitch_wheel" => &p.pitch_wheel,
        "mod_wheel" => &p.mod_wheel,
        _ => return None,
    })
}

/// The editor, as the plug-in hands it to the host.
pub struct Ca72Editor {
    params: Arc<Ca72Params>,
    meters: Arc<Meters>,
    /// The host's scale (f32 bits; 0 none given: the system's, as on macOS).
    scale: AtomicU32,
    open: Arc<AtomicBool>,
    /// The window grown for the presets' drawer below the bar.
    grown: Arc<AtomicBool>,
    /// The width fitted to the screen for this opening (0 not yet measured).
    fitted: Arc<AtomicU32>,
}

impl std::fmt::Debug for Ca72Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ca72Editor")
            .field("size", &self.size())
            .finish()
    }
}

impl Ca72Editor {
    pub fn new(params: Arc<Ca72Params>, meters: Arc<Meters>) -> Self {
        Ca72Editor {
            params,
            meters,
            scale: AtomicU32::new(0),
            open: Arc::new(AtomicBool::new(false)),
            grown: Arc::new(AtomicBool::new(false)),
            fitted: Arc::new(AtomicU32::new(0)),
        }
    }

    fn host_scale(&self) -> Option<f32> {
        let s = f32::from_bits(self.scale.load(Ordering::Relaxed));
        (s > 0.0 && s.is_finite()).then_some(s)
    }

    /// The editor opening: the drawer shut (the window does not open grown), the screen
    /// measured again, its size.
    fn opening(&self) -> (u32, u32) {
        self.grown.store(false, Ordering::Relaxed);
        self.fitted.store(0, Ordering::Relaxed);
        self.size()
    }

    /// The width fitted to the screen, measured once an opening and kept until the editor
    /// closes. Measured afresh at each of the host's questions, it changed with the drawer
    /// (on Linux it is the monitor under the pointer), and the panel was drawn again at
    /// another scale as the drawer opened and shut (the owner, 2026-10-03; decisions.md R19).
    fn fitted(&self) -> u32 {
        match self.fitted.load(Ordering::Relaxed) {
            0 => {
                let w = fitted_width(screen::usable(self.host_scale().map(f64::from)));
                self.fitted.store(w, Ordering::Relaxed);
                w
            }
            w => w,
        }
    }

    /// What resets the editor as its window closes (the host may ask its size before it
    /// opens it again: the drawer is shut then).
    fn closing(&self) -> Closing {
        Closing {
            open: Arc::clone(&self.open),
            grown: Arc::clone(&self.grown),
            fitted: Arc::clone(&self.fitted),
        }
    }
}

/// The editor's window closed: not open, the drawer shut, the screen to be measured again.
struct Closing {
    open: Arc<AtomicBool>,
    grown: Arc<AtomicBool>,
    fitted: Arc<AtomicU32>,
}

impl Drop for Closing {
    fn drop(&mut self) {
        self.open.store(false, Ordering::Release);
        self.grown.store(false, Ordering::Relaxed);
        self.fitted.store(0, Ordering::Relaxed);
    }
}

/// How baseview scales the window: on Windows and Linux by the host's scale, else by none.
/// nih-plug gives the host the editor's size times the host's scale (1 when it set none:
/// Ableton Live sets none), and the window must fill that frame; baseview's own scale there
/// (the monitor's, Xft.dpi) made the window draw at one scale, size itself at another and
/// place the pointer at a third (decisions.md R18). macOS scales by itself.
fn scale_policy(host: Option<f32>) -> WindowScalePolicy {
    if cfg!(target_os = "macos") {
        WindowScalePolicy::SystemScaleFactor
    } else {
        WindowScalePolicy::ScaleFactor(host.map_or(1.0, f64::from))
    }
}

impl Editor for Ca72Editor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        let (width, height) = self.opening();
        let scale = self.host_scale();
        let params = Arc::clone(&self.params);
        let meters = Arc::clone(&self.meters);
        let grown = Arc::clone(&self.grown);
        let parent = window::Parent(parent);
        // No window to open in (no parent, no X11 display): nothing shown, rather than
        // baseview's panic in the host's thread (decisions.md R18).
        if !parent.can_open() {
            return Box::new(window::Handle::new(None, self.closing()));
        }
        let window = Window::open_parented(
            &parent,
            WindowOpenOptions {
                title: String::from("CA-72"),
                size: Size::new(f64::from(width), f64::from(height)),
                scale: scale_policy(scale),
            },
            move |window: &mut Window<'_>| {
                PanelWindow::new(
                    window,
                    context,
                    params,
                    meters,
                    grown,
                    width,
                    f64::from(scale.unwrap_or(1.0)),
                    Library::shared(),
                )
            },
        );
        self.open.store(true, Ordering::Release);
        Box::new(window::Handle::new(Some(window), self.closing()))
    }

    fn size(&self) -> (u32, u32) {
        let width = match self.params.editor_width.load(Ordering::Relaxed) {
            0 => self.fitted(),
            w => clamp_width(w),
        };
        let drawer = if self.grown.load(Ordering::Relaxed) {
            drawer_height(width)
        } else {
            0
        };
        (width, height_for(width) + drawer)
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        // The window cannot follow a new scale once open (Ableton Live asks).
        if self.open.load(Ordering::Acquire) {
            return false;
        }
        self.scale.store(factor.to_bits(), Ordering::Relaxed);
        true
    }

    // The window reads the parameters every frame.
    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
    fn param_values_changed(&self) {}
}

/// A control being dragged.
#[derive(Debug)]
struct Drag {
    control: usize,
    /// Where the pointer pressed (logical pixels), and what it pressed.
    from: (f64, f64),
    pressed: Target,
    /// Its value at the press; the drag counts from `base` at `at` (Shift changes the rate
    /// mid-drag: the travel then counts from where it changed).
    value: f64,
    base: f64,
    at: (f64, f64),
    fine: bool,
    moved: bool,
}

/// A mouse wheel turning a control.
#[derive(Debug)]
struct Scroll {
    control: usize,
    value: f64,
    last: Instant,
    travel: f64,
}

/// The editor's window: its surface, and the panel being operated in it.
struct PanelWindow {
    surface: window::Surface,
    physical: (u32, u32),
    /// Its size in logical pixels, as last set or reported.
    logical: (u32, u32),
    /// When the frame was last shown on the window (none since it opened or resized).
    shown: Option<Instant>,
    editing: Editing,
    /// The keyboard the drawer took from another window (Windows), given back as it shuts.
    keys: Option<window::Keys>,
}

/// The panel being operated: what the pointer does to the parameters, and the scene drawn.
struct Editing {
    context: Arc<dyn GuiContext>,
    params: Arc<Ca72Params>,
    meters: Arc<Meters>,
    /// Physical pixels a logical pixel, and the window's width in logical pixels.
    dpr: f64,
    width: u32,
    renderer: Renderer,
    scene: Scene,
    /// The strip under the panel: its renderer and scene, an amount's slider being dragged,
    /// and each amount's last value but 0 (its switch turns it back on there).
    strip: StripRenderer,
    strip_scene: StripScene,
    sliding: Option<Amount>,
    last: [f32; 2],
    /// The presets: the bar's and the drawer's renderers, what they show and do, and the
    /// panel's frame with the drawer over it (while it shows).
    drawer: DrawerRenderer,
    pub(crate) browser: Browser,
    composed: Option<ca72_panel::Pixmap>,
    /// The window grown for the drawer below the bar (the editor's `size` reads it), the
    /// drawer's frame while it slides down there, and a size asked of the window.
    grown: Arc<AtomicBool>,
    slid: Option<ca72_panel::Pixmap>,
    resizing_window: bool,
    /// The drawer placed for its opening (below the strip or over the panel), asked once.
    placed: bool,
    /// The update check in the drawer; dropped with the editor, it ends a check under way.
    update: Update,
    /// The pointer, in logical pixels.
    pointer: (f64, f64),
    hover: Option<Target>,
    drag: Option<Drag>,
    scroll: Option<Scroll>,
    /// The presets' list's scrolling not yet a whole row, in rows (+ up).
    list_travel: f64,
    last_press: Option<(Instant, Target)>,
    /// The grip dragged: the pointer's x and the width at the press; the width it last took
    /// the window to, asked of the host at the next frame (once a frame, however often the
    /// pointer moves).
    resizing: Option<(f64, u32)>,
    gripped: Option<u32>,
    /// The usable screen (logical pixels) at the window's scale: the drawer opens below the
    /// strip only if the window then fits on it.
    screen: fn(f64) -> Option<(f64, f64)>,
}

impl PanelWindow {
    /// The editor's window: `width` logical pixels wide, `dpr` physical pixels a logical one,
    /// the presets from `library` (the user's, but in the tests).
    #[allow(clippy::too_many_arguments)]
    fn new(
        window: &mut Window<'_>,
        context: Arc<dyn GuiContext>,
        params: Arc<Ca72Params>,
        meters: Arc<Meters>,
        grown: Arc<AtomicBool>,
        width: u32,
        dpr: f64,
        library: Library,
    ) -> Self {
        let physical = (
            (f64::from(width) * dpr).round() as u32,
            (f64::from(height_for(width)) * dpr).round() as u32,
        );
        let surface = window::Surface::new(window, physical);
        PanelWindow {
            surface,
            physical,
            logical: (width, height_for(width)),
            shown: None,
            editing: Editing::new(
                context,
                params,
                meters,
                (width, physical.0, dpr),
                library,
                grown,
                |k| screen::usable(Some(k)),
            ),
            keys: None,
        }
    }

    /// The window now `info`'s size: the surface and every frame drawn to it.
    fn resized(&mut self, info: &WindowInfo) {
        let size = info.physical_size();
        let physical = (size.width.max(1), size.height.max(1));
        let logical = info.logical_size();
        self.logical = (logical.width.round() as u32, logical.height.round() as u32);
        if physical == self.physical && info.scale() == self.editing.dpr && self.shown.is_some() {
            return;
        }
        self.physical = physical;
        self.editing.fit(
            info.scale(),
            info.logical_size().width.round() as u32,
            physical.0,
        );
        self.surface.resize(self.physical);
        self.shown = None;
    }

    /// The window resized to what the editor asks (the grip, the drawer below the bar),
    /// when it asked: on macOS baseview reports no resize of its own making, so the frames
    /// follow here; elsewhere the report that follows finds the size already taken.
    fn follow(&mut self, window: &mut Window<'_>) {
        if !std::mem::take(&mut self.editing.resizing_window) {
            return;
        }
        let want = (self.editing.width, self.editing.window_height());
        if want != self.logical {
            let size = Size::new(f64::from(want.0), f64::from(want.1));
            window.resize(size);
            self.resized(&WindowInfo::from_logical_size(size, self.editing.dpr));
        }
    }

    /// The keyboard as the drawer wants it: taken when a field takes the caret (baseview's
    /// focus; not on X11, where baseview has none and the keys go to the window under the
    /// pointer), and on Windows given back to the window that had it as the drawer shuts
    /// (baseview there keeps every key its window is given, the host's shortcuts too:
    /// decisions.md R18).
    fn keyboard(&mut self, window: &mut Window<'_>) {
        if std::mem::take(&mut self.editing.browser.wants_keys) {
            let had = window::take_keys(window);
            if self.keys.is_none() {
                self.keys = had;
            }
        }
        if !self.editing.browser.open
            && let Some(keys) = self.keys.take()
        {
            window::give_keys_back(window, keys);
        }
    }

    fn present(&mut self) {
        let e = &self.editing;
        let panel = e.composed.as_ref().unwrap_or_else(|| e.renderer.frame());
        let mut frames = vec![panel, e.strip.frame()];
        if e.grown.load(Ordering::Relaxed) && (e.browser.open || e.browser.sliding()) {
            frames.push(e.slid.as_ref().unwrap_or_else(|| e.drawer.frame()));
        }
        self.surface.present(self.physical, &frames);
    }
}

impl Editing {
    /// An editor `width` logical pixels wide, `physical_width` physical ones, `dpr`
    /// physical pixels a logical one; `screen` the usable screen at a scale.
    fn new(
        context: Arc<dyn GuiContext>,
        params: Arc<Ca72Params>,
        meters: Arc<Meters>,
        (width, physical_width, dpr): (u32, u32, f64),
        library: Library,
        grown: Arc<AtomicBool>,
        screen: fn(f64) -> Option<(f64, f64)>,
    ) -> Self {
        Editing {
            context,
            params,
            meters,
            dpr,
            width,
            renderer: Renderer::new(f64::from(physical_width) / art::W, dpr),
            scene: Scene::default(),
            strip: StripRenderer::new(f64::from(physical_width) / art::W),
            strip_scene: StripScene::default(),
            sliding: None,
            last: [0.5; 2],
            drawer: DrawerRenderer::new(f64::from(physical_width) / art::W),
            browser: Browser::new(library),
            composed: None,
            grown,
            slid: None,
            resizing_window: false,
            placed: false,
            update: Update::default(),
            pointer: (0.0, 0.0),
            hover: None,
            drag: None,
            scroll: None,
            list_travel: 0.0,
            last_press: None,
            resizing: None,
            gripped: None,
            screen,
        }
    }

    /// The renderers at the window's scale: `dpr` physical pixels a logical one, `width`
    /// logical pixels wide, `physical_width` physical ones.
    fn fit(&mut self, dpr: f64, width: u32, physical_width: u32) {
        self.dpr = dpr;
        self.width = width;
        let scale = f64::from(physical_width) / art::W;
        self.renderer.rescale(scale, dpr);
        self.strip.rescale(scale);
        self.drawer.rescale(scale);
        self.composed = None;
    }

    fn setter(&self) -> ParamSetter<'_> {
        ParamSetter::new(self.context.as_ref())
    }

    /// The pointer's place in the drawing.
    fn in_drawing(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let k = self.dpr / self.renderer.scale();
        (x * k, y * k)
    }

    /// Whether the drawer is below the bar (the window grown), else over the panel.
    fn below(&self) -> bool {
        self.grown.load(Ordering::Relaxed)
    }

    /// The drawer's top now (drawing units) and where it is cut off, while it shows: below
    /// the strip sliding down from under it, else over the panel sliding up from its foot.
    fn drawer_span(&self) -> Option<(f64, f64, f64)> {
        let k = self.browser.reveal();
        if k <= 0.0 {
            return None;
        }
        let foot = art::H + STRIP_H;
        Some(if self.below() {
            (foot - (1.0 - k) * DRAWER_H, foot, foot + DRAWER_H)
        } else {
            let top = DRAWER_TOP + (1.0 - k) * DRAWER_H;
            (top, top, art::H)
        })
    }

    /// The pointer's place in the drawer (drawer units), if it is over it.
    fn in_drawer(&self, (x, y): (f64, f64)) -> Option<(f64, f64)> {
        let (top, from, to) = self.drawer_span()?;
        (y >= from && y < to).then_some((x, y - top))
    }

    /// The window's height (logical pixels): the panel, the strip and the bar, and the
    /// drawer below them while it is open there.
    fn window_height(&self) -> u32 {
        height_for(self.width)
            + if self.below() {
                drawer_height(self.width)
            } else {
                0
            }
    }

    /// Whether the window with the drawer below the strip fits on the usable screen (if the
    /// screen is known): on a short, wide one it would run off its foot.
    fn drawer_fits(&self) -> bool {
        let tall = height_for(self.width) + drawer_height(self.width);
        (self.screen)(self.dpr).is_none_or(|(_, h)| f64::from(tall) <= h)
    }

    /// The window grown for the drawer as it opens, and back as it shuts (if the host will
    /// not, or the window would then not fit on the screen, it opens over the panel:
    /// decisions.md R18).
    fn follow_drawer(&mut self) {
        let open = self.browser.open;
        if open == self.placed {
            return;
        }
        self.placed = open;
        if open {
            self.grown.store(true, Ordering::Relaxed);
            if self.drawer_fits() && self.context.request_resize() {
                self.browser.below = true;
            } else {
                self.grown.store(false, Ordering::Relaxed);
                self.browser.below = false;
            }
        } else if self.below() {
            // Shut below the strip: it slides back up under it as it came out, the window
            // keeping its height until it has (`shrink_shut_drawer`). It had jumped over the
            // panel and slid down off it (the owner, 2026-10-03; decisions.md R19).
            return;
        }
        self.composed = None;
        self.slid = None;
        self.resizing_window = true;
    }

    /// The window shrunk back once a drawer shut below the strip has slid up under it.
    fn shrink_shut_drawer(&mut self) {
        if !self.browser.open && self.below() && !self.browser.sliding() {
            self.grown.store(false, Ordering::Relaxed);
            self.context.request_resize();
            self.slid = None;
            self.resizing_window = true;
        }
    }

    /// The pointer's place on the presets' bar (bar units), if it is over it.
    /// The pointer's place on the presets' selector (the strip's left, its units), if there.
    fn on_bar(&self, (x, y): (f64, f64)) -> Option<(f64, f64)> {
        ((art::H..art::H + STRIP_H).contains(&y) && x < BAR_END).then_some((x, y - art::H))
    }

    fn operated(&self, control: usize) -> Option<&dyn Operated> {
        param(&self.params, CONTROLS.get(control)?.param)
    }

    fn value_of(&self, control: usize) -> f64 {
        self.operated(control).map_or(0.0, |p| f64::from(p.value()))
    }

    /// One change of a control, a gesture of its own.
    fn set_once(&self, control: usize, v: f64) {
        if let Some(p) = self.operated(control) {
            let s = self.setter();
            p.begin(&s);
            p.set(&s, v as f32);
            p.end(&s);
        }
    }

    fn set_power(&self, on: bool) {
        let s = self.setter();
        s.begin_set_parameter(&self.params.bypass);
        s.set_parameter(&self.params.bypass, !on);
        s.end_set_parameter(&self.params.bypass);
    }

    /// An amount's parameter (ENTROPY, SPREAD).
    fn amount(&self, a: Amount) -> &FloatParam {
        match a {
            Amount::Entropy => &self.params.entropy,
            Amount::Spread => &self.params.spread,
        }
    }

    /// VOICES stepped by `by`, a gesture of its own.
    fn step_voices(&self, by: i32) {
        let s = self.setter();
        let p = &self.params.voices;
        s.begin_set_parameter(p);
        s.set_parameter(p, p.value() + by);
        s.end_set_parameter(p);
    }

    /// A press on the strip under the panel, at (`x`, `y`) in its units.
    fn strip_press(&mut self, x: f64, y: f64) {
        let Some(target) = strip::hit(x, y) else {
            return;
        };
        if let StripTarget::Switch(a) = target {
            // Off: 0, the amount kept; on: back to it (half way if it never was).
            let now = self.amount(a).unmodulated_normalized_value();
            let to = if now > 0.0 {
                self.last[a as usize] = now;
                0.0
            } else {
                self.last[a as usize]
            };
            let (s, p) = (self.setter(), self.amount(a));
            s.begin_set_parameter(p);
            s.set_parameter_normalized(p, to);
            s.end_set_parameter(p);
            return;
        }
        let s = self.setter();
        match target {
            StripTarget::Poly => {
                let p = &self.params.poly;
                s.begin_set_parameter(p);
                s.set_parameter(p, !p.value());
                s.end_set_parameter(p);
            }
            StripTarget::Fewer => self.step_voices(-1),
            StripTarget::More => self.step_voices(1),
            StripTarget::Slider(a) => {
                let p = self.amount(a);
                s.begin_set_parameter(p);
                s.set_parameter_normalized(p, strip::slider_value(a, x) as f32);
                self.sliding = Some(a);
            }
            StripTarget::Switch(_) => {}
        }
    }

    fn press(&mut self, modifiers: Modifiers) {
        // A wheel's gesture still waiting for its rest, or a drag whose release never came
        // (its window lost the pointer): ended before anything else begins.
        self.end_gestures();
        let (x, y) = self.in_drawing(self.pointer);
        if let Some((dx, dy)) = self.in_drawer((x, y)) {
            match drawer_hit(self.drawer.fonts(), &self.browser.drawer, dx, dy) {
                Some(DrawerTarget::Update) => self.update.press(),
                Some(t) => {
                    let setter = ParamSetter::new(self.context.as_ref());
                    self.browser
                        .drawer_press(t, dx, &self.drawer, &self.params, &setter);
                }
                None => {}
            }
            self.follow_drawer();
            return;
        }
        if let Some((bx, by)) = self.on_bar((x, y)) {
            if let Some(t) = bar_hit(bx, by) {
                let setter = ParamSetter::new(self.context.as_ref());
                self.browser.bar_press(t, &self.params, &setter);
            }
            self.follow_drawer();
            return;
        }
        if y >= art::H {
            self.strip_press(x, y - art::H);
            return;
        }
        let Some(target) = interact::hit(self.renderer.layout(), x, y) else {
            return;
        };
        let now = Instant::now();
        let double = self
            .last_press
            .is_some_and(|(t, last)| last == target && now - t < DOUBLE_CLICK);
        self.last_press = Some((now, target));
        match target {
            Target::Control(i) | Target::Legend(i, _) => {
                let kind = CONTROLS[i].kind;
                if double && !matches!(kind, Kind::Rocker { .. }) {
                    if let Some(p) = self.operated(i) {
                        self.set_once(i, f64::from(p.default()));
                    }
                    self.last_press = None;
                    return;
                }
                let value = self.value_of(i);
                if !matches!(kind, Kind::Rocker { .. })
                    && let Some(p) = self.operated(i)
                {
                    p.begin(&self.setter());
                }
                self.drag = Some(Drag {
                    control: i,
                    from: self.pointer,
                    pressed: target,
                    value,
                    base: value,
                    at: self.pointer,
                    fine: modifiers.contains(Modifiers::SHIFT),
                    moved: false,
                });
            }
            Target::Power => self.set_power(self.params.bypass.value()),
            Target::Grip => {
                self.resizing = Some((self.pointer.0, self.width));
            }
        }
    }

    /// The pointer moved.
    fn moved(&mut self, modifiers: Modifiers) {
        if let Some((x0, w0)) = self.resizing {
            // Asked of the host at the next frame: a resize renders the whole panel again.
            let width = clamp_width((f64::from(w0) + self.pointer.0 - x0).round().max(0.0) as u32);
            self.gripped = Some(width);
            return;
        }
        let fine = modifiers.contains(Modifiers::SHIFT);
        let pointer = self.pointer;
        if let Some(a) = self.sliding {
            let (x, _) = self.in_drawing(pointer);
            let p = self.amount(a);
            self.setter()
                .set_parameter_normalized(p, strip::slider_value(a, x) as f32);
            return;
        }
        let Some(d) = &mut self.drag else {
            let at = self.in_drawing(pointer);
            let (x, y) = at;
            let over_drawer = self.in_drawer(at);
            self.browser.drawer.hover = over_drawer
                .and_then(|(dx, dy)| drawer_hit(self.drawer.fonts(), &self.browser.drawer, dx, dy));
            self.browser.bar.hover = self.on_bar(at).and_then(|(bx, by)| bar_hit(bx, by));
            (self.hover, self.strip_scene.hover) =
                if over_drawer.is_some() || self.on_bar(at).is_some() {
                    (None, None)
                } else if y >= art::H {
                    (None, strip::hit(x, y - art::H))
                } else {
                    (interact::hit(self.renderer.layout(), x, y), None)
                };
            return;
        };
        if (pointer.0 - d.from.0).abs() + (pointer.1 - d.from.1).abs() > 3.0 {
            d.moved = true;
        }
        if !d.moved {
            return;
        }
        if fine != d.fine {
            d.base = d.value;
            d.at = pointer;
            d.fine = fine;
        }
        let kind = CONTROLS[d.control].kind;
        if let Some(v) = interact::drag(
            &kind,
            d.base,
            pointer.0 - d.at.0,
            pointer.1 - d.at.1,
            d.fine,
        ) && v != d.value
        {
            d.value = v;
            let (i, v) = (d.control, v);
            if let Some(p) = self.operated(i) {
                p.set(&self.setter(), v as f32);
            }
        }
    }

    /// Whether a press is being held: a control, a slider or the grip dragged.
    fn dragging(&self) -> bool {
        self.drag.is_some() || self.sliding.is_some() || self.resizing.is_some()
    }

    fn released(&mut self) {
        if self.resizing.take().is_some() {
            return;
        }
        if let Some(a) = self.sliding.take() {
            self.setter().end_set_parameter(self.amount(a));
            return;
        }
        if let Some(d) = self.drag.take() {
            self.let_go(d, true);
        }
    }

    /// Every gesture still open ended (a wheel's, a drag's, a slider's; the grip let go):
    /// before another begins, as the editor closes and as its window loses the focus or the
    /// pointer's capture. A gesture never begins inside another (decisions.md R18).
    fn end_gestures(&mut self) {
        self.end_scroll();
        self.resizing = None;
        if let Some(a) = self.sliding.take() {
            self.setter().end_set_parameter(self.amount(a));
        }
        if let Some(d) = self.drag.take() {
            self.let_go(d, false);
        }
    }

    /// A drag over, its gesture ended: `released` by the pointer (a press that did not move
    /// then picks what it pressed), else cut short.
    fn let_go(&mut self, d: Drag, released: bool) {
        let kind = CONTROLS[d.control].kind;
        let mut v = d.value;
        if released && !d.moved {
            let picked = match d.pressed {
                Target::Legend(_, k) => Some(k as f64 / 5.0),
                _ => {
                    let (cx, _) = CONTROLS[d.control].centre();
                    interact::click(&kind, v, self.in_drawing(d.from).0 - cx)
                }
            };
            if let Some(p) = picked {
                v = p;
            }
        }
        v = interact::release(&kind, v);
        let Some(p) = self.operated(d.control) else {
            return;
        };
        let s = self.setter();
        if matches!(kind, Kind::Rocker { .. }) {
            if v != d.value {
                p.begin(&s);
                p.set(&s, v as f32);
                p.end(&s);
            }
        } else {
            if v != d.value {
                p.set(&s, v as f32);
            }
            p.end(&s);
        }
    }

    fn scrolled(&mut self, delta: ScrollDelta, modifiers: Modifiers) {
        if self.drag.is_some() || self.sliding.is_some() {
            return;
        }
        let (x, y) = self.in_drawing(self.pointer);
        if self.in_drawer((x, y)).is_some() {
            // The list scrolls a row for each row's travel, the rest kept for the next: a
            // wheel's lines (a slow turn on macOS gives fractions of one), a trackpad's or a
            // Magic Mouse's points as the list's rows at this size (each event a few points,
            // which a whole-notch threshold had dropped). A turn the other way starts again.
            let rows = match delta {
                ScrollDelta::Lines { y, .. } => f64::from(y),
                ScrollDelta::Pixels { y, .. } => {
                    f64::from(y) * self.dpr / self.renderer.scale() / ROW_H
                }
            };
            if rows * self.list_travel < 0.0 {
                self.list_travel = 0.0;
            }
            self.list_travel += rows;
            let whole = self.list_travel.trunc();
            if whole != 0.0 {
                self.browser.scroll(-(whole as i32));
                self.list_travel -= whole;
            }
            return;
        }
        if self.on_bar((x, y)).is_some() {
            return;
        }
        if y >= art::H {
            // VOICES steps with the wheel, a notch a voice.
            if let (Some(StripTarget::Fewer | StripTarget::More), ScrollDelta::Lines { y: dy, .. }) =
                (strip::hit(x, y - art::H), delta)
                && dy != 0.0
            {
                self.end_scroll();
                self.step_voices(if dy > 0.0 { 1 } else { -1 });
            }
            return;
        }
        let Some(Target::Control(i) | Target::Legend(i, _)) =
            interact::hit(self.renderer.layout(), x, y)
        else {
            return;
        };
        let (notches, travel) = match delta {
            ScrollDelta::Lines { y, .. } => (
                if y > 0.0 {
                    1.0
                } else if y < 0.0 {
                    -1.0
                } else {
                    0.0
                },
                0.0,
            ),
            ScrollDelta::Pixels { y, .. } => {
                let travel = self
                    .scroll
                    .as_ref()
                    .filter(|s| s.control == i)
                    .map_or(0.0, |s| s.travel)
                    + f64::from(y);
                if travel.abs() < NOTCH {
                    (0.0, travel)
                } else {
                    (travel.signum(), 0.0)
                }
            }
        };
        if self.scroll.as_ref().is_some_and(|s| s.control != i) {
            self.end_scroll();
        }
        if self.scroll.is_none() {
            let value = self.value_of(i);
            if let Some(p) = self.operated(i) {
                p.begin(&self.setter());
            }
            self.scroll = Some(Scroll {
                control: i,
                value,
                last: Instant::now(),
                travel: 0.0,
            });
        }
        let kind = CONTROLS[i].kind;
        let Some(s) = &mut self.scroll else { return };
        s.last = Instant::now();
        s.travel = travel;
        if notches != 0.0 {
            let v = interact::step(
                &kind,
                s.value,
                notches > 0.0,
                modifiers.contains(Modifiers::SHIFT),
            );
            if v != s.value {
                s.value = v;
                if let Some(p) = self.operated(i) {
                    p.set(&self.setter(), v as f32);
                }
            }
        }
    }

    fn end_scroll(&mut self) {
        let Some(s) = self.scroll.take() else { return };
        let kind = CONTROLS[s.control].kind;
        let v = interact::release(&kind, s.value);
        if let Some(p) = self.operated(s.control) {
            let setter = self.setter();
            if v != s.value {
                p.set(&setter, v as f32);
            }
            p.end(&setter);
        }
    }

    /// The width the grip took the window to since the last frame, asked of the host.
    fn grip(&mut self) {
        if let Some(width) = self.gripped.take() {
            self.resize(width);
        }
    }

    /// Asks the host for a window `width` wide: whether it agreed.
    fn resize(&mut self, width: u32) -> bool {
        if width == self.width {
            return false;
        }
        let old = self.params.editor_width.swap(width, Ordering::Relaxed);
        let agreed = self.context.request_resize();
        if agreed {
            self.width = width;
            self.resizing_window = true;
        } else {
            self.params.editor_width.store(old, Ordering::Relaxed);
        }
        agreed
    }

    /// The scene from the parameters, the controls operated and what the audio reports.
    fn update_scene(&mut self) {
        for (i, c) in CONTROLS.iter().enumerate() {
            self.scene.values[i] = match (&self.drag, &self.scroll) {
                (Some(d), _) if d.control == i && !matches!(c.kind, Kind::Rocker { .. }) => d.value,
                (_, Some(s)) if s.control == i => s.value,
                _ => self.value_of(i),
            };
        }
        self.scene.power = !self.params.bypass.value();
        self.scene.overload = Meters::load(&self.meters.overload);
        self.scene.midi = (
            Meters::load(&self.meters.bend),
            Meters::load(&self.meters.modulation),
        );
        self.scene.tip = self.tip();
        let p = &self.params;
        self.strip_scene.poly = p.poly.value();
        self.strip_scene.voices = u32::try_from(p.voices.value()).unwrap_or(4);
        self.strip_scene.entropy = f64::from(p.entropy.unmodulated_normalized_value());
        self.strip_scene.spread = f64::from(p.spread.unmodulated_normalized_value());
    }

    /// The tip: over the control operated, else over the one under the pointer.
    fn tip(&self) -> Option<(String, f64, f64)> {
        let operated = self
            .drag
            .as_ref()
            .map(|d| d.control)
            .or(self.scroll.as_ref().map(|s| s.control));
        let target = match operated {
            Some(i) => Target::Control(i),
            None => self.hover?,
        };
        match target {
            Target::Control(i) | Target::Legend(i, _) => {
                let c = &CONTROLS[i];
                let p = self.operated(i)?;
                let (x, y) = c.centre();
                let top = art::bounds(&c.kind)[1];
                let label = if c.param == "feedback" && feedback_silent(&self.scene.values) {
                    FEEDBACK_SILENT
                } else {
                    c.label
                };
                Some((
                    format!("{label}: {}", p.text(self.scene.values[i] as f32)),
                    x,
                    y + top,
                ))
            }
            Target::Power => {
                let state = if self.params.bypass.value() {
                    "Off (bypassed)"
                } else {
                    "On"
                };
                Some((
                    format!("POWER: {state}"),
                    art::COL + POWER.0,
                    art::TOP + POWER.1 - 64.0,
                ))
            }
            Target::Grip => None,
        }
    }

    /// Brings the frame up to the parameters and the gestures: whether it changed.
    fn draw(&mut self) -> bool {
        if self
            .scroll
            .as_ref()
            .is_some_and(|s| s.last.elapsed() > SCROLL_REST)
        {
            self.end_scroll();
        }
        self.update_scene();
        self.browser.tick(&self.params);
        self.update.tick();
        self.browser.drawer.update = self.update.scene();
        self.shrink_shut_drawer();
        let panel = self.renderer.render(&self.scene);
        self.strip_scene.bar.clone_from(&self.browser.bar);
        let strip = self.strip.render(&self.strip_scene);
        // The drawer over the panel, while it shows (and moves).
        let k = self.browser.reveal();
        let mut drawer = false;
        if k > 0.0 && self.below() {
            // Below the bar: sliding down from under it.
            drawer = self.drawer.render(&self.browser.drawer);
            if self.browser.sliding() {
                let frame = self.drawer.frame();
                let s = self.slid.get_or_insert_with(|| frame.clone());
                if (s.width(), s.height()) != (frame.width(), frame.height()) {
                    *s = frame.clone();
                }
                s.fill(ca72_panel::presets::background());
                let top = (-(1.0 - k) * DRAWER_H * self.renderer.scale()).round();
                overlay(s, frame, top as i32);
                drawer = true;
            } else if self.slid.take().is_some() {
                drawer = true;
            }
        } else if k > 0.0 {
            drawer = self.drawer.render(&self.browser.drawer);
            let sliding = self.browser.sliding();
            if panel || drawer || sliding || self.composed.is_none() {
                let frame = self.renderer.frame();
                let c = self.composed.get_or_insert_with(|| frame.clone());
                if (c.width(), c.height()) == (frame.width(), frame.height()) {
                    c.data_mut().copy_from_slice(frame.data());
                } else {
                    *c = frame.clone();
                }
                let top = ((DRAWER_TOP + (1.0 - k) * DRAWER_H) * self.renderer.scale()).round();
                overlay(c, self.drawer.frame(), top as i32);
                drawer = true;
            }
        } else if self.composed.take().is_some() {
            drawer = true;
        }
        panel || strip || drawer
    }

    /// The pointer's events: whether the event was the panel's.
    fn mouse(&mut self, e: MouseEvent) -> bool {
        match e {
            MouseEvent::CursorMoved {
                position,
                modifiers,
            } => {
                self.pointer = (position.x, position.y);
                self.moved(modifiers);
            }
            MouseEvent::ButtonPressed {
                button: MouseButton::Left,
                modifiers,
            } => self.press(modifiers),
            MouseEvent::ButtonReleased {
                button: MouseButton::Left,
                ..
            } => self.released(),
            MouseEvent::WheelScrolled { delta, modifiers } => self.scrolled(delta, modifiers),
            MouseEvent::CursorLeft => {
                if self.drag.is_none() {
                    self.hover = None;
                }
            }
            _ => return false,
        }
        true
    }
}

impl Editing {
    /// A key: whether the drawer took it (else it is the host's).
    fn keyed(&mut self, k: &KeyboardEvent) -> bool {
        // A key the drawer acts on may set the parameters (the arrows choose presets): no
        // gesture is left open under it. Shift and the other modifiers alone leave a drag.
        if self.browser.open && k.state == KeyState::Down && !modifier(&k.key) {
            self.end_gestures();
        }
        let setter = ParamSetter::new(self.context.as_ref());
        let taken = self.browser.key(k, &self.params, &setter);
        self.follow_drawer();
        taken
    }
}

impl Drop for Editing {
    // The editor closed mid-drag, or within a wheel's rest: no parameter is left touched.
    fn drop(&mut self) {
        self.end_gestures();
    }
}

/// How long a frame shown on an X11 window is trusted to stay shown. X11 drops what is drawn
/// to a window not yet on screen (a host maps the editor's window after its first frame:
/// Bitwig on Linux showed it black until the pointer changed something) and, without a
/// compositor, what a covering window hid; baseview reports neither. So on Linux the last
/// frame is shown again at this pace, unchanged (a copy, no rendering; decisions.md R19).
/// macOS and Windows keep a window's pixels.
const REPAINT: Option<Duration> = if cfg!(target_os = "linux") {
    Some(Duration::from_millis(250))
} else {
    None
};

/// Whether the frame must be shown again though nothing in it changed: not yet shown since
/// the window opened or resized, or (Linux) shown longer ago than [`REPAINT`].
fn repaint_due(shown: Option<Instant>, now: Instant) -> bool {
    match (shown, REPAINT) {
        (None, _) => true,
        (Some(at), Some(every)) => now.saturating_duration_since(at) >= every,
        (Some(_), None) => false,
    }
}

impl WindowHandler for PanelWindow {
    fn on_frame(&mut self, window: &mut Window<'_>) {
        // A drag whose window lost the pointer's capture (Windows: Alt-Tab mid-drag) never
        // sees its release: it ends here (decisions.md R18).
        if self.editing.dragging() && window::lost_capture(window) {
            self.editing.end_gestures();
        }
        // The grip's width, asked of the host once a frame.
        self.editing.grip();
        self.follow(window);
        let now = Instant::now();
        let changed = self.editing.draw();
        // A drawer that has just slid shut has asked the host for the shorter window: the
        // editor's view follows in the same frame, before anything is shown. A frame between
        // them showed, on macOS, the tall view in the short window, its top cut off (the
        // owner, 2026-10-03: a flicker as the drawer finished shutting; decisions.md R19).
        self.follow(window);
        if changed || repaint_due(self.shown, now) {
            self.present();
            self.shown = Some(now);
        }
    }

    fn on_event(&mut self, window: &mut Window<'_>, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                self.resized(&info);
                EventStatus::Captured
            }
            // No gesture is left open (decisions.md R18).
            Event::Window(WindowEvent::Unfocused | WindowEvent::WillClose) => {
                self.editing.end_gestures();
                EventStatus::Ignored
            }
            Event::Mouse(e) => {
                let released = matches!(e, MouseEvent::ButtonReleased { .. });
                let status = if self.editing.mouse(e) {
                    EventStatus::Captured
                } else {
                    EventStatus::Ignored
                };
                if released {
                    window::release_capture(window);
                }
                // The drawer opening or shutting below the bar: the window follows (the
                // owner, 2026-10-02: on macOS it had stayed drawn at the old size until
                // reopened).
                self.follow(window);
                self.keyboard(window);
                status
            }
            // The drawer, while open, takes the keys (the host keeps its shortcuts).
            Event::Keyboard(k) => {
                let taken = self.editing.keyed(&k);
                self.follow(window);
                self.keyboard(window);
                if taken {
                    EventStatus::Captured
                } else {
                    EventStatus::Ignored
                }
            }
            _ => EventStatus::Ignored,
        }
    }
}

/// A modifier key alone.
fn modifier(key: &Key) -> bool {
    matches!(
        key,
        Key::Shift
            | Key::Control
            | Key::Alt
            | Key::AltGraph
            | Key::Meta
            | Key::Super
            | Key::CapsLock
            | Key::Fn
            | Key::FnLock
            | Key::Hyper
            | Key::NumLock
            | Key::ScrollLock
            | Key::Symbol
            | Key::SymbolLock
    )
}

/// The usable part of the screen (less the menu bar, dock or taskbar), in logical pixels
/// (`scale` the window's, where the platform counts physical ones): macOS's main screen,
/// Windows' primary monitor, on X11 the monitor the pointer is on (else the primary).
#[allow(unsafe_code)]
mod screen {
    #[cfg(target_os = "macos")]
    pub fn usable(_scale: Option<f64>) -> Option<(f64, f64)> {
        use cocoa::appkit::NSScreen;
        use cocoa::base::nil;
        // SAFETY: a class method of AppKit's, nil where there is no screen.
        let screen = unsafe { NSScreen::mainScreen(nil) };
        if screen == nil {
            return None;
        }
        // SAFETY: `screen` is an NSScreen.
        let frame = unsafe { NSScreen::visibleFrame(screen) };
        Some((frame.size.width, frame.size.height))
    }

    #[cfg(target_os = "windows")]
    pub fn usable(scale: Option<f64>) -> Option<(f64, f64)> {
        use winapi::shared::windef::RECT;
        use winapi::um::winuser::{SPI_GETWORKAREA, SystemParametersInfoW};
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: SPI_GETWORKAREA writes one RECT through the pointer.
        let ok =
            unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut r as *mut RECT).cast(), 0) };
        if ok == 0 {
            return None;
        }
        // The work area is in the pixels of the host's thread, which the window counts in
        // unless the host gave a scale (decisions.md R18).
        let k = scale.unwrap_or(1.0);
        Some((
            f64::from(r.right - r.left) / k,
            f64::from(r.bottom - r.top) / k,
        ))
    }

    /// The X11 screen spans every monitor: two side by side made the editor open wider
    /// than the one it is on, its grip off it (decisions.md R18). RandR's monitors (1.5),
    /// the window manager's work area (`_NET_WORKAREA`), through a connection of our own.
    #[cfg(target_os = "linux")]
    pub fn usable(scale: Option<f64>) -> Option<(f64, f64)> {
        use x11rb::connection::Connection;
        use x11rb::protocol::randr::ConnectionExt as _;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
        let (conn, n) = x11rb::connect(None).ok()?;
        let screen = conn.setup().roots.get(n)?;
        let root = screen.root;
        let whole = Rect {
            x: 0,
            y: 0,
            w: i32::from(screen.width_in_pixels),
            h: i32::from(screen.height_in_pixels),
        };
        let monitors: Vec<(Rect, bool)> = conn
            .randr_query_version(1, 5)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|_| conn.randr_get_monitors(root, true).ok()?.reply().ok())
            .map(|r| {
                r.monitors
                    .iter()
                    .map(|m| {
                        let r = Rect {
                            x: i32::from(m.x),
                            y: i32::from(m.y),
                            w: i32::from(m.width),
                            h: i32::from(m.height),
                        };
                        (r, m.primary)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let pointer = conn
            .query_pointer(root)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|p| (i32::from(p.root_x), i32::from(p.root_y)));
        let work = conn
            .intern_atom(true, b"_NET_WORKAREA")
            .ok()
            .and_then(|c| c.reply().ok())
            .filter(|a| a.atom != 0)
            .and_then(|a| {
                let p = conn
                    .get_property(false, root, a.atom, AtomEnum::CARDINAL, 0, 4)
                    .ok()?
                    .reply()
                    .ok()?;
                let v: Vec<u32> = p.value32()?.collect();
                let [x, y, w, h] = v[..] else { return None };
                Some(Rect {
                    x: i32::try_from(x).ok()?,
                    y: i32::try_from(y).ok()?,
                    w: i32::try_from(w).ok()?,
                    h: i32::try_from(h).ok()?,
                })
            });
        let area = monitor_area(whole, &monitors, pointer, work);
        let k = scale.unwrap_or(1.0);
        Some((f64::from(area.w) / k, f64::from(area.h) / k))
    }

    /// A rectangle of the X11 screen, in pixels.
    #[cfg(any(target_os = "linux", test))]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Rect {
        pub x: i32,
        pub y: i32,
        pub w: i32,
        pub h: i32,
    }

    #[cfg(any(target_os = "linux", test))]
    impl Rect {
        fn contains(&self, (x, y): (i32, i32)) -> bool {
            (self.x..self.x + self.w).contains(&x) && (self.y..self.y + self.h).contains(&y)
        }

        fn and(&self, o: &Rect) -> Option<Rect> {
            let (x, y) = (self.x.max(o.x), self.y.max(o.y));
            let (w, h) = (
                (self.x + self.w).min(o.x + o.w) - x,
                (self.y + self.h).min(o.y + o.h) - y,
            );
            (w > 0 && h > 0).then_some(Rect { x, y, w, h })
        }
    }

    /// Where on the X11 screen the editor opens: the monitor the pointer is on (where the
    /// host was just clicked), else the primary, else the first, else the whole screen; less
    /// the window manager's panels where its work area says (one rectangle for all the
    /// monitors, so only its part on this monitor counts).
    #[cfg(any(target_os = "linux", test))]
    pub fn monitor_area(
        whole: Rect,
        monitors: &[(Rect, bool)],
        pointer: Option<(i32, i32)>,
        work: Option<Rect>,
    ) -> Rect {
        let monitor = pointer
            .and_then(|p| monitors.iter().find(|(r, _)| r.contains(p)))
            .or_else(|| monitors.iter().find(|(_, primary)| *primary))
            .or(monitors.first())
            .map_or(whole, |(r, _)| *r);
        work.and_then(|w| monitor.and(&w)).unwrap_or(monitor)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    pub fn usable(_scale: Option<f64>) -> Option<(f64, f64)> {
        None
    }
}

/// The platform's windows: the host's parent window as baseview takes it, softbuffer's
/// surface on baseview's window (the two use different versions of `raw-window-handle`), and
/// what baseview leaves out: the keyboard given back, and the pointer's capture (Windows).
#[allow(unsafe_code)]
mod window {
    use std::num::{NonZeroIsize, NonZeroU32};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr::NonNull;

    use baseview::WindowHandle;
    use nih_plug::editor::ParentWindowHandle;
    use raw_window_handle::{
        HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    };
    use raw_window_handle_06 as rwh6;

    /// The host's window the editor opens in.
    pub struct Parent(pub ParentWindowHandle);

    impl Parent {
        /// Whether a window can open in it: the host gave one, and on Linux there is an X11
        /// display (baseview panics in the host's thread without one).
        pub fn can_open(&self) -> bool {
            let given = match self.0 {
                ParentWindowHandle::X11Window(w) => w != 0,
                ParentWindowHandle::AppKitNsView(v) => !v.is_null(),
                ParentWindowHandle::Win32Hwnd(h) => !h.is_null(),
            };
            given && display()
        }
    }

    #[cfg(target_os = "linux")]
    fn display() -> bool {
        use x11::xlib;
        // SAFETY: a connection of our own, closed at once.
        unsafe {
            let d = xlib::XOpenDisplay(std::ptr::null());
            if d.is_null() {
                return false;
            }
            xlib::XCloseDisplay(d);
        }
        true
    }

    #[cfg(not(target_os = "linux"))]
    fn display() -> bool {
        true
    }

    // SAFETY: the handle is the host's window, alive while the editor is open, and given as
    // the platform's own kind.
    unsafe impl HasRawWindowHandle for Parent {
        fn raw_window_handle(&self) -> RawWindowHandle {
            match self.0 {
                ParentWindowHandle::X11Window(window) => {
                    let mut h = raw_window_handle::XcbWindowHandle::empty();
                    h.window = window;
                    RawWindowHandle::Xcb(h)
                }
                ParentWindowHandle::AppKitNsView(ns_view) => {
                    let mut h = raw_window_handle::AppKitWindowHandle::empty();
                    h.ns_view = ns_view;
                    RawWindowHandle::AppKit(h)
                }
                ParentWindowHandle::Win32Hwnd(hwnd) => {
                    let mut h = raw_window_handle::Win32WindowHandle::empty();
                    h.hwnd = hwnd;
                    RawWindowHandle::Win32(h)
                }
            }
        }
    }

    /// The open window (if one could open), closed when dropped, the editor then reset.
    pub struct Handle {
        window: Option<WindowHandle>,
        _closing: super::Closing,
    }

    impl Handle {
        pub fn new(window: Option<WindowHandle>, closing: super::Closing) -> Self {
            Handle {
                window,
                _closing: closing,
            }
        }
    }

    // SAFETY: the host drops the editor's handle on its GUI thread, where it opened it; the
    // raw pointers inside are not used from any other.
    unsafe impl Send for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            if let Some(w) = &mut self.window {
                w.close();
            }
        }
    }

    /// The keyboard as the drawer took it from another window (Windows: that window).
    #[derive(Debug)]
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub struct Keys {
        #[cfg(target_os = "windows")]
        had: winapi::shared::windef::HWND,
    }

    #[cfg(target_os = "windows")]
    fn hwnd(window: &baseview::Window<'_>) -> Option<winapi::shared::windef::HWND> {
        match window.raw_window_handle() {
            RawWindowHandle::Win32(h) if !h.hwnd.is_null() => Some(h.hwnd.cast()),
            _ => None,
        }
    }

    /// The keyboard to the editor's window (baseview's focus): on Windows, the window that
    /// had it if another did. Not on X11, where baseview's focus is `unimplemented!()` (it
    /// panicked the editor's thread, the window gone): there the keys go to the window under
    /// the pointer within the host's focused window, so to the drawer while the pointer is
    /// over the editor.
    pub fn take_keys(window: &mut baseview::Window<'_>) -> Option<Keys> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::GetFocus;
            let hwnd = hwnd(window)?;
            // SAFETY: the calling thread's focus, the editor's window being on it.
            let had = unsafe { GetFocus() };
            window.focus();
            (had != hwnd).then_some(Keys { had })
        }
        #[cfg(target_os = "macos")]
        {
            window.focus();
            None
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = window;
            None
        }
    }

    /// The keyboard given back as the drawer shuts, to the window it was taken from (else
    /// the host's window holding the editor's), if the editor's window still has it.
    /// baseview on Windows keeps every key its window is given, the host's shortcuts too.
    pub fn give_keys_back(window: &baseview::Window<'_>, keys: Keys) {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{GetFocus, GetParent, IsWindow, SetFocus};
            let Some(hwnd) = hwnd(window) else { return };
            // SAFETY: the calling thread's focus and windows; `keys.had` is checked to be a
            // window still before it is given the focus.
            unsafe {
                if GetFocus() != hwnd {
                    return;
                }
                let to = if !keys.had.is_null() && IsWindow(keys.had) != 0 {
                    keys.had
                } else {
                    GetParent(hwnd)
                };
                SetFocus(to);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, keys);
        }
    }

    /// Whether the editor's window has lost the pointer's capture (Windows: baseview
    /// captures it at a press and lets it go at the release, and ignores its loss, at Alt-Tab
    /// mid-drag, when no release comes). Elsewhere a release always comes.
    pub fn lost_capture(window: &baseview::Window<'_>) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::GetCapture;
            // SAFETY: the calling thread's capture.
            hwnd(window).is_some_and(|hwnd| unsafe { GetCapture() } != hwnd)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            false
        }
    }

    /// The pointer's capture let go once no button is down (Windows). baseview counts the
    /// presses and releases to let it go; a release lost with the capture leaves the count
    /// one up, and its window then kept the capture after every release, the host's own
    /// clicks coming to the editor.
    pub fn release_capture(window: &baseview::Window<'_>) {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{
                GetCapture, GetKeyState, ReleaseCapture, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
                VK_XBUTTON1, VK_XBUTTON2,
            };
            let Some(hwnd) = hwnd(window) else { return };
            // SAFETY: the calling thread's capture and the buttons as its messages have them.
            unsafe {
                let down = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
                    .iter()
                    .any(|&b| GetKeyState(b) < 0);
                if !down && GetCapture() == hwnd {
                    ReleaseCapture();
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
        }
    }

    /// baseview's window as softbuffer takes it.
    #[derive(Clone)]
    pub struct Target {
        display: rwh6::RawDisplayHandle,
        window: rwh6::RawWindowHandle,
    }

    impl rwh6::HasDisplayHandle for Target {
        fn display_handle(&self) -> Result<rwh6::DisplayHandle<'_>, rwh6::HandleError> {
            // SAFETY: the display outlives the window drawn in, which outlives the surface.
            Ok(unsafe { rwh6::DisplayHandle::borrow_raw(self.display) })
        }
    }

    impl rwh6::HasWindowHandle for Target {
        fn window_handle(&self) -> Result<rwh6::WindowHandle<'_>, rwh6::HandleError> {
            // SAFETY: the window outlives its surface (both are the editor window's).
            Ok(unsafe { rwh6::WindowHandle::borrow_raw(self.window) })
        }
    }

    fn target(window: &baseview::Window<'_>) -> Option<Target> {
        let display = match window.raw_display_handle() {
            RawDisplayHandle::AppKit(_) => {
                rwh6::RawDisplayHandle::AppKit(rwh6::AppKitDisplayHandle::new())
            }
            RawDisplayHandle::Xlib(h) => rwh6::RawDisplayHandle::Xlib(
                rwh6::XlibDisplayHandle::new(NonNull::new(h.display), h.screen),
            ),
            RawDisplayHandle::Xcb(h) => rwh6::RawDisplayHandle::Xcb(rwh6::XcbDisplayHandle::new(
                NonNull::new(h.connection),
                h.screen,
            )),
            RawDisplayHandle::Windows(_) => {
                rwh6::RawDisplayHandle::Windows(rwh6::WindowsDisplayHandle::new())
            }
            _ => return None,
        };
        let window = match window.raw_window_handle() {
            RawWindowHandle::AppKit(h) => rwh6::RawWindowHandle::AppKit(
                rwh6::AppKitWindowHandle::new(NonNull::new(h.ns_view)?),
            ),
            RawWindowHandle::Xlib(h) => {
                rwh6::RawWindowHandle::Xlib(rwh6::XlibWindowHandle::new(h.window))
            }
            RawWindowHandle::Xcb(h) => {
                rwh6::RawWindowHandle::Xcb(rwh6::XcbWindowHandle::new(NonZeroU32::new(h.window)?))
            }
            RawWindowHandle::Win32(h) => {
                let mut w = rwh6::Win32WindowHandle::new(NonZeroIsize::new(h.hwnd as isize)?);
                w.hinstance = NonZeroIsize::new(h.hinstance as isize);
                rwh6::RawWindowHandle::Win32(w)
            }
            _ => return None,
        };
        Some(Target { display, window })
    }

    /// softbuffer's call, or nothing if it panicked: its Win32 backend asserts that GDI
    /// gave it a device context and a bitmap, which without a desktop it may not, and a
    /// panic there crossed into the host (pluginval crashed, decisions.md R18).
    fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
        catch_unwind(AssertUnwindSafe(f)).ok()
    }

    /// Where the frames are shown; nothing if the platform's window could not be drawn in.
    pub struct Surface {
        inner: Option<(
            softbuffer::Context<Target>,
            softbuffer::Surface<Target, Target>,
        )>,
    }

    impl Surface {
        pub fn new(window: &baseview::Window<'_>, size: (u32, u32)) -> Self {
            let inner = target(window).and_then(|t| {
                guarded(|| {
                    let context = softbuffer::Context::new(t.clone()).ok()?;
                    let surface = softbuffer::Surface::new(&context, t).ok()?;
                    Some((context, surface))
                })
                .flatten()
            });
            let mut s = Surface { inner };
            s.resize(size);
            s
        }

        pub fn resize(&mut self, (w, h): (u32, u32)) {
            if let (Some((_, surface)), Some(w), Some(h)) =
                (&mut self.inner, NonZeroU32::new(w), NonZeroU32::new(h))
                && guarded(|| surface.resize(w, h)).is_none()
            {
                self.inner = None;
            }
        }

        /// Shows frames (premultiplied RGBA, opaque), one under the other from the top left
        /// corner, in a window of `size` pixels.
        pub fn present(&mut self, size: (u32, u32), frames: &[&ca72_panel::Pixmap]) {
            let Some((_, surface)) = &mut self.inner else {
                return;
            };
            if guarded(|| Self::show(surface, size, frames)).is_none() {
                self.inner = None;
            }
        }

        fn show(
            surface: &mut softbuffer::Surface<Target, Target>,
            (w, h): (u32, u32),
            frames: &[&ca72_panel::Pixmap],
        ) {
            let Ok(mut buffer) = surface.buffer_mut() else {
                return;
            };
            let (w, h) = (w as usize, h as usize);
            if buffer.len() < w * h {
                return;
            }
            let mut frames = frames.iter();
            let mut frame = frames.next();
            let mut top = 0usize;
            for y in 0..h {
                while let Some(f) = frame
                    && y >= top + f.height() as usize
                {
                    top += f.height() as usize;
                    frame = frames.next();
                }
                let row = &mut buffer[y * w..(y + 1) * w];
                for (x, out) in row.iter_mut().enumerate() {
                    *out = match frame {
                        Some(f) if x < f.width() as usize => {
                            let (fw, fy) = (f.width() as usize, y - top);
                            let p = &f.data()[(fy * fw + x) * 4..(fy * fw + x) * 4 + 4];
                            (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])
                        }
                        _ => 0x003b_2213,
                    };
                }
            }
            let _ = buffer.present();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use baseview::Point;
    use nih_plug::context::PluginApi;
    use nih_plug::wrapper::state::PluginState;

    use super::*;
    use ca72_panel::presets::{self as presets_ui, BarTarget, DrawerTarget, FieldId, RowAction};

    /// What the editor asked of the host.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Call {
        Begin(ParamPtr),
        Set(ParamPtr, f32),
        End(ParamPtr),
    }

    /// A host that records the editor's gestures (and does not apply them).
    #[derive(Default)]
    struct Host {
        calls: Mutex<Vec<Call>>,
        /// It will not resize the window.
        fixed: AtomicBool,
        /// How often it was asked to.
        resizes: AtomicU32,
    }

    impl Host {
        fn take(&self) -> Vec<Call> {
            std::mem::take(&mut *self.calls.lock().expect("not poisoned"))
        }
        fn push(&self, c: Call) {
            self.calls.lock().expect("not poisoned").push(c);
        }
    }

    #[allow(unsafe_code)]
    impl GuiContext for Host {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn request_resize(&self) -> bool {
            self.resizes.fetch_add(1, Ordering::Relaxed);
            !self.fixed.load(Ordering::Relaxed)
        }
        unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
            self.push(Call::Begin(param));
        }
        unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
            self.push(Call::Set(param, normalized));
        }
        unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
            self.push(Call::End(param));
        }
        fn get_state(&self) -> PluginState {
            panic!("the editor does not read the state")
        }
        fn set_state(&self, _state: PluginState) {}
    }

    fn editing() -> (Editing, Arc<Host>, Arc<Ca72Params>) {
        // A library that is not there (nothing of the user's read; these tests write none).
        editing_with(Library::at(
            std::env::temp_dir().join("ca72-editor-tests-no-library"),
        ))
    }

    fn editing_with(library: Library) -> (Editing, Arc<Host>, Arc<Ca72Params>) {
        let host = Arc::new(Host::default());
        let params = Arc::new(Ca72Params::default());
        let context: Arc<dyn GuiContext> = host.clone();
        let e = Editing::new(
            context,
            Arc::clone(&params),
            Arc::new(Meters::default()),
            (1720, 1720, 1.0),
            library,
            Arc::new(AtomicBool::new(false)),
            // No screen known: the drawer fits below.
            |_| None,
        );
        (e, host, params)
    }

    /// Every gesture well formed: a parameter's begin only while it has none open, its
    /// changes and end only inside one, and none left open.
    fn well_formed(calls: &[Call]) {
        let mut open: Vec<ParamPtr> = Vec::new();
        for (i, c) in calls.iter().enumerate() {
            match *c {
                Call::Begin(p) => {
                    assert!(
                        !open.contains(&p),
                        "a begin inside a gesture at {i}: {calls:?}"
                    );
                    open.push(p);
                }
                Call::Set(p, _) => {
                    assert!(
                        open.contains(&p),
                        "a change outside a gesture at {i}: {calls:?}"
                    )
                }
                Call::End(p) => {
                    let k = open.iter().position(|&q| q == p);
                    let k = k.unwrap_or_else(|| panic!("an end with no begin at {i}: {calls:?}"));
                    open.remove(k);
                }
            }
        }
        assert!(open.is_empty(), "left open: {calls:?}");
    }

    fn press(e: &mut Editing) {
        e.mouse(MouseEvent::ButtonPressed {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
    }

    fn release(e: &mut Editing) {
        e.mouse(MouseEvent::ButtonReleased {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
    }

    fn wheel(e: &mut Editing, lines: f32) {
        e.mouse(MouseEvent::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: lines },
            modifiers: Modifiers::empty(),
        });
    }

    fn below(p: Point, dy: f64) -> Point {
        Point {
            x: p.x,
            y: p.y + dy,
        }
    }

    /// Where a point of the drawing is in the window, in logical pixels.
    fn at(e: &Editing, (x, y): (f64, f64)) -> Point {
        let k = e.renderer.scale() / e.dpr;
        Point { x: x * k, y: y * k }
    }

    fn centre(param: &str) -> (f64, f64) {
        CONTROLS[ca72_panel::controls::index(param).expect("a control")].centre()
    }

    fn go(e: &mut Editing, p: Point) {
        e.mouse(MouseEvent::CursorMoved {
            position: p,
            modifiers: Modifiers::empty(),
        });
    }

    fn click(e: &mut Editing, p: Point) {
        go(e, p);
        e.mouse(MouseEvent::ButtonPressed {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        e.mouse(MouseEvent::ButtonReleased {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
    }

    #[test]
    fn a_drag_is_one_gesture_of_its_knob() {
        let (mut e, host, params) = editing();
        let p = at(&e, centre("cutoff"));
        go(&mut e, p);
        e.mouse(MouseEvent::ButtonPressed {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        go(
            &mut e,
            Point {
                x: p.x,
                y: p.y - 2.0,
            },
        );
        go(
            &mut e,
            Point {
                x: p.x,
                y: p.y - 60.0,
            },
        );
        go(
            &mut e,
            Point {
                x: p.x,
                y: p.y - 300.0,
            },
        );
        e.mouse(MouseEvent::ButtonReleased {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        let cutoff = params.cutoff.as_ptr();
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(cutoff),
                Call::Set(cutoff, 0.75),
                Call::Set(cutoff, 1.0),
                Call::End(cutoff)
            ]
        );
    }

    #[test]
    fn clicks_flip_switches_pick_legends_and_turn_power() {
        let (mut e, host, params) = editing();
        {
            let p = at(&e, centre("osc1_on"));
            click(&mut e, p);
        }
        let on = params.osc1_on.as_ptr();
        assert_eq!(
            host.take(),
            vec![Call::Begin(on), Call::Set(on, 0.0), Call::End(on)]
        );

        let i = ca72_panel::controls::index("osc1_range").expect("a control");
        let Kind::Selector(set) = CONTROLS[i].kind else {
            panic!("a selector")
        };
        let (cx, cy) = CONTROLS[i].centre();
        let (lx, ly) = e.renderer.layout().legend_at(set, 0);
        {
            let p = at(&e, (cx + lx, cy + ly));
            click(&mut e, p);
        }
        let range = params.osc1_range.as_ptr();
        assert_eq!(
            host.take(),
            vec![Call::Begin(range), Call::Set(range, 0.0), Call::End(range)]
        );

        {
            let p = at(&e, (art::COL + POWER.0, art::TOP + POWER.1));
            click(&mut e, p);
        }
        let bypass = params.bypass.as_ptr();
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(bypass),
                Call::Set(bypass, 1.0),
                Call::End(bypass)
            ]
        );
    }

    #[test]
    fn a_double_click_restores_the_default() {
        let (mut e, host, params) = editing();
        let p = at(&e, centre("osc2_frequency"));
        click(&mut e, p);
        host.take();
        click(&mut e, p);
        let f = params.osc2_frequency.as_ptr();
        assert_eq!(
            host.take(),
            vec![Call::Begin(f), Call::Set(f, 0.5), Call::End(f)]
        );
    }

    #[test]
    fn the_pitch_wheel_settles_in_its_detent() {
        let (mut e, host, params) = editing();
        let p = at(&e, centre("pitch_wheel"));
        go(&mut e, p);
        e.mouse(MouseEvent::ButtonPressed {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        go(
            &mut e,
            Point {
                x: p.x,
                y: p.y - 5.0,
            },
        );
        e.mouse(MouseEvent::ButtonReleased {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        let w = params.pitch_wheel.as_ptr();
        let calls = host.take();
        assert_eq!(calls.first(), Some(&Call::Begin(w)));
        assert_eq!(
            &calls[calls.len() - 2..],
            &[Call::Set(w, 0.5), Call::End(w)]
        );
    }

    #[test]
    fn the_mouse_wheel_turns_a_knob_until_it_rests() {
        let (mut e, host, params) = editing();
        {
            let p = at(&e, centre("emphasis"));
            go(&mut e, p);
        }
        for _ in 0..2 {
            e.mouse(MouseEvent::WheelScrolled {
                delta: ScrollDelta::Lines { x: 0.0, y: 1.0 },
                modifiers: Modifiers::empty(),
            });
        }
        let emphasis = params.emphasis.as_ptr();
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(emphasis),
                Call::Set(emphasis, 0.02),
                Call::Set(emphasis, 0.04)
            ]
        );
        if let Some(s) = &mut e.scroll {
            s.last -= SCROLL_REST * 2;
        }
        e.draw();
        assert_eq!(host.take(), vec![Call::End(emphasis)]);
    }

    /// A wheel's gesture waits for the wheel to rest, and a press ends it before beginning
    /// its own, on that control or another: never a begin inside a gesture, which CLAP
    /// forbids (decisions.md R18).
    #[test]
    fn a_wheels_gesture_ends_before_another_begins() {
        let (mut e, host, params) = editing();
        let cutoff = params.cutoff.as_ptr();
        let p = at(&e, centre("cutoff"));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        // CUTOFF grabbed within the wheel's rest.
        press(&mut e);
        go(&mut e, below(p, -60.0));
        release(&mut e);
        let calls = host.take();
        well_formed(&calls);
        assert_eq!(
            (calls[0], calls[2], calls[3], calls[calls.len() - 1]),
            (
                Call::Begin(cutoff),
                Call::End(cutoff),
                Call::Begin(cutoff),
                Call::End(cutoff)
            ),
            "{calls:?}"
        );
        // A wheel on one knob, then a switch clicked; then VOICES stepped by the wheel.
        let p = at(&e, centre("emphasis"));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        let p = at(&e, centre("osc1_on"));
        click(&mut e, p);
        let p = at(&e, centre("emphasis"));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        let p = on_strip(&e, strip_at(StripTarget::More));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        let calls = host.take();
        well_formed(&calls);
        assert_eq!(
            calls.iter().filter(|c| matches!(c, Call::Begin(_))).count(),
            4
        );
    }

    /// The editor closed within a wheel's rest, or mid-drag: every gesture ends.
    #[test]
    fn closing_the_editor_ends_every_gesture() {
        let (mut e, host, params) = editing();
        let p = at(&e, centre("emphasis"));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        drop(e);
        let calls = host.take();
        well_formed(&calls);
        assert_eq!(calls.last(), Some(&Call::End(params.emphasis.as_ptr())));

        let (mut e, host, _params) = editing();
        let p = at(&e, centre("cutoff"));
        go(&mut e, p);
        press(&mut e);
        go(&mut e, below(p, -60.0));
        drop(e);
        well_formed(&host.take());

        let (mut e, host, _params) = editing();
        let p = on_strip(&e, strip_at(StripTarget::Slider(Amount::Spread)));
        go(&mut e, p);
        press(&mut e);
        go(&mut e, below(p, 0.0));
        drop(e);
        well_formed(&host.take());
    }

    /// A drag whose release never came (on Windows its window lost the pointer's capture,
    /// at Alt-Tab) ends as the next press comes, and as the window loses the focus or the
    /// capture, picking nothing.
    #[test]
    fn a_drag_whose_release_was_lost_ends_at_the_next_press() {
        let (mut e, host, params) = editing();
        let (cutoff, emphasis) = (params.cutoff.as_ptr(), params.emphasis.as_ptr());
        let p = at(&e, centre("cutoff"));
        go(&mut e, p);
        press(&mut e);
        go(&mut e, below(p, -60.0));
        // No release: EMPHASIS pressed next.
        let p = at(&e, centre("emphasis"));
        go(&mut e, p);
        press(&mut e);
        release(&mut e);
        let calls = host.take();
        well_formed(&calls);
        let at_end = calls.iter().position(|c| *c == Call::End(cutoff));
        let at_begin = calls.iter().position(|c| *c == Call::Begin(emphasis));
        assert!(at_end < at_begin && at_end.is_some(), "{calls:?}");
        // RANGE pressed (a click would step it), cut short by the window: nothing picked.
        let range = params.osc1_range.as_ptr();
        let p = at(&e, centre("osc1_range"));
        go(&mut e, p);
        press(&mut e);
        assert!(e.dragging());
        e.end_gestures();
        assert!(!e.dragging());
        release(&mut e);
        assert_eq!(host.take(), vec![Call::Begin(range), Call::End(range)]);
    }

    /// The name plate does nothing (the owner, 2026-10-03: it no longer turns over): a click
    /// on it sets nothing, redraws nothing and shows no tip.
    #[test]
    fn the_name_plate_does_nothing() {
        let (mut e, host, _params) = editing();
        e.draw();
        let p = at(&e, (art::PLATE_X + 20.0, art::PLATE_Y + 20.0));
        click(&mut e, p);
        assert_eq!(host.take(), vec![]);
        assert!(!e.draw(), "redrawn");
        assert_eq!(e.scene.tip, None);
    }

    #[test]
    fn a_tip_names_the_control_and_its_value() {
        let (mut e, _host, _params) = editing();
        {
            let p = at(&e, centre("osc1_volume"));
            go(&mut e, p);
        }
        e.update_scene();
        assert_eq!(
            e.scene.tip.as_ref().map(|t| t.0.as_str()),
            Some("OSCILLATOR-1 VOLUME: 8.00")
        );
    }

    #[test]
    fn every_control_operates_a_parameter() {
        let p = Ca72Params::default();
        for c in CONTROLS.iter() {
            assert!(param(&p, c.param).is_some(), "{}", c.param);
        }
    }

    #[test]
    fn the_editor_keeps_the_panels_proportions() {
        // The panel's 529 and the strip's 48 under it (the presets' selector in its row).
        assert_eq!(height_for(1720), 577);
        assert_eq!(clamp_width(10), MIN_WIDTH);
        assert_eq!(clamp_width(99_999), MAX_WIDTH);
    }

    #[test]
    fn it_opens_at_four_fifths_of_the_screen_until_resized() {
        // A 14-inch MacBook Pro's screen less the menu bar; a 34-inch ultrawide's.
        assert_eq!(fitted_width(Some((1512.0, 949.0))), 1210);
        assert_eq!(fitted_width(Some((3440.0, 1400.0))), 2752);
        // Short and wide: the height's share sets it (the panel and the strip).
        assert_eq!(fitted_width(Some((3840.0, 800.0))), 1908);
        assert_eq!(height_for(1908), 640);
        assert_eq!(fitted_width(None), DEFAULT_WIDTH);
        assert_eq!(
            Ca72Params::default().editor_width.load(Ordering::Relaxed),
            0
        );
    }

    /// X11's screen spans every monitor: the first size comes from the monitor the pointer
    /// is on (two 2560 by 1440 side by side made the editor 3435 wide on one, its grip off
    /// it), less the panels the work area leaves out.
    #[test]
    fn on_x11_it_opens_to_fit_the_monitor_it_is_on() {
        use screen::{Rect, monitor_area};
        let r = |x, y, w, h| Rect { x, y, w, h };
        let whole = r(0, 0, 5120, 1440);
        let (left, right) = (r(0, 0, 2560, 1440), r(2560, 0, 2560, 1440));
        let monitors = [(left, true), (right, false)];
        assert_eq!(fitted_width(Some((5120.0, 1440.0))), 3435);
        assert_eq!(
            monitor_area(whole, &monitors, Some((3000, 700)), None),
            right
        );
        assert_eq!(fitted_width(Some((2560.0, 1440.0))), 2048);
        // The pointer elsewhere: the primary; no RandR: the whole screen, as before.
        assert_eq!(monitor_area(whole, &monitors, None, None), left);
        assert_eq!(monitor_area(whole, &[], Some((3000, 700)), None), whole);
        // A panel along the screen's top; a work area that misses the monitor is ignored.
        let work = Some(r(0, 32, 5120, 1408));
        assert_eq!(
            monitor_area(whole, &monitors, Some((3000, 700)), work),
            r(2560, 32, 2560, 1408)
        );
        assert_eq!(
            monitor_area(whole, &monitors, Some((3000, 700)), Some(left)),
            right
        );
    }

    /// On Windows and Linux the window takes the host's scale or none, never the system's:
    /// the host frames it at nih-plug's size times its own scale (1 when it gave none).
    #[test]
    fn the_window_scales_as_the_host_says() {
        if cfg!(target_os = "macos") {
            assert_eq!(scale_policy(None), WindowScalePolicy::SystemScaleFactor);
        } else {
            assert_eq!(scale_policy(None), WindowScalePolicy::ScaleFactor(1.0));
            assert_eq!(scale_policy(Some(1.5)), WindowScalePolicy::ScaleFactor(1.5));
        }
    }

    /// The size the host asks before it opens the window again: the drawer shut, though the
    /// window closed with it open below (it reopened a drawer's height too tall, blank).
    #[test]
    fn the_editor_reopens_with_the_drawer_shut() {
        let params = Arc::new(Ca72Params::default());
        params.editor_width.store(1720, Ordering::Relaxed);
        let editor = Ca72Editor::new(Arc::clone(&params), Arc::new(Meters::default()));
        let shut = (1720, height_for(1720));
        editor.grown.store(true, Ordering::Relaxed);
        assert_eq!(editor.size(), (1720, shut.1 + drawer_height(1720)));
        drop(editor.closing());
        assert_eq!(editor.size(), shut);
        editor.grown.store(true, Ordering::Relaxed);
        assert_eq!(editor.opening(), shut);
    }

    /// A point of the strip under the panel (its units) in the window.
    fn on_strip(e: &Editing, (x, y): (f64, f64)) -> Point {
        at(e, (x, art::H + y))
    }

    /// Where a strip target is, found by scanning its middle row.
    fn strip_at(target: StripTarget) -> (f64, f64) {
        let y = STRIP_H / 2.0;
        let xs: Vec<f64> = (0..3438)
            .map(f64::from)
            .filter(|&x| strip::hit(x, y) == Some(target))
            .collect();
        assert!(!xs.is_empty(), "{target:?} not found");
        (xs[xs.len() / 2], y)
    }

    /// POLY, VOICES, ENTROPY and SPREAD from the strip under the panel: each its own
    /// parameter, a click one gesture, a slider's drag one gesture; an amount's switch turns
    /// it off and back on at its amount.
    #[test]
    fn the_strip_operates_poly_voices_entropy_and_spread() {
        let (mut e, host, params) = editing();
        let p = on_strip(&e, strip_at(StripTarget::Poly));
        click(&mut e, p);
        let poly = params.poly.as_ptr();
        assert_eq!(
            host.take(),
            vec![Call::Begin(poly), Call::Set(poly, 1.0), Call::End(poly)]
        );
        let voices = params.voices.as_ptr();
        let p = on_strip(&e, strip_at(StripTarget::More));
        click(&mut e, p);
        // (4 to 5 of 2..10.)
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(voices),
                Call::Set(voices, 0.375),
                Call::End(voices)
            ]
        );
        let p = on_strip(&e, strip_at(StripTarget::Fewer));
        e.mouse(MouseEvent::CursorMoved {
            position: p,
            modifiers: Modifiers::empty(),
        });
        e.mouse(MouseEvent::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: -1.0 },
            modifiers: Modifiers::empty(),
        });
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(voices),
                Call::Set(voices, 0.125),
                Call::End(voices)
            ]
        );
        // ENTROPY's slider: pressed at its middle, dragged to its end, one gesture.
        let entropy = params.entropy.as_ptr();
        let (x, y) = strip_at(StripTarget::Slider(Amount::Entropy));
        let (from, to) = (on_strip(&e, (x, y)), on_strip(&e, (x + 2000.0, y)));
        go(&mut e, from);
        e.mouse(MouseEvent::ButtonPressed {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        go(&mut e, to);
        e.mouse(MouseEvent::ButtonReleased {
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        let calls = host.take();
        assert_eq!(calls.first(), Some(&Call::Begin(entropy)));
        assert_eq!(
            &calls[calls.len() - 2..],
            &[Call::Set(entropy, 1.0), Call::End(entropy)]
        );
        assert!(
            calls.iter().all(|c| match c {
                Call::Begin(p) | Call::Set(p, _) | Call::End(p) => *p == entropy,
            }),
            "{calls:?}"
        );
        // SPREAD's switch: on from 0 at half way (it never was), off again keeping it.
        let spread = params.spread.as_ptr();
        let p = on_strip(&e, strip_at(StripTarget::Switch(Amount::Spread)));
        click(&mut e, p);
        assert_eq!(
            host.take(),
            vec![
                Call::Begin(spread),
                Call::Set(spread, 0.5),
                Call::End(spread)
            ]
        );
        // The strip draws what the parameters hold.
        e.update_scene();
        assert!(e.draw());
    }

    #[test]
    fn the_grip_saves_the_width_it_was_given() {
        let (mut e, _host, params) = editing();
        assert!(e.resize(1500));
        assert_eq!(
            (e.width, params.editor_width.load(Ordering::Relaxed)),
            (1500, 1500)
        );
        assert!(!e.resize(1500));
        // Every frame then drawn at that size (on macOS the window applies it itself: its
        // baseview reports no resize of its own making).
        e.fit(1.0, 1500, 1500);
        e.draw();
        let widths = [e.renderer.frame().width(), e.strip.frame().width()];
        assert_eq!(widths, [1500; 2]);
        let tall: u32 = [e.renderer.frame(), e.strip.frame()]
            .iter()
            .map(|f| f.height())
            .sum();
        assert!(
            tall.abs_diff(height_for(1500)) <= 2,
            "{tall} for {}",
            height_for(1500)
        );
    }

    /// The grip's drag asks the host for a size once a frame at most, the last the pointer
    /// took it to (each resize renders the whole panel again).
    #[test]
    fn the_grip_resizes_once_a_frame() {
        let (mut e, host, params) = editing();
        let grip = at(&e, (art::W - 27.0, art::H - 27.0));
        go(&mut e, grip);
        press(&mut e);
        assert!(e.resizing.is_some());
        for dx in 1..=20 {
            let p = Point {
                x: grip.x + f64::from(dx) * 5.0,
                y: grip.y,
            };
            go(&mut e, p);
        }
        assert_eq!(
            host.resizes.load(Ordering::Relaxed),
            0,
            "none between frames"
        );
        e.grip();
        assert_eq!(host.resizes.load(Ordering::Relaxed), 1);
        assert_eq!(
            (e.width, params.editor_width.load(Ordering::Relaxed)),
            (1820, 1820)
        );
        e.grip();
        assert_eq!(
            host.resizes.load(Ordering::Relaxed),
            1,
            "nothing moved since"
        );
        release(&mut e);
    }

    // ---- The presets (decisions.md R10).

    /// A point of the presets' bar (its units) in the window.
    fn on_bar(e: &Editing, (x, y): (f64, f64)) -> Point {
        at(e, (x, art::H + y))
    }

    /// A point of the drawer (its units) in the window, the drawer open.
    fn in_drawer(e: &Editing, (x, y): (f64, f64)) -> Point {
        let (top, _, _) = e.drawer_span().expect("the drawer open");
        at(e, (x, top + y))
    }

    fn key(e: &mut Editing, k: Key) -> bool {
        let setter = ParamSetter::new(e.context.as_ref());
        e.browser.key(
            &KeyboardEvent {
                state: KeyState::Down,
                key: k,
                ..KeyboardEvent::default()
            },
            &e.params,
            &setter,
        )
    }

    fn typed(e: &mut Editing, s: &str) {
        for c in s.chars() {
            key(e, Key::Character(c.to_string()));
        }
    }

    /// The drawer, opened from the bar's name and slid open.
    fn open(e: &mut Editing) {
        click(e, on_bar(e, presets_ui::bar_centre(BarTarget::Name)));
        assert!(e.browser.open);
        std::thread::sleep(crate::presets::SLIDE + Duration::from_millis(20));
        e.draw();
        assert_eq!(e.browser.reveal(), 1.0);
    }

    /// The drawer's slide over, and drawn.
    fn slid(e: &mut Editing) {
        std::thread::sleep(crate::presets::SLIDE + Duration::from_millis(20));
        e.draw();
    }

    fn row_of(e: &Editing, name: &str) -> usize {
        e.browser
            .drawer
            .rows
            .iter()
            .position(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} not listed"))
    }

    fn preset(p: &Ca72Params) -> String {
        p.preset.read().unwrap().clone()
    }

    /// A trackpad's or a Magic Mouse's scrolling (a few points an event) and a wheel turned
    /// slowly (fractions of a line) scroll the list a row for each row's travel; a turn the
    /// other way starts again (the owner, 2026-10-02: neither the wheel nor touch scrolled it).
    #[test]
    fn small_scrolls_add_up_to_rows() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, _host, _params) = editing_with(Library::at(dir.path()));
        e.draw();
        open(&mut e);
        let mid = in_drawer(&e, presets_ui::field_centre(FieldId::SaveName, None));
        go(
            &mut e,
            Point {
                x: mid.x,
                y: mid.y - 120.0,
            },
        );
        let wheel = |e: &mut Editing, delta: ScrollDelta| {
            e.mouse(MouseEvent::WheelScrolled {
                delta,
                modifiers: Modifiers::empty(),
            });
        };
        // A row's travel in points at this size, in 4-point events: one row down.
        let row_points = ROW_H * e.renderer.scale() / e.dpr;
        let events = (row_points / 4.0).ceil() as usize;
        assert!(events > 2, "{row_points} points a row");
        for _ in 0..events {
            wheel(&mut e, ScrollDelta::Pixels { x: 0.0, y: -4.0 });
        }
        assert_eq!(e.browser.drawer.first, 1, "a row's travel of a trackpad");
        // A wheel turned slowly: three tenths of a line at a time.
        for _ in 0..4 {
            wheel(&mut e, ScrollDelta::Lines { x: 0.0, y: -0.3 });
        }
        assert_eq!(e.browser.drawer.first, 2, "1.2 lines of a slow wheel");
        // Back up: the 0.2 left over is dropped, so one line up is one row.
        wheel(&mut e, ScrollDelta::Lines { x: 0.0, y: 1.0 });
        assert_eq!(e.browser.drawer.first, 1);
    }

    #[test]
    fn the_bar_opens_the_drawer_below_it_and_a_row_sets_the_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, params) = editing_with(Library::at(dir.path()));
        e.draw();
        let shut = e.window_height();
        assert_eq!(shut, height_for(1720));
        open(&mut e);
        // Below the bar, the window grown for it (the host asked), the panel untouched.
        assert!(e.below() && e.resizing_window);
        assert_eq!(e.window_height(), shut + drawer_height(1720));
        assert!(e.composed.is_none());
        let (top, _, _) = e.drawer_span().unwrap();
        assert_eq!(top, art::H + STRIP_H);
        assert!(e.browser.bar.below && e.browser.bar.open);
        assert_eq!(e.browser.drawer.rows.len(), crate::library::factory().len());
        // A row sets every parameter it names (or its default), the preset remembered.
        host.take();
        let i = row_of(&e, "Undertow Growl");
        // Below the rows shown: the wheel scrolls the list to it.
        let mid = in_drawer(&e, presets_ui::field_centre(FieldId::SaveName, None));
        let list = Point {
            x: mid.x,
            y: mid.y - 120.0,
        };
        go(&mut e, list);
        while presets_ui::row_centre(&e.browser.drawer, i).is_none() {
            let first = e.browser.drawer.first;
            e.mouse(MouseEvent::WheelScrolled {
                delta: ScrollDelta::Lines { x: 0.0, y: -1.0 },
                modifiers: Modifiers::empty(),
            });
            assert_eq!(
                e.browser.drawer.first,
                first + 1,
                "the wheel scrolls the list"
            );
        }
        let (x, y) = presets_ui::row_centre(&e.browser.drawer, i).expect("shown");
        let pt = in_drawer(&e, (x, y));
        click(&mut e, pt);
        assert_eq!(preset(&params), "Undertow Growl");
        let calls = host.take();
        let set = |p: ParamPtr| {
            calls
                .iter()
                .any(|c| matches!(c, Call::Set(q, _) if *q == p))
        };
        assert!(
            set(params.osc2_on.as_ptr())
                && set(params.cutoff.as_ptr())
                && set(params.mod_wheel.as_ptr())
        );
        assert!(!set(params.pitch_wheel.as_ptr()) && !set(params.bypass.as_ptr()));
        // Each a gesture of its own.
        let begins = calls.iter().filter(|c| matches!(c, Call::Begin(_))).count();
        let ends = calls.iter().filter(|c| matches!(c, Call::End(_))).count();
        assert_eq!(begins, ends);
        e.draw();
        assert_eq!(e.browser.bar.name, "Undertow Growl");
        assert!(e.browser.drawer.rows[i].current);
        // A double click on a row chooses it and closes the drawer, the window shrinking.
        e.browser.scroll(-100);
        let j = row_of(&e, "Brass Tutti");
        let pt = in_drawer(
            &e,
            presets_ui::row_centre(&e.browser.drawer, j).expect("shown"),
        );
        click(&mut e, pt);
        click(&mut e, pt);
        assert_eq!(preset(&params), "Brass Tutti");
        assert!(!e.browser.open);
        slid(&mut e);
        assert!(!e.below());
        assert_eq!(e.window_height(), shut);
        // Escape closes it too.
        open(&mut e);
        assert!(key(&mut e, Key::Escape));
        slid(&mut e);
        assert!(!e.browser.open && !e.below());
        assert!(
            !key(&mut e, Key::Character("a".into())),
            "closed: the host's keys"
        );
    }

    /// The editor's width, fitted to the screen as it opens, stays while the drawer opens
    /// and shuts (only its height changes), and is measured again when it next opens.
    #[test]
    fn the_width_stays_while_the_drawer_opens_and_shuts() {
        let ed = Ca72Editor::new(Arc::new(Ca72Params::default()), Arc::new(Meters::default()));
        let (w, h) = ed.opening();
        // A screen that would measure otherwise now: the width the editor opened at stays.
        ed.fitted.store(w, Ordering::Relaxed);
        ed.grown.store(true, Ordering::Relaxed);
        assert_eq!(ed.size(), (w, h + drawer_height(w)));
        ed.grown.store(false, Ordering::Relaxed);
        assert_eq!(ed.size(), (w, h));
        drop(ed.closing());
        assert_eq!(
            ed.fitted.load(Ordering::Relaxed),
            0,
            "measured again next time"
        );
    }

    /// Shut below the strip, the drawer slides back up under it as it came out: the window
    /// keeps its height, and the drawer is not drawn over the panel, until it has; then the
    /// window shrinks (decisions.md R19).
    #[test]
    fn the_drawer_shuts_the_way_it_opened() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, _params) = editing_with(Library::at(dir.path()));
        e.draw();
        let shut = e.window_height();
        open(&mut e);
        assert!(e.below());
        let tall = e.window_height();
        let asked = host.resizes.load(Ordering::Relaxed);
        assert!(key(&mut e, Key::Escape));
        e.follow_drawer();
        // Held at the slide's start (begun an hour hence: however long a draw takes here).
        e.browser
            .slide_began(Instant::now() + Duration::from_secs(3600));
        e.draw();
        assert!(!e.browser.open && e.browser.sliding());
        assert!(
            e.below(),
            "the window keeps its height while the drawer slides"
        );
        assert_eq!(e.window_height(), tall);
        assert!(e.slid.is_some(), "the drawer sliding below the strip");
        assert!(e.composed.is_none(), "not over the panel");
        assert_eq!(host.resizes.load(Ordering::Relaxed), asked);
        // Its end past.
        e.browser
            .slide_began(Instant::now() - crate::presets::SLIDE * 2);
        e.draw();
        assert!(!e.below());
        assert_eq!(e.window_height(), shut);
        assert_eq!(host.resizes.load(Ordering::Relaxed), asked + 1);
    }

    /// A host that will not resize the window: the drawer opens over the panel instead.
    #[test]
    fn where_the_window_cannot_grow_the_drawer_opens_over_the_panel() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, _params) = editing_with(Library::at(dir.path()));
        host.fixed.store(true, Ordering::Relaxed);
        e.draw();
        open(&mut e);
        assert!(!e.below());
        assert_eq!(e.window_height(), height_for(1720));
        assert!(!e.browser.bar.below);
        let c = e.composed.as_ref().expect("the drawer over the panel");
        let frame = e.renderer.frame();
        let row = |p: &ca72_panel::Pixmap, y: u32| {
            let w = p.width() as usize;
            p.data()[(y as usize * w) * 4..((y as usize + 1) * w) * 4].to_vec()
        };
        let low = (frame.height() as f64 * 0.9) as u32;
        let high = (frame.height() as f64 * 0.1) as u32;
        assert_ne!(row(c, low), row(frame, low));
        assert_eq!(row(c, high), row(frame, high));
        // A row there sets the plug-in too: one on the list's first page, not the current
        // preset (which one depends on how many presets sort before it).
        let i = e.browser.drawer.first + 1;
        let name = e.browser.drawer.rows[i].name.clone();
        assert!(!e.browser.drawer.rows[i].current);
        let pt = in_drawer(
            &e,
            presets_ui::row_centre(&e.browser.drawer, i).expect("shown"),
        );
        click(&mut e, pt);
        e.draw();
        assert_eq!(e.browser.bar.name, name);
    }

    /// On a short, wide screen the window with the drawer below would run off its foot (at
    /// 3440 by 1400 the editor opens 2752 wide: 923 pixels tall, 1600 with the drawer): the
    /// drawer opens over the panel, the host not asked.
    #[test]
    fn where_the_window_would_not_fit_the_drawer_opens_over_the_panel() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, _params) = editing_with(Library::at(dir.path()));
        e.screen = |_| Some((3440.0, 1400.0));
        let w = fitted_width(Some((3440.0, 1400.0)));
        assert_eq!(
            (w, height_for(w), height_for(w) + drawer_height(w)),
            (2752, 923, 1600)
        );
        e.fit(1.0, w, w);
        e.draw();
        open(&mut e);
        assert!(!e.below() && !e.browser.bar.below);
        assert_eq!(host.resizes.load(Ordering::Relaxed), 0);
        assert!(e.composed.is_some(), "the drawer over the panel");
        // Shut and opened again where it fits: below the strip.
        assert!(key(&mut e, Key::Escape));
        e.follow_drawer();
        assert_eq!(host.resizes.load(Ordering::Relaxed), 0);
        e.screen = |_| Some((3440.0, 1600.0));
        open(&mut e);
        assert!(e.below());
        assert_eq!(e.window_height(), 1600);
    }

    #[test]
    fn typing_searches_and_the_arrows_step_through_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, _host, params) = editing_with(Library::at(dir.path()));
        open(&mut e);
        typed(&mut e, "GLIDE");
        let names: Vec<&str> = e
            .browser
            .drawer
            .rows
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert!(names.contains(&"Lead") && names.contains(&"Hollow Glider"));
        assert!(!names.contains(&"Bass"), "{names:?}");
        key(&mut e, Key::Backspace);
        assert_eq!(e.browser.drawer.search.text, "GLID");
        key(&mut e, Key::ArrowDown);
        let first = e.browser.drawer.rows[0].name.clone();
        assert_eq!(preset(&params), first);
        key(&mut e, Key::ArrowDown);
        assert_eq!(preset(&params), e.browser.drawer.rows[1].name);
        key(&mut e, Key::ArrowUp);
        assert_eq!(preset(&params), first);
        // The host's shortcuts stay the host's.
        let setter = ParamSetter::new(e.context.as_ref());
        assert!(!e.browser.key(
            &KeyboardEvent {
                state: KeyState::Down,
                key: Key::Character("s".into()),
                modifiers: Modifiers::CONTROL,
                ..KeyboardEvent::default()
            },
            &e.params,
            &setter,
        ));
        // A tag chip filters; the bar's arrows step too.
        key(&mut e, Key::Escape);
        open(&mut e);
        let bass = e
            .browser
            .drawer
            .chips
            .iter()
            .position(|(t, _)| t == "whistle")
            .expect("a chip");
        let setter = ParamSetter::new(e.context.as_ref());
        e.browser
            .drawer_press(DrawerTarget::Chip(bass), 0.0, &e.drawer, &e.params, &setter);
        let names: Vec<&str> = e
            .browser
            .drawer
            .rows
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(names, ["Cruising Whistle"]);
        let pt = on_bar(&e, presets_ui::bar_centre(BarTarget::Next));
        click(&mut e, pt);
        assert_eq!(preset(&params), "Cruising Whistle");
    }

    /// An arrow choosing a preset ends a wheel's gesture still open on a knob it sets.
    #[test]
    fn a_key_choosing_a_preset_ends_a_gesture_first() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, _params) = editing_with(Library::at(dir.path()));
        e.draw();
        open(&mut e);
        let p = at(&e, centre("cutoff"));
        go(&mut e, p);
        wheel(&mut e, 1.0);
        assert!(e.keyed(&KeyboardEvent {
            state: KeyState::Down,
            key: Key::ArrowDown,
            ..KeyboardEvent::default()
        }));
        let calls = host.take();
        well_formed(&calls);
        assert!(calls.iter().filter(|c| matches!(c, Call::Begin(_))).count() > 2);
    }

    #[test]
    fn saved_renamed_tagged_deleted_and_restored_from_the_drawer() {
        let dir = tempfile::tempdir().unwrap();
        let (mut e, _host, params) = editing_with(Library::at(dir.path()));
        // SAVE… opens the drawer at the name; Tab to the tags; Enter saves.
        e.draw();
        let pt = on_bar(&e, presets_ui::bar_centre(BarTarget::Save));
        click(&mut e, pt);
        assert_eq!(e.browser.drawer.focus, Some(FieldId::SaveName));
        std::thread::sleep(crate::presets::SLIDE + Duration::from_millis(20));
        e.draw();
        typed(&mut e, "My Bass");
        key(&mut e, Key::Tab);
        typed(&mut e, "bass, test");
        key(&mut e, Key::Enter);
        assert!(dir.path().join("My Bass.toml").exists());
        assert_eq!(preset(&params), "My Bass");
        let lib = Library::at(dir.path());
        let mine = lib.find("My Bass").unwrap();
        assert_eq!(mine.sound.tags, ["bass", "test"]);
        // Its values: every parameter but the PITCH wheel and the bypass, plain.
        let ids: Vec<&str> = mine.sound.values.iter().map(|(k, _)| k.as_str()).collect();
        assert!(ids.contains(&"cutoff") && ids.contains(&"osc1_range") && ids.contains(&"voices"));
        assert!(!ids.contains(&"pitch_wheel") && !ids.contains(&"bypass"));
        assert!(mine.sound.values.contains(&("osc1_range".to_owned(), 3.0)));
        // Saving as a factory preset's name says it replaces it.
        let setter = ParamSetter::new(e.context.as_ref());
        e.browser.drawer_press(
            DrawerTarget::Field(FieldId::SaveName),
            0.0,
            &e.drawer,
            &e.params,
            &setter,
        );
        typed(&mut e, "Bass");
        assert!(e.browser.drawer.replace);
        assert!(e.browser.drawer.hint.contains("factory"));
        // Renamed in place (the plug-in keeps it), its tags edited.
        let i = row_of(&e, "My Bass");
        let (x, y) =
            presets_ui::action_centre(&e.browser.drawer, i, RowAction::Rename).expect("shown");
        let pt = in_drawer(&e, (x, y));
        click(&mut e, pt);
        assert_eq!(e.browser.drawer.focus, Some(FieldId::Edit));
        for _ in 0.."My Bass".len() {
            key(&mut e, Key::Backspace);
        }
        typed(&mut e, "Your Bass");
        key(&mut e, Key::Enter);
        assert!(lib.find("Your Bass").is_some() && lib.find("My Bass").is_none());
        assert_eq!(preset(&params), "Your Bass");
        let i = row_of(&e, "Your Bass");
        let (x, y) =
            presets_ui::action_centre(&e.browser.drawer, i, RowAction::Tags).expect("shown");
        let pt = in_drawer(&e, (x, y));
        click(&mut e, pt);
        key(&mut e, Key::End);
        typed(&mut e, ", deep");
        key(&mut e, Key::Enter);
        assert_eq!(
            lib.find("Your Bass").unwrap().sound.tags,
            ["bass", "test", "deep"]
        );
        // Deleted after asking.
        let i = row_of(&e, "Your Bass");
        let (x, y) =
            presets_ui::action_centre(&e.browser.drawer, i, RowAction::Delete).expect("shown");
        let pt = in_drawer(&e, (x, y));
        click(&mut e, pt);
        assert!(lib.find("Your Bass").is_some(), "deleted before asked");
        let (x, y) =
            presets_ui::action_centre(&e.browser.drawer, i, RowAction::Confirm).expect("asked");
        let pt = in_drawer(&e, (x, y));
        click(&mut e, pt);
        assert!(lib.find("Your Bass").is_none());
        e.draw();
        assert!(!e.browser.bar.found, "the bar says the preset is gone");
        // A factory preset deleted, then restored; the star makes a favourite.
        let i = row_of(&e, "Lead");
        let setter = ParamSetter::new(e.context.as_ref());
        for a in [RowAction::Delete, RowAction::Confirm] {
            e.browser.drawer_press(
                DrawerTarget::Action(i, a),
                0.0,
                &e.drawer,
                &e.params,
                &setter,
            );
        }
        assert!(e.browser.drawer.rows.iter().all(|r| r.name != "Lead"));
        e.browser
            .drawer_press(DrawerTarget::Restore, 0.0, &e.drawer, &e.params, &setter);
        let i = row_of(&e, "Lead");
        e.browser
            .drawer_press(DrawerTarget::Star(i), 0.0, &e.drawer, &e.params, &setter);
        assert!(lib.find("Lead").unwrap().favorite);
    }

    #[test]
    fn a_frame_is_shown_again_on_linux_only() {
        let now = Instant::now();
        assert!(repaint_due(None, now), "the first frame is always shown");
        assert!(!repaint_due(Some(now), now));
        let later = now + Duration::from_secs(1);
        assert_eq!(repaint_due(Some(now), later), cfg!(target_os = "linux"));
    }

    /// The window as it opens (the panel and the strip under it), written to
    /// `$CA72_EDITOR_PNG` for a look: the README's `docs/panel.png`.
    #[test]
    #[ignore = "writes an image for a look"]
    fn the_window_png() {
        let Some(out) = std::env::var_os("CA72_EDITOR_PNG") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let (mut e, _host, _params) = editing_with(Library::at(dir.path()));
        e.draw();
        let frames = [e.renderer.frame(), e.strip.frame()];
        let w = frames[0].width();
        let h: u32 = frames.iter().map(|f| f.height()).sum();
        let mut all = ca72_panel::Pixmap::new(w, h).unwrap();
        let mut y = 0usize;
        for f in frames {
            let n = (f.width() * f.height() * 4) as usize;
            let start = y * w as usize * 4;
            all.data_mut()[start..start + n].copy_from_slice(f.data());
            y += f.height() as usize;
        }
        all.save_png(out).unwrap();
    }

    /// The drawer's update check (decisions.md R27): its button the editor's, not the
    /// presets'; a check that could not be made offers the releases' page. No parameter is
    /// touched and the drawer stays open.
    #[test]
    fn the_drawer_checks_for_updates() {
        use crate::update::{RELEASES, VERSION, fake};
        let dir = tempfile::tempdir().unwrap();
        let (mut e, host, _params) = editing_with(Library::at(dir.path()));
        e.update = Update::with(fake::no_curl, fake::browser);
        e.draw();
        open(&mut e);
        let s = &e.browser.drawer.update;
        assert_eq!(
            (s.text.as_str(), s.button.as_str()),
            (format!("CA-72 {VERSION}").as_str(), "CHECK FOR UPDATES")
        );
        let button = in_drawer(&e, presets_ui::update_centre());
        go(&mut e, button);
        assert_eq!(e.browser.drawer.hover, Some(DrawerTarget::Update));
        click(&mut e, button);
        e.draw();
        assert_eq!(e.browser.drawer.update.text, "COULD NOT CHECK");
        click(&mut e, button);
        assert_eq!(fake::opened(), vec![RELEASES.to_owned()]);
        assert!(e.browser.open);
        assert_eq!(host.take(), vec![]);
    }

    /// The window's frames stacked (the panel with the drawer over it, the strip, the bar),
    /// written to `$CA72_EDITOR_PNG` for a look.
    #[test]
    #[ignore = "writes an image for a look"]
    fn the_window_with_the_drawer_open_png() {
        let Some(out) = std::env::var_os("CA72_EDITOR_PNG") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let (mut e, _host, _params) = editing_with(Library::at(dir.path()));
        e.draw();
        open(&mut e);
        let i = row_of(&e, "Brass Tutti");
        let pt = in_drawer(
            &e,
            presets_ui::row_centre(&e.browser.drawer, i).expect("shown"),
        );
        click(&mut e, pt);
        let pt = in_drawer(
            &e,
            presets_ui::row_centre(&e.browser.drawer, i + 1).expect("shown"),
        );
        go(&mut e, pt);
        e.draw();
        let frames = [e.renderer.frame(), e.strip.frame(), e.drawer.frame()];
        let w = frames[0].width();
        let h: u32 = frames.iter().map(|f| f.height()).sum();
        let mut all = ca72_panel::Pixmap::new(w, h).unwrap();
        let mut y = 0usize;
        for f in frames {
            let n = (f.width() * f.height() * 4) as usize;
            let start = y * w as usize * 4;
            all.data_mut()[start..start + n].copy_from_slice(f.data());
            y += f.height() as usize;
        }
        all.save_png(out).unwrap();
    }
}

/// The editor in a real window on Windows, in a host that resizes windows (decisions.md R26).
/// Opening the presets' drawer, shutting it and dragging the grip ask the host for another size
/// from within the editor's event handler. The host may resize the editor's window right there,
/// from within `IPlugFrame::resizeView()` (here `resizes_editor`), or the editor's own resize may
/// run while it still handles the press (the drawer taking the keyboard sent its window messages
/// from within the handler). Either way baseview gave the busy handler a `WM_SIZE`: a panic inside
/// its window procedure, which aborted the host's process (0.1.0, in Sonar). These tests aborted
/// the test process then. The windows are the test's own, off the screen, so the pointer is never
/// over them; the presets come from an empty folder of the test's.
#[cfg(all(test, target_os = "windows"))]
#[allow(unsafe_code)]
mod windows_window_tests {
    use std::path::PathBuf;
    use std::ptr::null_mut;

    use ca72_panel::presets::{self as presets_ui, BarTarget};
    use nih_plug::context::PluginApi;
    use nih_plug::editor::ParentWindowHandle;
    use nih_plug::wrapper::state::PluginState;
    use winapi::shared::minwindef::{LPARAM, WPARAM};
    use winapi::shared::windef::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, HWND, RECT};
    use winapi::um::winuser::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GW_CHILD, GetClientRect,
        GetWindow, MK_LBUTTON, MSG, PM_REMOVE, PeekMessageW, PostMessageW, RegisterClassW,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetThreadDpiAwarenessContext, SetWindowPos,
        TranslateMessage, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WNDCLASSW, WS_CLIPCHILDREN,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    use super::*;

    /// A host that grows its own window (the editor's parent) to the size the editor asks, and
    /// with `resizes_editor` the editor's window as well, from within the request.
    struct WindowHost {
        editor: Arc<Ca72Editor>,
        parent: usize,
        resizes_editor: bool,
        resizes: AtomicU32,
    }

    impl GuiContext for WindowHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Vst3
        }
        fn request_resize(&self) -> bool {
            self.resizes.fetch_add(1, Ordering::Relaxed);
            let (w, h) = self.editor.size();
            let (w, h) = (w as i32, h as i32);
            let parent = self.parent as HWND;
            let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
            // SAFETY: the test's own windows, on the thread that made them (the editor asks from
            // its event handler).
            unsafe {
                SetWindowPos(parent, null_mut(), 0, 0, w, h, flags);
                if self.resizes_editor {
                    SetWindowPos(GetWindow(parent, GW_CHILD), null_mut(), 0, 0, w, h, flags);
                }
            }
            true
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            panic!("the editor does not read the state")
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// The editor's window open in the host's, at the narrowest width and a pixel a point.
    struct Opened {
        parent: HWND,
        editor: Arc<Ca72Editor>,
        host: Arc<WindowHost>,
        handle: baseview::WindowHandle,
        _presets: tempfile::TempDir,
    }

    impl Opened {
        fn new(resizes_editor: bool) -> Self {
            // The windows in pixels whatever the screen's scale (baseview sets the process's
            // awareness only once its first window is open, and the tests share the process).
            // SAFETY: this thread's own setting.
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
            let presets = tempfile::tempdir().expect("a temporary folder");
            let params = Arc::new(Ca72Params::default());
            params.editor_width.store(MIN_WIDTH, Ordering::Relaxed);
            let meters = Arc::new(Meters::default());
            let editor = Arc::new(Ca72Editor::new(Arc::clone(&params), Arc::clone(&meters)));
            let (w, h) = editor.size();
            let parent = host_window(w, h);
            let host = Arc::new(WindowHost {
                editor: Arc::clone(&editor),
                parent: parent as usize,
                resizes_editor,
                resizes: AtomicU32::new(0),
            });
            let context: Arc<dyn GuiContext> = host.clone();
            let grown = Arc::clone(&editor.grown);
            let dir: PathBuf = presets.path().to_path_buf();
            let handle = Window::open_parented(
                &window::Parent(ParentWindowHandle::Win32Hwnd(parent.cast())),
                WindowOpenOptions {
                    title: String::from("CA-72"),
                    size: Size::new(f64::from(w), f64::from(h)),
                    scale: WindowScalePolicy::ScaleFactor(1.0),
                },
                move |window: &mut Window<'_>| {
                    PanelWindow::new(
                        window,
                        context,
                        params,
                        meters,
                        grown,
                        w,
                        1.0,
                        Library::at(dir),
                    )
                },
            );
            pump(Duration::from_millis(300));
            Opened {
                parent,
                editor,
                host,
                handle,
                _presets: presets,
            }
        }

        /// The editor's window.
        fn window(&self) -> HWND {
            // SAFETY: the test's own window.
            unsafe { GetWindow(self.parent, GW_CHILD) }
        }

        /// Its size, in pixels (logical ones: its scale is 1).
        fn size(&self) -> (u32, u32) {
            let mut r = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            // SAFETY: the test's own window and a rectangle of this function's.
            unsafe { GetClientRect(self.window(), &mut r) };
            ((r.right - r.left) as u32, (r.bottom - r.top) as u32)
        }

        /// Drawing units to the window's pixels.
        fn at(&self, (x, y): (f64, f64)) -> (f64, f64) {
            let k = f64::from(self.size().0) / art::W;
            (x * k, y * k)
        }

        /// The pointer moved to `at` (pixels), pressed, moved `dx` across in steps, released.
        fn drag(&self, at: (f64, f64), dx: f64) {
            let lp = |(x, y): (f64, f64)| {
                (((y.round() as u32) << 16) | (x.round() as u32 & 0xFFFF)) as LPARAM
            };
            let w = self.window();
            // SAFETY: messages posted to the test's own window.
            unsafe {
                PostMessageW(w, WM_MOUSEMOVE, 0, lp(at));
                PostMessageW(w, WM_LBUTTONDOWN, MK_LBUTTON as WPARAM, lp(at));
            }
            pump(Duration::from_millis(50));
            for i in 1..=4 {
                let to = (at.0 + dx * f64::from(i) / 4.0, at.1);
                // SAFETY: as above.
                unsafe { PostMessageW(w, WM_MOUSEMOVE, MK_LBUTTON as WPARAM, lp(to)) };
                pump(Duration::from_millis(50));
            }
            // SAFETY: as above.
            unsafe { PostMessageW(w, WM_LBUTTONUP, 0, lp((at.0 + dx, at.1))) };
            pump(Duration::from_millis(400));
        }

        /// A click on the preset's name, in the strip under the panel.
        fn click_name(&self) {
            let (x, y) = presets_ui::bar_centre(BarTarget::Name);
            self.drag(self.at((x, art::H + y)), 0.0);
        }

        fn close(mut self) {
            self.handle.close();
            pump(Duration::from_millis(100));
            // SAFETY: the test's own window.
            unsafe { DestroyWindow(self.parent) };
        }
    }

    /// The host's window, `w` by `h`, off the screen, with no button on the taskbar and not
    /// brought to the front, so that it does not take the developer's pointer or keys.
    fn host_window(w: u32, h: u32) -> HWND {
        let class: Vec<u16> = "CA72TestHost\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: null_mut(),
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
            lpszClassName: class.as_ptr(),
        };
        // SAFETY: a window class of the test's (registered by the first test to get here; the
        // others' calls fail harmlessly) and a window of it, on this thread.
        let hwnd = unsafe {
            RegisterClassW(&wc);
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
                -20000,
                -20000,
                w as i32,
                h as i32,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        assert!(!hwnd.is_null(), "a window for the test");
        hwnd
    }

    /// This thread's messages, for `d`.
    fn pump(d: Duration) {
        let until = Instant::now() + d;
        // SAFETY: a message structure of this function's, for this thread's messages.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while Instant::now() < until {
            // SAFETY: as above.
            while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
                // SAFETY: a message just taken from this thread's queue.
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The editor's window's size once it is `want`, or after 5 s, its messages handled meanwhile.
    fn settled(o: &Opened, want: (u32, u32)) -> (u32, u32) {
        let until = Instant::now() + Duration::from_secs(5);
        while o.size() != want && Instant::now() < until {
            pump(Duration::from_millis(20));
        }
        o.size()
    }

    fn the_drawer_opens_and_shuts(resizes_editor: bool) {
        let o = Opened::new(resizes_editor);
        let short = (MIN_WIDTH, height_for(MIN_WIDTH));
        assert_eq!(settled(&o, short), short);
        o.click_name();
        let tall = (MIN_WIDTH, height_for(MIN_WIDTH) + drawer_height(MIN_WIDTH));
        assert_eq!(
            settled(&o, tall),
            tall,
            "the window grown for the drawer below the strip"
        );
        assert!(
            o.host.resizes.load(Ordering::Relaxed) >= 1,
            "the host asked"
        );
        o.click_name();
        assert_eq!(
            settled(&o, short),
            short,
            "the window back once the drawer has shut"
        );
        o.close();
    }

    /// The host grows its own window, the editor its own (the drawer took the keyboard as the
    /// press was handled: its window's messages ran the resize early).
    #[test]
    fn the_drawer_opens_and_shuts_where_the_host_grows_its_own_window() {
        the_drawer_opens_and_shuts(false);
    }

    /// The host resizes the editor's window as well, from within the editor's request.
    #[test]
    fn the_drawer_opens_and_shuts_where_the_host_resizes_the_editors_window() {
        the_drawer_opens_and_shuts(true);
    }

    /// The grip asks for its width at the editor's next frame; the host resizes the editor's
    /// window from within that.
    #[test]
    fn the_grip_resizes_where_the_host_resizes_the_editors_window() {
        let o = Opened::new(true);
        let grip = o.at((
            (art::GRIP[0] + art::GRIP[2]) / 2.0,
            (art::GRIP[1] + art::GRIP[3]) / 2.0,
        ));
        o.drag(grip, 120.0);
        assert!(
            o.host.resizes.load(Ordering::Relaxed) >= 1,
            "the host asked"
        );
        let asked = o.editor.size();
        assert_eq!(
            settled(&o, asked),
            asked,
            "the window as the editor last asked"
        );
        o.close();
    }
}
