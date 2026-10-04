//! The plug-in in nih-plug's standalone host: audio and MIDI from the system's devices, the
//! editor in a window of its own.

fn main() {
    nih_plug::nih_export_standalone::<ca72_plugin::Ca72>();
}
