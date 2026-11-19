//! The [`Renderer`] draws frames on the screen.
//!
//! To do that, it needs two things.
//!
//! The first is a connection to the GPU. The renderer opens it once, when it is created, and keeps
//! it for as long as the renderer exists. A [`GpuContext`] stores it.
//!
//! The second is a surface. A surface is the part of a window that the GPU draws into. Unlike the
//! connection, a surface can be lost and replaced while the program runs. A
//! [`SurfaceContext`](crate::context::SurfaceContext) stores it.
//!
//! Both are defined in the `context` module. This module defines the renderer, which draws each
//! frame with them.

use glam::{Mat4, Quat, Vec4};
use hermes::{MeshData, SkinData};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use wgpu::{
    BindGroup, Buffer, Color, CommandEncoderDescriptor, CurrentSurfaceTexture,
    DisplayAndWindowHandle, IndexFormat, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline, StoreOp,
    TextureViewDescriptor,
};

use crate::Camera;
use crate::RendererError;
use crate::context::GpuContext;
use crate::mesh::{Mesh, SkinnedVertex, Vertex};
use crate::object::RenderObject;
use crate::skeleton::Skeleton;
use crate::uniform::{ObjectUniforms, SceneUniforms};

/// A window that a [`Renderer`] can draw into.
///
/// A type is a `RenderWindow` once it meets three bounds: it provides the display and window
/// handles that wgpu needs to create a surface, it implements `Debug`, and it is `'static`. The
/// last bound means that the type does not contain a borrowed reference that could expire. The
/// renderer needs that, since it keeps the window for as long as the renderer exists.
///
/// Every type that meets the three bounds implements `RenderWindow` automatically. A winit `Window`
/// is one such type.
///
/// # Examples
///
/// The example writes a function that accepts any window that the renderer can draw into.
///
/// ```ignore
/// // Creates a renderer at 1280 by 720 pixels, for any window that meets the three bounds
/// async fn create_renderer<W: RenderWindow>(
///     window: Arc<W>,
///     hand: &Model,
/// ) -> Result<Renderer, RendererError> {
///     Renderer::new(window, 1280, 720, &hand.mesh, &hand.skin).await
/// }
/// ```
pub trait RenderWindow: DisplayAndWindowHandle + Debug + 'static {}

impl<W> RenderWindow for W where W: DisplayAndWindowHandle + Debug + 'static {}

/// Draws frames into one window.
///
/// A caller creates a `Renderer` with `new` once the window exists. From then on, the caller calls
/// `render` each time the window needs redrawing. It also passes each new window size to `resize`.
///
/// Two more methods serve a platform that can take the window away from a running app, such as a
/// phone. The caller calls `suspend` when the app loses its window, and `resume` when it gets a
/// window back. winit never takes the window away on Windows, meaning a winit app there never
/// calls them.
///
/// # Examples
///
/// The example creates a renderer, then hands it the two window events that concern it.
///
/// ```ignore
/// // Creates the renderer once the window exists, from the hand's mesh and skin
/// let size = window.inner_size();
/// let renderer_setup =
///     Renderer::new(Arc::clone(&window), size.width, size.height, &hand.mesh, &hand.skin);
/// let mut renderer = block_on(renderer_setup)?;
///
/// // Passes each new window size to `resize`, and calls `render` with the bones' rotations each
/// // time the window needs redrawing
/// match event {
///     WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
///     WindowEvent::RedrawRequested => {
///         let bone_rotations = simulation.find_bone_rotations();
///         renderer.render(&camera, hand_transform, bone_rotations)?
///     }
///     _ => {}
/// }
/// ```
pub struct Renderer {
    gpu: GpuContext,
    hand: RenderObject,
    skeleton: Skeleton,          // builds the hand's joint matrices from its bone rotations
    joint_buffer: Buffer,        // the hand's joint matrices, in GPU memory
    joint_bind_group: BindGroup, // hands `joint_buffer` to the shader, at group 2
    scene_buffer: Buffer,        // the camera's matrix and the light's direction
    scene_bind_group: BindGroup, // hands `scene_buffer` to the shader, at group 0
    ground_pipeline: RenderPipeline, // draws the ground, with the checker
    hand_pipeline: RenderPipeline, // draws the hand, in one colour
    ground: RenderObject,        // the ground's buffers and bind group
}

impl Renderer {
    /// The dark blue that fills each frame before the renderer draws the ground and the hand.
    const CLEAR_COLOUR: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    // downwards, and a little to the left and away from the camera
    const LIGHT_DIRECTION: Vec4 = Vec4::new(-0.3, -1.0, -0.5, 0.0);

