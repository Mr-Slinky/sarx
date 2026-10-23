use std::error::Error;
use std::sync::Arc;

use sarx_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::executor::block_on;

#[derive(Default)]
pub(crate) struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

impl App {
    /// Prints `error` to standard error, followed by the error that caused it, where there is one.
    fn print_error(error: &dyn Error) {
        eprintln!("Error: {error}");
        if let Some(cause) = error.source() {
            eprintln!("Caused by: {cause}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Gives the renderer a new surface for the window on every call after the first, since
        // Android destroys the surface each time the app is suspended
        if let (Some(window), Some(renderer)) = (&self.window, self.renderer.as_mut()) {
            let size = window.inner_size();
            let window_handle = Arc::clone(window);
            match renderer.resume(window_handle, size.width, size.height) {
                // Draws a first frame, since the new surface starts out empty
                Ok(()) => window.request_redraw(),
                Err(error) => {
                    Self::print_error(&error);
                    event_loop.exit();
                }
            }
            return;
        }

        // Avoids two windows being created, by creating one only on the FIRST call
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Sarx");
            let window = event_loop
                .create_window(attributes)
                .expect("Failed to create the window");
            let window = Arc::new(window);

            // Connects the renderer to the window, or ends the program when no GPU can draw to it
            let size = window.inner_size();
            let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height);
            match block_on(renderer_setup) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    Self::print_error(&error);
                    event_loop.exit();
                }
            }
            self.window = Some(window);
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        // Drops the renderer's surface, since Android destroys it while the app is suspended
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.suspend();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.render();
                }
            }
            other => println!("{other:?}"),
            // TODO:
            //  WindowEvent::KeyboardInput { event, .. } => input.record_key(event),
            //  _ => {}
            //  etc...
        }
    }
}
