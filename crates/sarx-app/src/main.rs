mod app;
mod executor;

use winit::event_loop::EventLoop;

use app::App;

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create the event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("The event loop failed");
}
