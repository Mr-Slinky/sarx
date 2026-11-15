//! Handles the events that winit reports for the window.
//!
//! winit is the library that opens the window. For each event, such as a resize or the player
//! closing the window, winit calls a method on [`App`].
//!
//! The `App` owns the window, together with the [`Renderer`] that draws into it. Each method passes
//! its event on to the renderer, stores the cursor's position, prints the event, or ends the
//! program.

use crate::executor::block_on;
use amaterasu::{Camera, Renderer};
use glam::{Mat4, Vec2, Vec3};
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};
use zurvan::Simulation;

/// Owns the window and the renderer, and defines the methods that winit calls for each event.
///
/// `App::new()` builds the camera and the simulation, and leaves the window and the renderer as
/// `None`. The first call to `resumed` then creates both.
///
/// # Examples
///
/// The example hands a new `App` to winit's event loop, which calls its methods for every event
/// until the program ends.
///
/// ```ignore
/// // Creates the event loop, then hands it a new `App` with no window or renderer yet
/// let event_loop = EventLoop::new()?;
/// let mut app = App::new();
/// event_loop.run_app(&mut app)?;
/// ```
pub(crate) struct App {
    window: Option<Arc<Window>>, // the window, or `None` before the first `resumed`
    renderer: Option<Renderer>,  // the renderer, or `None` before the first `resumed`
    simulation: Simulation,      // the world, and the fixed steps that move it
    camera: Camera,              // where the view is drawn from
    cursor: Option<Vec2>, // the cursor's last position in pixels, or `None` before it first moves over the window
    last_frame: Option<Instant>, // when the last frame started, or `None` before the first frame
}

impl App {
    const HAND_MODEL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/hand-model.glb"
    ));
    // Where the camera stands, tuned so that the whole hand fits on screen
    const CAMERA_EYE: Vec3 = Vec3::new(0.3, 4.0, 3.0);
    // The point the camera looks at, tuned to the middle of the hand
    const CAMERA_TARGET: Vec3 = Vec3::new(0.3, 0.0, -2.0);

    pub(crate) fn new() -> Self {
        let camera = Camera::new(Self::CAMERA_EYE, Self::CAMERA_TARGET);
        Self {
            window: None,
            renderer: None,
            cursor: None,
            last_frame: None,
            simulation: Simulation::new(),
            camera,
        }
    }

    fn draw_frame(&mut self, event_loop: &ActiveEventLoop) {
        // Skips the frame until both the window and the renderer exist
        let (Some(window), Some(renderer)) = (&self.window, self.renderer.as_mut()) else {
            return;
        };

        let now = Instant::now();
        // Measures the time since the last frame, or zero on the first frame
        let elapsed = self.last_frame.map_or(Duration::ZERO, |last| now - last);
        self.last_frame = Some(now);

        let size = window.inner_size();
        // Converts the window size to floats for the camera's maths
        let window_size = Vec2::new(size.width as f32, size.height as f32);
        // Aims the hand at the ground under the cursor. The camera casts a ray through the cursor,
        // and the aim changes only once the cursor has moved over the window and the ray meets
        // the ground
        if let Some(cursor) = self.cursor
            && let Some(ray) = self.camera.cast_ray(cursor, window_size)
            && let Some(ground_point) = ray.meet_ground()
        {
            self.simulation.set_hand_target(ground_point);
        }

        self.simulation.advance(elapsed);
        let hand_position = self.simulation.find_hand_position();
        let hand_transform = Mat4::from_translation(hand_position);

        // Draws the frame, and ends the program where the renderer cannot carry on
        if let Err(error) = renderer.render(&self.camera, hand_transform) {
            Self::print_error(&error);
            event_loop.exit();
            return;
        }
        // Asks winit for the next frame at once, which keeps the simulation and the drawing running
        window.request_redraw();
    }

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
    /// Every later call gives the renderer a new surface for the window. Where the hand model
    /// cannot be read, the renderer cannot be set up, or the renderer cannot take its new surface,
    /// `resumed` prints the error and ends the program.
    ///
    /// # Panics
    ///
    /// Panics where winit cannot create the window.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Gives the renderer a new surface on every call after the first, once the platform hands
        // the window back
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

        // Creates the window and the renderer on the first call, when no window exists yet
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Sarx");
            let window = event_loop
                .create_window(attributes)
                .expect("Failed to create the window");
            let window = Arc::new(window);

            // Reads the hand's mesh, or ends the program where the hand model cannot be read
            let hand = match hermes::read_mesh(Self::HAND_MODEL) {
                Ok(hand) => hand,
                Err(error) => {
                    Self::print_error(&error);
                    event_loop.exit();
                    return;
                }
            };

            // Creates the renderer for the window, or ends the program where the setup fails
            let size = window.inner_size();
            let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height, hand);
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
    /// `window_event` passes each resize on to the renderer, and draws a frame for each redraw. It
    /// stores the cursor's position whenever the cursor moves, ends the program when the player
    /// closes the window, and prints every other event to standard output.
    ///
    /// Where `render` returns an error, such as a lost connection to the GPU or a surface that the
    /// renderer cannot replace, `window_event` prints the error and ends the program too.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            // Draws one frame, and asks for the next
            WindowEvent::RedrawRequested => self.draw_frame(event_loop),
            // Stores where the cursor is, in pixels from the window's top-left corner
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some(Vec2::new(position.x as f32, position.y as f32));
            }
            other => println!("{other:?}"),
            // TODO: the match must pass each `KeyboardInput` to the input system, as in
            //  `WindowEvent::KeyboardInput { event, .. } => input.record_key(event)`, and must
            //  ignore every other event with `_ => {}` in place of the `other` arm
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
