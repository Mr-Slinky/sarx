use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[derive(Default)]
pub(crate) struct App {
    window: Option<Window>,
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
