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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
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
                    eprintln!("Error: {error}");
                    if let Some(cause) = error.source() {
                        eprintln!("Caused by: {cause}");
                    }
                    event_loop.exit();
                }
            }
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
