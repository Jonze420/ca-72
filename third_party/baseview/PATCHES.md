# baseview, patched

The `baseview` crate from [RustAudio/baseview](https://github.com/RustAudio/baseview) at
commit `9a0b42c09d712777b2edb4c5e0cb6baf21e988f0` (MIT OR Apache-2.0, `LICENSE-MIT`,
`LICENSE-APACHE`), the revision the editor took from git until 0.1.0. The examples, the CI
and the manifest's `[workspace]`, `[[example]]` and `[dev-dependencies]` tables are left
out; the manifest names the repository, for the notices, and allows two of upstream's own
compiler warnings (`non_snake_case` in `src/win/drop_target.rs`, and
`mismatched_lifetime_syntaxes`). `src/macos/window.rs` has one `use` list in the order
today's rustfmt gives it, so that `cargo fmt --all` leaves the tree as it is.

One change, in `src/win/window.rs` and `src/win/drop_target.rs` (Windows only; the CA-72's
`docs/decisions.md` R26):

1. **The window procedure never calls the handler while it is busy.** baseview keeps the
   window's handler in a `RefCell` and borrows it for each event. A message can be sent to
   the window from within one of the handler's own calls: a VST3 host may resize the
   editor's window from within `IPlugFrame::resizeView()`, which the editor asks for from its
   event handler; `SetFocus` sends focus messages; a modal loop dispatches the window's
   messages. For any such message `wnd_proc` ran the deferred tasks (a resize the handler
   had asked for: `SetWindowPos`, so a `WM_SIZE`), and `WM_SIZE` borrowed the handler again:
   a panic inside the window procedure, which cannot unwind, so the host's process aborted
   (0xC0000409). The CA-72 0.1.0 did this in Sonar as its presets' drawer opened. Now:
   - an event that arrives while the handler is busy waits in a queue (`pending_events`),
     and a frame then is skipped;
   - the waiting events, then the deferred tasks, run only when the handler is not busy: in
     the `wnd_proc` call whose handler call has just returned (or after a drag and drop
     event). Each in its order; an event waiting is given before a task asked for earlier
     runs (a `Resize` after the `Resized` of a host's `WM_SIZE`), which is harmless;
   - `Window::focus()` is a deferred task, as upstream did in
     [#252](https://github.com/RustAudio/baseview/pull/252) (2026-05-25): "a panic will
     occur when calling `Window.focus` while handling events";
   - `WM_MOUSEMOVE` no longer keeps the pointer's "outside" flag borrowed across the
     handler's call;
   - a drag and drop event that finds the handler busy is dropped and the drop refused;
   - each `wnd_proc` call holds a reference to the window's state until it returns. Closing
     the window (`DestroyWindow` for `BV_WINDOW_MUST_CLOSE`) sends `WM_NCDESTROY` from within
     the call, which freed the state while that call still read it afterwards (upstream
     did, through its deferred tasks), and a host destroying its window while the handler
     was busy freed the handler under its own borrow. `WM_NCDESTROY` also empties both
     queues.

   #252 alone does not do: a host that resizes the editor's window from within
   `resizeView()` still delivered `WM_SIZE` to the busy handler (the CA-72's tests
   `the_drawer_opens_and_shuts_where_the_host_resizes_the_editors_window` and
   `the_grip_resizes_where_the_host_resizes_the_editors_window` abort with #252's change
   only).

Upstream's master has since rewritten the Windows backend and the handler's interface; to
move to it, the editor must be ported, and the tests above show whether it still needs this.
