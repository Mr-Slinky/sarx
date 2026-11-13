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
//! Both live in the `gpu_context` module. This module holds the renderer, which draws each frame
//! with them.

use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use wgpu::{
    Buffer, Color, CommandEncoderDescriptor, CurrentSurfaceTexture, DisplayAndWindowHandle, LoadOp,
    Operations, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, StoreOp,
    TextureViewDescriptor,
};

use crate::RendererError;
use crate::context::GpuContext;
use crate::mesh::Mesh;

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
/// async fn create_renderer<W: RenderWindow>(window: Arc<W>) -> Result<Renderer, RendererError> {
///     Renderer::new(window, 1280, 720).await
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
/// // Creates the renderer once the window exists
/// let size = window.inner_size();
/// let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height);
/// let mut renderer = block_on(renderer_setup)?;
///
/// // Passes each new window size to `resize`, and calls `render` each time the window needs
/// // redrawing
/// match event {
///     WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
///     WindowEvent::RedrawRequested => renderer.render()?,
///     _ => {}
/// }
/// ```
pub struct Renderer {
    /// The connection to the GPU. It also stores the surface for the window.
    gpu: GpuContext,
    /// The vertices of the triangle, copied into GPU memory.
    vertex_buffer: Buffer,
    /// The number of vertices in `vertex_buffer`. `render` passes it to each draw call. `new`
    /// copies it from the mesh before the mesh is dropped.
    vertex_count: u32,
    /// The pipeline that draws the triangle. A pipeline is the set of settings that the GPU follows
    /// to draw, from reading each vertex to writing each pixel's colour.
    pipeline: RenderPipeline,
}

impl Renderer {
    /// The dark blue that fills each frame before the renderer draws the triangle.
    const CLEAR_COLOUR: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    /// Creates a renderer that draws into `window`, at `width` by `height` pixels.
    ///
    /// `new` first connects to a GPU that can draw to the window. It then copies the triangle into
    /// GPU memory and compiles the shader. Last, it builds the pipeline, which is the set of
    /// settings that the GPU follows to draw the triangle.
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
    /// // Creates a renderer that draws into the window at its current size
    /// let size = window.inner_size();
    /// let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height);
    ///
    /// // Waits for the setup to finish, since `new` returns a future
    /// let renderer = block_on(renderer_setup)?;
    /// ```
    pub async fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, RendererError>
    where
        W: RenderWindow,
    {
        let gpu = GpuContext::new(window, width, height).await?;
        let mesh = Mesh::build_triangle();
        let vertex_buffer = gpu.create_vertex_buffer(&mesh);
        let shader_source = include_str!("../shaders/flat_colour.wgsl");
        let shader = gpu.create_shader("Flat colour shader", shader_source);
        let pipeline = gpu.create_pipeline(&shader);
        Ok(Self {
            gpu,
            vertex_buffer,
            vertex_count: mesh.vertex_count(),
            pipeline,
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
    /// renderer.render()?;
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
    /// - The GPU cannot draw to the surface in the pixel format that the pipeline was built for.
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
    /// Each frame shows an orange triangle over a dark blue background.
    ///
    /// Some calls skip the frame. While the app is suspended, no surface exists, and `render`
    /// returns at once.
    ///
    /// Otherwise, each frame is drawn into a texture, which is an image in GPU memory that the
    /// surface hands out for that frame. Where no texture comes back, `render` skips the frame.
    /// Before skipping, it reconfigures an outdated surface and replaces a lost one with a new
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
    /// - The GPU cannot draw to the new surface in the pixel format that the pipeline was built
    ///   for.
    ///
    /// # Examples
    ///
    /// A caller draws a frame each time winit asks for one. The example ends the program where
    /// `render` returns an error.
    ///
    /// ```ignore
    /// // Draws one frame and shows it in the window
    /// if let Err(error) = renderer.render() {
    ///     // Ends the program, since each error points to a dead connection, a lost surface that
    ///     // could not be replaced, or a bug
    ///     eprintln!("Error: {error}");
    ///     event_loop.exit();
    /// }
    /// ```
    pub fn render(&mut self) -> Result<(), RendererError> {
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
            CurrentSurfaceTexture::Outdated => {
                self.gpu.reconfigure();
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => return self.gpu.recreate(),
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(()),
            // Returns an error rather than skipping the frame, since a skip would hide the bug
            CurrentSurfaceTexture::Validation => return Err(RendererError::Validation),
        };
        // Creates a view of the frame's texture, which the pass resolves the multisampled texture
        // into
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        // Creates the command encoder, which records this frame's commands for the GPU to run
        let encoder_descriptor = CommandEncoderDescriptor {
            label: Some("Sarx frame"),
        };
        let mut encoder = self.gpu.device.create_command_encoder(&encoder_descriptor);

        // Points the pass at the multisampled texture, which stores several samples for each
        // pixel. The pass clears that texture and draws into it. It then averages each pixel's
        // samples into the surface texture, thereby smoothing the triangle's edges
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
        let pass_descriptor = RenderPassDescriptor {
            label: Some("Triangle"),
            color_attachments: &[Some(colour_attachment)],
            ..Default::default()
        };
        // Records a render pass, which is one batch of draws into the colour attachment. This pass
        // draws the triangle
        let mut render_pass = encoder.begin_render_pass(&pass_descriptor);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        // Runs the pipeline over every vertex in the buffer, drawing one copy of the mesh
        render_pass.draw(0..self.vertex_count, 0..1);
        // Ends the pass, which releases its borrow of the encoder. `finish` can then take the
        // encoder
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
