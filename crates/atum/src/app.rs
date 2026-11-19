//! Handles the events that winit reports for the window.
//!
//! winit is the library that opens the window. For each event, such as a resize or the player
//! closing the window, winit calls a method on [`App`].
//!
//! The `App` owns the window, together with the [`Renderer`] that draws into it. Each method passes
//! its event on to the renderer, draws a frame, stores the cursor's position, prints the event, or
//! ends the program. Where the renderer fails, the method prints the error before it ends the
//! program.
//!
//! The example on [`App`] hands it to winit's event loop.

use crate::executor::block_on;
use crate::input;
use amaterasu::{Camera, Renderer};
use glam::{Mat4, Quat, Vec2, Vec3};
use hermes::Model;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;
use winit::window::{Fullscreen, Window, WindowId};
use zurvan::{HandPoses, Pose, Simulation};

/// Owns the window and the renderer, and defines the methods that winit calls for each event.
///
/// [`App::new`] reads the hand model, then builds the camera and the simulation from it. It leaves
/// the window and the renderer as `None`, and the first call to `resumed` then creates both.
///
/// # Examples
///
/// The example hands a new `App` to winit's event loop, which calls its methods for every event
/// until the program ends.
///
/// ```ignore
/// // Creates the event loop, then hands it a new `App` with no window or renderer yet
/// let event_loop = EventLoop::new()?;
/// let mut app = App::new()?;
/// event_loop.run_app(&mut app)?;
/// ```
pub(crate) struct App {
    window: Option<Arc<Window>>, // the window, or `None` before the first `resumed`
    renderer: Option<Renderer>,  // the renderer, or `None` before the first `resumed`
    simulation: Simulation,      // the world, and the fixed steps that move it
    camera: Camera,              // where the view is drawn from
    cursor: Option<Vec2>, // the cursor's last position in pixels, or `None` before it first moves over the window
    last_frame: Option<Instant>, // when the last frame started, or `None` before the first frame
    hand_model: Model, // the hand's mesh, skin and poses, which the renderer reads once it exists
}