    const HAND_COLOUR: Vec4 = Vec4::new(0.85, 0.64, 0.52, 1.0);
    // The colour of the ground's light squares, as RGBA from 0 to 1
    const GROUND_COLOUR: Vec4 = Vec4::new(0.36, 0.55, 0.3, 1.0);
    // How far the ground reaches from the origin in each direction along x and z
    const GROUND_HALF_SIZE: f32 = 50.0;

    /// Creates a renderer that draws the hand into `window`, at `width` by `height` pixels.
    ///
    /// `hand_mesh` and `hand_skin` are the hand's shape and its skin, as `hermes` reads them.
    /// `new` first connects to a GPU that can draw to the window. It then builds a square of
    /// ground, copies its mesh and the hand's skinned mesh into GPU memory, creates the buffers
    /// that the shader reads the camera and the light from, and compiles the shader. It also
    /// builds the hand's skeleton, and a buffer for the joint matrices that `render` fills each
    /// frame.
    ///
    /// Last, it builds two pipelines. A pipeline is the set of settings that the GPU follows to
    /// draw a mesh. The ground's pipeline draws the ground with a checker of light and dark
    /// squares. The hand's pipeline bends the hand by its bones, and draws it in one colour.
    ///
    /// # Errors
    ///
    /// Returns a `RendererError` in each of these cases:
    ///
    /// - wgpu cannot create a surface for the window.
    /// - No GPU can draw to that surface.
    /// - The GPU refuses to open a device.
    /// - The GPU does not offer a pixel format or a present mode for the surface.
    /// - The GPU cannot anti-alias in the surface's pixel format. Anti-aliasing smooths the jagged
    ///   edges of a shape.
    ///
    /// # Examples
    ///
    /// The example creates a renderer for a winit window, at the window's current size.
    ///
    /// ```ignore
    /// // Creates a renderer that draws the hand into the window at its current size
    /// let size = window.inner_size();
    /// let renderer_setup =
    ///     Renderer::new(Arc::clone(&window), size.width, size.height, &hand.mesh, &hand.skin);
    ///
    /// // Waits for the setup to finish, since `new` returns a future
    /// let renderer = block_on(renderer_setup)?;
    /// ```
    pub async fn new<W>(
        window: Arc<W>,
        width: u32,
        height: u32,
        hand_mesh: &MeshData,
        hand_skin: &SkinData,
    ) -> Result<Self, RendererError>
    where
        W: RenderWindow,
    {
        // Connects to the GPU, and creates the window's surface
        let gpu = GpuContext::new(window, width, height).await?;
        // Builds the hand's skinned vertices from its mesh and its skin
        let hand_mesh = Mesh::from_skinned_data(hand_mesh, hand_skin);

        let scene_buffer = gpu.create_uniform_buffer("Scene uniforms", size_of::<SceneUniforms>());
        let scene_layout = gpu.create_uniform_layout("Scene layout");
        let object_layout = gpu.create_uniform_layout("Object layout");

        let hand = gpu.create_render_object("Hand", &hand_mesh, &object_layout);
        // Converts the bones once, for every frame to come
        let skeleton = Skeleton::from_bones(&hand_skin.bones);
        // Creates a storage buffer with room for one matrix per bone, and hands it to the shader
        // at group 2
        let joint_size = skeleton.count_bones() * size_of::<Mat4>();
        let joint_buffer = gpu.create_storage_buffer("Joint matrices", joint_size);
        let joint_layout = gpu.create_storage_layout("Joint layout");
        let joint_bind_group =
            gpu.create_bind_group("Joint bind group", &joint_layout, &joint_buffer);
        // Builds the square of ground, and creates its buffers and bind group
        let ground_mesh = Mesh::build_ground(Self::GROUND_HALF_SIZE);
        let ground = gpu.create_render_object("Ground", &ground_mesh, &object_layout);
        let scene_bind_group =
            gpu.create_bind_group("Scene bind group", &scene_layout, &scene_buffer);

        let shader = gpu.create_shader("Lit shader", include_str!("../shaders/lit.wgsl"));
        // Builds the ground's pipeline: plain vertices, the checker, and groups 0 and 1
        let ground_pipeline = gpu.create_pipeline::<Vertex>(
            "Ground pipeline",
            &shader,
            ("vs_static", "fs_ground"),
            &[&scene_layout, &object_layout],
        );
        // Builds the hand's pipeline: skinned vertices, one colour, and groups 0, 1 and 2
        let hand_pipeline = gpu.create_pipeline::<SkinnedVertex>(
            "Hand pipeline",
            &shader,
            ("vs_skinned", "fs_lit"),
            &[&scene_layout, &object_layout, &joint_layout],
        );
        Ok(Self {
            gpu,
            scene_buffer,
            hand,
            skeleton,
            joint_buffer,
            joint_bind_group,
            scene_bind_group,
            ground_pipeline,
            hand_pipeline,
            ground,
        })
    }

