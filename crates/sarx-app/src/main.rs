//! Opens the window and runs the event loop.
//!
//! An event loop waits for events from the operating system, such as a resize or a key press. It
//! hands each event to the program as it arrives.
//!
//! `main` creates winit's event loop and hands it an [`App`]. winit then calls a method on the
//! `App` for each event. The first call, to `resumed`, creates the window. The player closing the
//! window ends the program.

mod app;
mod executor;

use winit::event_loop::EventLoop;

use app::App;

/// Creates the event loop and hands it an `App`.
///
/// winit then calls the `App` for every event, until the program ends.
///
/// # Panics
///
/// Panics where winit cannot create the event loop, or where the event loop fails while it runs.
fn main() {
    let event_loop = EventLoop::new().expect("Failed to create the event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("The event loop failed");
}
