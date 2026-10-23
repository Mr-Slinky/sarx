//! Opens Sarx's window and runs the event loop that drives the game.

mod app;
mod executor;

use winit::event_loop::EventLoop;

use app::App;

/// Creates the event loop and hands it an `App`, which winit then calls for every event until
/// the program ends.
fn main() {
    let event_loop = EventLoop::new().expect("Failed to create the event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("The event loop failed");
}