    /// Changes the size at which the renderer draws each frame, to `width` by `height` pixels.
    ///
    /// A width or height of zero leaves the size as it is.
    ///
    /// # Examples
    ///
    /// A minimised window typically reports a size of zero, which the renderer ignores.
    ///
    /// ```ignore
    /// renderer.resize(1920, 1080); // draws each later frame at 1920 by 1080
    /// renderer.resize(0, 0);       // leaves the size at 1920 by 1080
    /// ```
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    /// Drops the surface, for a caller whose app has lost its window.
    ///
    /// From then on, `render` skips every frame. It draws again once `resume` gives the renderer a
    /// new surface.
    ///
    /// # Examples
    ///
    /// After `suspend`, each call to `render` skips the frame.
    ///
    /// ```ignore
    /// // Drops the surface when the app loses its window
    /// renderer.suspend();
    ///
    /// // Skips the frame and returns `Ok(())`
    /// renderer.render(&camera, hand_transform, bone_rotations)?;
    /// ```
    pub fn suspend(&mut self) {
        self.gpu.suspend();
    }

    /// Creates a new surface for `window`, at `width` by `height` pixels.
    ///
    /// A caller calls `resume` when the app gets its window back. `render` draws into the new
    /// surface from then on.
    ///
    /// # Errors
    ///
    /// Returns a `RendererError` in each of these cases:
    ///
    /// - wgpu cannot create the surface.
    /// - The GPU cannot draw to the surface in the pixel format that the pipelines were built for.
    ///
    /// # Examples
    ///
    /// When the app gets its window back, the caller gives the renderer a new surface, then asks
    /// for a frame.
    ///
    /// ```ignore
    /// // Creates a new surface for the window, at the window's current size
    /// let size = window.inner_size();
    /// renderer.resume(Arc::clone(&window), size.width, size.height)?;
    ///
    /// // Asks winit for a frame, which `render` draws into the new surface
    /// window.request_redraw();
    /// ```
    pub fn resume<W>(
        &mut self,
        window: Arc<W>,
        width: u32,
        height: u32,
    ) -> Result<(), RendererError>
    where
        W: RenderWindow,
    {
        self.gpu.resume(window, width, height)
    }