impl App {
    const HAND_MODEL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/hand-model.glb"
    ));
    // The centre of the hand model's palm, relative to the model's origin at the wrist cut-off.
    // Measured from the skeleton, halfway between the wrist joint and the average of the four
    // knuckles
    const PALM_CENTRE: Vec3 = Vec3::new(0.0, -0.03, -1.89);
    // Where the camera stands, tuned so that the whole hand fits on screen
    const CAMERA_EYE: Vec3 = Vec3::new(-0.3, 4.0, 3.0);
    // The point the camera looks at, tuned to the middle of the hand
    const CAMERA_TARGET: Vec3 = Vec3::new(-0.3, 0.0, -2.0);
    // The idle poses, in the order that the hand cycles through them
    const IDLE_POSES: [&str; 4] = ["idle_1", "idle_2", "idle_3", "idle_4"];
    // Each action pose, beside the key that holds it. The order here sets each pose's number in
    // `HandPoses::actions`
    const ACTION_KEYS: [(KeyCode, &str); 4] = [
        (KeyCode::Digit1, "fist"),
        (KeyCode::Digit2, "point"),
        (KeyCode::Digit3, "ok"),
        (KeyCode::Digit4, "thumbs_up"),
    ];

    /// Returns an `App` with the hand model read, the hand's poses handed to a new simulation, and
    /// the camera in place. The window and the renderer stay `None` until the first `resumed`.
    ///
    /// The example on [`App`] shows a call.
    ///
    /// # Errors
    ///
    /// Returns an error where the hand model cannot be read, or where it lacks one of the poses
    /// that `IDLE_POSES` and `ACTION_KEYS` name.
    pub(crate) fn new() -> Result<Self, Box<dyn Error>> {
        let hand_model = hermes::read_model(Self::HAND_MODEL)?;
        let poses = Self::build_hand_poses(&hand_model)?;
        let camera = Camera::new(Self::CAMERA_EYE, Self::CAMERA_TARGET);
        Ok(Self {
            window: None,
            renderer: None,
            cursor: None,
            last_frame: None,
            simulation: Simulation::new(poses),
            camera,
            hand_model,
        })
    }

    /// Returns the hand's poses from `model`: its rest pose, built from each bone's rest rotation,
    /// then the idle poses and the action poses, in the order that `IDLE_POSES` and `ACTION_KEYS`
    /// list them.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first pose that `model` lacks.
    fn build_hand_poses(model: &Model) -> Result<HandPoses, String> {
        // Converts each bone's rest rotation into a glam quaternion
        let rest_rotations = model
            .skin
            .bones
            .iter()
            .map(|bone| Quat::from_array(bone.rotation))
            .collect();
        let mut idle = Vec::with_capacity(Self::IDLE_POSES.len());
        for name in Self::IDLE_POSES {
            // Adds the pose, or fails if the file has no pose of that name
            idle.push(Self::find_pose(model, name)?);
        }
        let mut actions = Vec::with_capacity(Self::ACTION_KEYS.len());
        for (_, name) in Self::ACTION_KEYS {
            // Adds the pose, or fails if the file has no pose of that name
            actions.push(Self::find_pose(model, name)?);
        }

        // Returns the rest pose, the idle poses and the action poses together
        Ok(HandPoses {
            rest: Pose(rest_rotations),
            idle,
            actions,
        })
    }

    /// Returns the pose called `name` in `model`, with one rotation for each bone. A bone that the
    /// pose leaves out takes its rest rotation.
    ///
    /// # Errors
    ///
    /// Returns a message naming the pose where `model` has no pose called `name`.
    fn find_pose(model: &Model, name: &str) -> Result<Pose, String> {
        let Some(pose) = model.poses.iter().find(|pose| pose.name == name) else {
            return Err(format!("the hand model has no pose named '{name}'"));
        };
        let mut rotations = Vec::with_capacity(pose.rotations.len());
        // Pairs each rotation in the pose with its bone
        for (rotation, bone) in pose.rotations.iter().zip(&model.skin.bones) {
            // Takes the pose's rotation for the bone, or the bone's rest rotation if the pose has none
            let array = rotation.unwrap_or(bone.rotation);
            rotations.push(Quat::from_array(array));
        }
        Ok(Pose(rotations))
    }

    /// Advances the simulation by the time since the last frame, draws the frame, and asks winit
    /// for the next one.
    ///
    /// Before it advances the simulation, the method aims the hand at the point under the cursor.
    /// It does nothing until the window and the renderer both exist. Where the renderer fails,
    /// it prints the error and ends the program.
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
        // Aims the hand at the point under the cursor at the hover height, or keeps the last aim
        // if there is no such point
        if let Some(hover_point) = input::find_hand_target(self.cursor, window_size, &self.camera) {
            self.simulation.set_hand_target(hover_point);
        }

        self.simulation.advance(elapsed);
        let hand_position = self.simulation.hand_position();
        // Slides the palm centre onto the model's origin, then moves the hand to its position, so
        // that the palm, rather than the wrist, sits under the cursor
        let hand_transform =
            Mat4::from_translation(hand_position) * Mat4::from_translation(-Self::PALM_CENTRE);

        // Borrows the rotation of every bone after those steps
        let bone_rotations = self.simulation.find_bone_rotations();

        // Draws the frame, and ends the program IF the renderer cannot carry on
        if let Err(error) = renderer.render(&self.camera, hand_transform, bone_rotations) {
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
    /// Every later call gives the renderer a new surface for the window. Where the renderer cannot
    /// be set up, or cannot take its new surface, `resumed` prints the error and ends the program.
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
            let fullscreen = Fullscreen::Borderless(None);
            let attributes = Window::default_attributes()
                .with_title("Sarx")
                .with_fullscreen(Some(fullscreen));
            let window = event_loop // <-- First window instance is created here
                .create_window(attributes)
                .expect("Failed to create the window");
            // window.set_cursor_visible(false);
            let window = Arc::new(window);

            // Creates the renderer for the window from the hand's mesh and skin, which `App`
            // already stores, or ends the program where the setup fails
            let size = window.inner_size();
            let hand = &self.hand_model;
            let renderer_setup = Renderer::new(
                Arc::clone(&window),
                size.width,
                size.height,
                &hand.mesh,
                &hand.skin,
            );
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
