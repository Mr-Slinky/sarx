//! Handles the events that winit reports for the window.
//!
//! winit is the library that opens the window. For each event, such as a resize or the player
//! closing the window, winit calls a method on [`App`].
//!
//! The `App` owns the window, together with the [`Renderer`] that draws into it. Each method passes
//! its event on to the renderer, prints it, or ends the program.

use std::error::Error;
use std::sync::Arc;

use sarx_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::executor::block_on;

/// Owns the window and the renderer, and defines the methods that winit calls for each event.
///
/// `App::default()` sets both fields to `None`. The first call to `resumed` then creates the
/// window and the renderer.
///
/// # Examples
///
/// The example hands a new `App` to winit's event loop. The event loop calls its methods for every
/// event until the program ends.
///
/// ```ignore
/// // Creates the event loop, then hands it a new `App` with both fields set to `None`
/// let event_loop = EventLoop::new()?;
/// let mut app = App::default();
/// event_loop.run_app(&mut app)?;
/// ```
#[derive(Default)]
pub(crate) struct App {
    /// The window, or `None` before the first `resumed`.
    ///
    /// The `Arc` lets the app and the renderer's surface share the window. An `Arc` counts its
    /// owners, and drops the window once the last owner is gone.
    window: Option<Arc<Window>>,
    /// The renderer, or `None` before the first `resumed`. It stays `None` where the setup fails.
    renderer: Option<Renderer>,
}

impl App {
    /// Prints `error` to standard error, followed by the error that caused it, where there is one.
    ///
    /// # Examples
    ///
    /// A device failure prints two lines, the second one for the wgpu error that caused it.
    ///
    /// ```ignore
    /// // Prints "Error: failed to open a device on the GPU", then "Caused by: " and the wgpu
    /// // error on a second line
    /// let error = RendererError::Device(device_error);
    /// App::print_error(&error);
    /// ```
    fn print_error(error: &dyn Error) {
        eprintln!("Error: {error}");
        if let Some(cause) = error.source() {
            eprintln!("Caused by: {cause}");
        }
    }
}

impl ApplicationHandler for App {
    /// Creates the window and the renderer on the first call.
    ///
    /// Every later call gives the renderer a new surface for the window. Where the renderer cannot
    /// be set up, or cannot take its new surface, `resumed` ends the event loop.
    ///
    /// # Panics
    ///
    /// Panics where winit cannot create the window.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Gives the renderer a new surface on every call after the first. winit calls `resumed`
        // again only on a platform that can take the window away from a running app, such as a
        // phone
        if let (Some(window), Some(renderer)) = (&self.window, self.renderer.as_mut()) {
            let size = window.inner_size();
            let window_handle = Arc::clone(window);
            match renderer.resume(window_handle, size.width, size.height) {
                // Asks winit for a first frame, since the new surface likely starts out empty
                Ok(()) => window.request_redraw(),
                Err(error) => {
                    Self::print_error(&error);
                    event_loop.exit();
                }
            }
            return;
        }

        // Creates the window and the renderer on the first call, while no window exists yet
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Sarx");
            let window = event_loop
                .create_window(attributes)
                .expect("Failed to create the window");
            let window = Arc::new(window);

            // Creates the renderer for the window, or ends the program where the setup fails
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

    /// Handles one event for the window.
    ///
    /// When the player closes the window, `window_event` ends the program. It passes each resize
    /// and each redraw on to the renderer. It prints every other event to standard output.
    ///
    /// `render` returns an error when the renderer loses its surface and cannot create a new one.
    /// `window_event` then ends the program too.
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
                if let Some(renderer) = self.renderer.as_mut()
                    && let Err(error) = renderer.render()
                {
                    Self::print_error(&error);
                    event_loop.exit();
                }
            }
            other => println!("{other:?}"),
            // TODO:
            //  WindowEvent::KeyboardInput { event, .. } => input.record_key(event),
            //  _ => {}
            //  etc...
        }
    }

    /// Drops the renderer's surface while the app is suspended.
    ///
    /// winit calls `suspended` only on a platform that can take the window away from a running app,
    /// such as a phone. On Windows, winit never calls it, even for a minimised window.
    fn suspended(&mut self, _: &ActiveEventLoop) {
        // Drops the renderer's surface, since the platform destroys it while the app is suspended
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.suspend();
        }
    }
}
