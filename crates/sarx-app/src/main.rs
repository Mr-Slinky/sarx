use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    window: Option<Window>,
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create the event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("The event loop failed");
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Avoids two windows being created, by creating one only on the FIRST call
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Sarx");
            let window = event_loop
                .create_window(attributes)
                .expect("Failed to create the window");
            self.window = Some(window);
        }
        // TODO: create the surface for the window, every time
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            other => println!("{other:?}"),
            // TODO:
            //  WindowEvent::Resized(size) => renderer.resize(size),
            //  WindowEvent::RedrawRequested => renderer.draw(&world),
            //  WindowEvent::KeyboardInput { event, .. } => input.record_key(event),
            //  _ => {}
            //  etc...
        }
    }
}