    /// Draws one frame and shows it in the window.
    ///
    /// `camera` sets the view, and `hand_transform` places the hand in the world.
    /// `bone_rotations` gives each bone of the hand its rotation for this frame, in bone order, and
    /// MUST hold one rotation for each bone.
    ///
    /// # Panics
    ///
    /// Panics if `bone_rotations` does not hold exactly one rotation for each bone.
    ///
    /// Some calls skip the frame. While the app is suspended, no surface exists, and `render`
    /// returns at once.
    ///
    /// Otherwise, each frame is drawn into a texture. If no texture comes back, `render` skips the frame,
    /// before which, it reconfigures an outdated surface or replaces a lost one with a new
    /// surface for the same window.
    ///
    /// # Errors
    ///
    /// Returns `RendererError::DeviceLost` once the connection to the GPU has died. Every later
    /// call returns it too, since nothing made from the dead connection works again. A call made
    /// while the app is suspended is the exception, and returns `Ok(())` as before.
    ///
    /// Returns `RendererError::Validation` where wgpu rejects a call as invalid while handing out
    /// the frame's texture. That points to a bug in the renderer.
    ///
    /// Returns a `RendererError` after wgpu reports the surface lost, in each of these cases:
    ///
    /// - wgpu cannot create the new surface.
    /// - The GPU cannot draw to the new surface in the pixel format that the pipelines were built
    ///   for.
    ///
    /// # Examples
    ///
    /// A caller draws a frame each time winit asks for one. The example ends the program where
    /// `render` returns an error.
    ///
    /// ```ignore
    /// // Draws one frame and shows it in the window
    /// if let Err(error) = renderer.render(&camera, hand_transform, bone_rotations) {
    ///     // Ends the program, since each error points to a dead connection, a lost surface that
    ///     // could not be replaced, or a bug
    ///     eprintln!("Error: {error}");
    ///     event_loop.exit();
    /// }
    /// ```
    pub fn render(
        &mut self,
        camera: &Camera,
        hand_transform: Mat4,
        bone_rotations: &[Quat],
    ) -> Result<(), RendererError> {
        // Returns at once while the app is suspended, since no surface exists then
        let Some(surface_context) = &self.gpu.surface_context else {
            return Ok(());
        };
        // Returns an error once the connection to the GPU has died. The check comes before the
        // texture request, since a dead connection also reports the surface lost. Without it,
        // `recreate` would build a new surface on the dead connection
        if self.gpu.device_lost.load(Ordering::Relaxed) {
            return Err(RendererError::DeviceLost);
        }

        // Gets the next texture from the surface, and skips the frame where none comes back.
        // Before skipping, reconfigures an outdated surface and replaces a lost one
        let (frame, suboptimal) = match surface_context.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => (frame, false),
            CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            // Return branches:
            CurrentSurfaceTexture::Outdated => {
                self.gpu.reconfigure();
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => return self.gpu.recreate(),
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(()),
            // Returns an error rather than skipping the frame, since a skip would hide the bug
            CurrentSurfaceTexture::Validation => return Err(RendererError::Validation),
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        let config = &surface_context.config;
        let aspect_ratio = config.width as f32 / config.height as f32;
        let scene = SceneUniforms {
            view_projection: camera.build_view_projection(aspect_ratio),
            light_direction: Self::LIGHT_DIRECTION,
        };

        self.gpu
            .queue
            .write_buffer(&self.scene_buffer, 0, bytemuck::bytes_of(&scene));

        let hand_uniforms = ObjectUniforms {
            model: hand_transform,
            colour: Self::HAND_COLOUR,
        };

        // Copies the hand's matrix and colour into its buffer
        self.hand.write_uniforms(&self.gpu.queue, &hand_uniforms);
        // Builds one joint matrix per bone from this frame's rotations, and copies them into the
        // joint buffer
        let joint_matrices = self.skeleton.build_joint_matrices(bone_rotations);
        self.gpu
            .queue
            .write_buffer(&self.joint_buffer, 0, bytemuck::cast_slice(&joint_matrices));

        // Leaves the ground where its mesh puts it, and gives it the ground colour
        let ground_uniforms = ObjectUniforms {
            model: Mat4::IDENTITY,
            colour: Self::GROUND_COLOUR,
        };
        // Copies the ground's matrix and colour into its buffer
        self.ground.write_uniforms(&self.gpu.queue, &ground_uniforms);

        // Creates the command encoder, which records this frame's commands for the GPU to run
        let encoder_descriptor = CommandEncoderDescriptor {
            label: Some("Sarx frame"),
        };
        let mut encoder = self.gpu.device.create_command_encoder(&encoder_descriptor);

        // Points the pass at the multisampled texture, which stores several samples for each
        // pixel. The pass clears that texture and draws into it. It then averages each pixel's
        // samples into the surface texture, thereby smoothing the hand's edges
        let colour_attachment = RenderPassColorAttachment {
            view: &surface_context.msaa_view,
            depth_slice: None,
            resolve_target: Some(&view),
            ops: Operations {
                load: LoadOp::Clear(Self::CLEAR_COLOUR),
                // Discards the samples once the pass has averaged them, skipping a write to memory
                store: StoreOp::Discard,
            },
        };

        // Points the pass at the depth buffer, and clears every depth to 1.0, the far edge of the view
        let depth_attachment = RenderPassDepthStencilAttachment {
            view: &surface_context.depth_view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                // Discards the depths once the pass ends, since nothing reads them afterwards
                store: StoreOp::Discard,
            }),
            stencil_ops: None,
        };
        let pass_descriptor = RenderPassDescriptor {
            label: Some("Scene"),
            color_attachments: &[Some(colour_attachment)],
            depth_stencil_attachment: Some(depth_attachment),
            ..Default::default()
        };

        let mut render_pass = encoder.begin_render_pass(&pass_descriptor);
        // Points group 0 at the camera and the light, which both pipelines share
        render_pass.set_bind_group(0, &self.scene_bind_group, &[]);

        // Draws the ground through the checker
        render_pass.set_pipeline(&self.ground_pipeline);
        self.ground.draw(&mut render_pass);
        // Draws the hand in its own colour, bent by its bones through the joint matrices at group 2
        render_pass.set_pipeline(&self.hand_pipeline);
        render_pass.set_bind_group(2, &self.joint_bind_group, &[]);
        self.hand.draw(&mut render_pass);
        drop(render_pass);

        // Sends the recorded commands to the GPU, then shows the frame's texture in the window
        let queue = &self.gpu.queue;
        queue.submit([encoder.finish()]);
        queue.present(frame);

        // Reconfigures a suboptimal surface, whose textures no longer match the window. The
        // reconfigure comes after `present` has taken the frame, since `configure` panics while a
        // texture from the surface is still alive
        if suboptimal {
            self.gpu.reconfigure();
        }
        Ok(())
    }
}
