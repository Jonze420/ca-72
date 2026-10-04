//! The CA-72's front panel, drawn: the instrument's layout and lettering as SVG (in panel
//! units), rendered with resvg into a frame, and what a pointer finds on it. It knows no
//! window and no plugin: the editor shows its frames and turns pointer gestures into
//! parameter changes with it.

pub mod art;
pub mod controls;
pub mod fonts;
pub mod interact;
pub mod presets;
pub mod render;
pub mod strip;
mod svg;

pub use art::{H, Layout, PLATE_NAME, W};
pub use controls::{CONTROLS, Control, Kind};
pub use interact::Target;
pub use render::{Renderer, Scene};
/// The frames' type.
pub use resvg::tiny_skia::Pixmap;
