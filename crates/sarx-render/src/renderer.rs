//! The [`Renderer`] draws frames on the screen.
//!
//! To do that, it needs two things.
//!
//! The first is a connection to the GPU. The renderer opens it once, when it is created, and keeps
//! it for as long as the renderer exists. A [`GpuContext`] stores it.
//!
//! The second is a surface. A surface is the part of a window that the GPU draws into. Unlike the
//! connection, a surface can be lost and replaced while the program runs. A [`SurfaceContext`]
//! stores it.

use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BlendState, Buffer, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, CurrentSurfaceTexture, Device, DeviceDescriptor,
    DisplayAndWindowHandle, Extent3d, FragmentState, Instance, InstanceDescriptor, Limits, LoadOp,
    MultisampleState, Operations, PresentMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModule, ShaderModuleDescriptor, ShaderSource, StoreOp, Surface,
    SurfaceConfiguration, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor, VertexState,
};

use crate::RendererError;
use crate::mesh::Mesh;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Draws frames into one window.
///
/// A caller creates a `Renderer` with `new` once the window exists. From then on, the caller calls
/// `render` each time the window needs redrawing. It also passes each new window size to `resize`.
///
/// Two more methods serve a platform that can take the window away from a running app, such as a
/// phone. The caller calls `suspend` when the app loses its window. It calls `resume` when the app
/// gets a window back. winit never takes the window away on Windows, meaning these two methods run
/// only on another platform.
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
    /// Setting up takes three steps. `new` first connects to a GPU that can draw to the window. It
    /// then copies the triangle into GPU memory. Last, it builds the pipeline, which is the set of
    /// settings that the GPU follows to draw the triangle.
    ///
    /// # Errors
    ///
    /// Returns a `RendererError` in each of these cases:
    ///
    /// - wgpu cannot create a surface for the window.
    /// - No GPU can draw to that surface.
    /// - The GPU refuses to open a device.
    /// - The GPU offers no pixel format or no present mode for the surface.
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
    pub async fn new<W>(window: W, width: u32, height: u32) -> Result<Self, RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle + Debug + Clone + Send + Sync + 'static,
    {
        let gpu = GpuContext::new(window, width, height).await?;
        let mesh = Mesh::triangle();
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
    pub fn resume<W>(&mut self, window: W, width: u32, height: u32) -> Result<(), RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        self.gpu.resume(window, width, height)
    }

    /// Draws one frame and shows it in the window.
    ///
    /// Each frame shows an orange triangle over a dark blue background.
    ///
    /// Some calls skip the frame. While the app is suspended, no surface exists. As a result,
    /// `render` returns at once.
    ///
    /// Each frame is drawn into a texture, which is an image in GPU memory. The surface hands out
    /// one texture for each frame. Where the surface has no texture ready, `render` skips the
    /// frame. Where wgpu reports the surface lost, `render` creates a new surface for the same
    /// window, then skips the frame.
    ///
    /// # Errors
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
    ///     // Ends the program, since the renderer could not replace its lost surface
    ///     eprintln!("Error: {error}");
    ///     event_loop.exit();
    /// }
    /// ```
    pub fn render(&mut self) -> Result<(), RendererError> {
        // Returns at once while the app is suspended, since no surface exists then
        let Some(surface_context) = &self.gpu.surface_context else {
            return Ok(());
        };

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
            CurrentSurfaceTexture::Timeout
            | CurrentSurfaceTexture::Occluded
            | CurrentSurfaceTexture::Validation => return Ok(()),
        };
        // Creates a view of the whole texture, since a render pass draws into a texture through a
        // view
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

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// The connection to one GPU, together with the surface that the GPU draws into.
///
/// The connection lasts as long as the renderer. The surface is dropped while the app is
/// suspended, leaving `surface_context` as `None`.
struct GpuContext {
    /// The surface and the parts built to match it. It is `None` while the app is suspended.
    surface_context: Option<SurfaceContext>,
    /// The wgpu instance, which creates each surface. `new` also uses it to find the adapter.
    instance: Instance,
    /// The GPU that the renderer draws with. wgpu calls a GPU an adapter.
    adapter: Adapter,
    /// The open connection to the adapter. The device creates every buffer, texture, shader and
    /// pipeline.
    device: Device,
    /// The queue that sends recorded commands to the GPU.
    queue: Queue,
    /// The pixel format of the first surface. A pixel format sets how each pixel's colour is laid
    /// out in memory. `create_pipeline` builds the pipeline for this format. Every later surface
    /// therefore takes the same format.
    format: TextureFormat,
}

impl GpuContext {
    /// The number of samples that the GPU takes for each pixel.
    ///
    /// A sample tests whether a shape covers one point inside the pixel. The GPU averages the
    /// samples into the pixel's colour, which smooths the jagged edges of a shape. This smoothing
    /// is called anti-aliasing.
    const SAMPLE_COUNT: u32 = 4;

    // ====================================================================================== \\
    //                                      Constructors                                      \\
    // ====================================================================================== \\
    /// Connects to a GPU that can draw to `window`.
    ///
    /// It also creates the window's surface, and configures it at `width` by `height` pixels.
    ///
    /// # Errors
    ///
    /// Returns a `RendererError` in each of these cases:
    ///
    /// - wgpu cannot create a surface for the window.
    /// - No GPU can draw to that surface.
    /// - The GPU refuses to open a device.
    /// - The GPU offers no pixel format or no present mode for the surface.
    /// - The GPU cannot anti-alias in the surface's pixel format.
    ///
    /// # Examples
    ///
    /// The example connects to a GPU for a window of 1280 by 720 pixels.
    ///
    /// ```ignore
    /// // Connects to a GPU, and configures the window's surface at 1280 by 720 pixels
    /// let gpu = GpuContext::new(window, 1280, 720).await?;
    /// ```
    async fn new<W>(window: W, width: u32, height: u32) -> Result<Self, RendererError>
    where
        W: DisplayAndWindowHandle + Debug + Clone + Send + Sync + 'static,
    {
        // Creates the wgpu instance, and gives it the window's display handle. wgpu uses the
        // display handle only when it draws through OpenGL, where it needs the handle to show
        // frames. DirectX 12 and Vulkan leave it unused. This is a decision, taken to keep a port
        // to an OpenGL device possible
        let display = Box::new(window.clone());
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(display));
        let window = Arc::new(window);
        let surface = instance.create_surface(Arc::clone(&window))?;

        // Chooses a GPU that can draw to the surface
        let adapter_options = RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        };
        let adapter = instance.request_adapter(&adapter_options).await?;

        // Opens the device and its queue, within the WebGPU baseline limits that ADR 0003 settles
        // on
        let device_descriptor = DeviceDescriptor {
            label: Some("Sarx device"),
            required_limits: Limits::default(),
            ..Default::default()
        };
        let (device, queue) = adapter.request_device(&device_descriptor).await?;

        // Gets the surface's default configuration at the window's size
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        // Lists every pixel format that the GPU can draw to the surface in
        let caps = surface.get_capabilities(&adapter);
        // Finds the first sRGB format in that list, or `None` where the list has none
        let srgb_format = caps.formats.iter().copied().find(|format| format.is_srgb());
        // Uses the sRGB format, and keeps the default format where the list has none
        config.format = srgb_format.unwrap_or(config.format);
        // Records the chosen format, which the pipeline and every later surface take
        let format = config.format;

        // Checks that the GPU can anti-alias in the surface's pixel format
        let format_features = adapter.get_texture_format_features(format);
        let flags = format_features.flags;
        if !flags.sample_count_supported(Self::SAMPLE_COUNT) {
            return Err(RendererError::UnsupportedSampleCount);
        }

        // Configures the surface, and creates the multisampled texture that frames draw into
        let surface_context = SurfaceContext::new(window, surface, &device, config);

        Ok(Self {
            surface_context: Some(surface_context),
            instance,
            adapter,
            device,
            queue,
            format,
        })
    }

    // ====================================================================================== \\
    //                                      API Methods                                       \\
    // ====================================================================================== \\
    /// Resizes the surface and the multisampled texture to `width` by `height` pixels.
    ///
    /// While the app is suspended, no surface exists, and `resize` returns at once. A width or
    /// height of zero also leaves both as they are, since `Surface::configure` panics on a size of
    /// zero.
    ///
    /// # Examples
    ///
    /// A minimised window typically reports a size of zero, which `resize` ignores.
    ///
    /// ```ignore
    /// gpu.resize(1920, 1080); // resizes both to 1920 by 1080
    /// gpu.resize(0, 0);       // leaves both as they are, at 1920 by 1080
    /// ```
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(surface_context) = self.surface_context.as_mut() {
            surface_context.resize(&self.device, width, height);
        }
    }

    /// Rebuilds the surface's textures from the current configuration, while a surface exists.
    ///
    /// # Examples
    ///
    /// `render` calls it when wgpu reports the surface outdated, meaning the surface's textures no
    /// longer match the window.
    ///
    /// ```ignore
    /// // Rebuilds the surface's textures, then skips the frame
    /// CurrentSurfaceTexture::Outdated => {
    ///     self.gpu.reconfigure();
    ///     return Ok(());
    /// }
    /// ```
    fn reconfigure(&self) {
        if let Some(surface_context) = &self.surface_context {
            surface_context.reconfigure(&self.device);
        }
    }

    /// Replaces a lost surface with a new one for the same window, at the same size.
    ///
    /// While the app is suspended, no surface exists, and `recreate` returns `Ok(())` at once.
    ///
    /// # Errors
    ///
    /// Returns `RendererError::Surface` where wgpu cannot create the new surface. Returns
    /// `RendererError::UnsupportedSurface` where the GPU cannot draw to the new surface in the
    /// pixel format that the pipeline was built for.
    ///
    /// # Examples
    ///
    /// `render` calls it when wgpu reports the surface lost.
    ///
    /// ```ignore
    /// // Replaces the lost surface with a new one, then skips the frame
    /// CurrentSurfaceTexture::Lost => return self.gpu.recreate(),
    /// ```
    fn recreate(&mut self) -> Result<(), RendererError> {
        let Some(old) = self.surface_context.take() else {
            return Ok(());
        };

        let window = Arc::clone(&old.window);
        let width = old.config.width;
        let height = old.config.height;
        drop(old);

        let surface_context = self.create_surface_context(window, width, height)?;
        self.surface_context = Some(surface_context);
        Ok(())
    }

    /// Drops the surface, its configuration and the multisampled texture.
    ///
    /// # Examples
    ///
    /// The renderer calls it when the app loses its window.
    ///
    /// ```ignore
    /// // Drops the surface, its configuration and the multisampled texture
    /// gpu.suspend();
    /// ```
    fn suspend(&mut self) {
        self.surface_context = None;
    }

    /// Creates a surface for `window` at `width` by `height` pixels.
    ///
    /// The surface takes the pixel format that the pipeline is built for.
    ///
    /// # Errors
    ///
    /// Returns `RendererError::Surface` where wgpu cannot create the surface. Returns
    /// `RendererError::UnsupportedSurface` where the GPU cannot draw to the surface in that pixel
    /// format.
    ///
    /// # Examples
    ///
    /// The renderer calls it when the app gets its window back.
    ///
    /// ```ignore
    /// // Creates a surface for the window at 1280 by 720 pixels
    /// gpu.resume(window, 1280, 720)?;
    /// ```
    fn resume<W>(&mut self, window: W, width: u32, height: u32) -> Result<(), RendererError>
    where
        W: DisplayAndWindowHandle + Send + Sync + 'static,
    {
        // Drops the old surface first, where one exists, since Vulkan can refuse a second surface
        // for a window that already has one
        self.surface_context = None;

        let window: Arc<dyn DisplayAndWindowHandle> = Arc::new(window);

        let surface_context = self.create_surface_context(window, width, height)?;
        self.surface_context = Some(surface_context);
        Ok(())
    }

    /// Copies the vertices of `mesh` into a new vertex buffer, and returns the buffer.
    ///
    /// A buffer is a block of GPU memory. A vertex buffer is a buffer that a draw reads its
    /// vertices from.
    ///
    /// # Examples
    ///
    /// The example copies the triangle into a vertex buffer, then hands the buffer to a render
    /// pass.
    ///
    /// ```ignore
    /// // Copies the vertices of the triangle into a new vertex buffer, then binds the buffer to
    /// // vertex buffer slot 0
    /// let mesh = Mesh::triangle();
    /// let vertex_buffer = gpu.create_vertex_buffer(&mesh);
    /// render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    /// ```
    fn create_vertex_buffer(&self, mesh: &Mesh) -> Buffer {
        let contents = mesh.to_bytes(); // flattens the mesh into a vector of bytes
        let descriptor = BufferInitDescriptor {
            label: Some("Mesh vertices"),
            contents: &contents,
            usage: BufferUsages::VERTEX,
        };
        self.device.create_buffer_init(&descriptor)
    }

    /// Compiles the WGSL code in `source` into a shader for the GPU.
    ///
    /// A shader is a small program that the GPU runs, once for each vertex or once for each pixel.
    /// WGSL is the language that WebGPU shaders are written in. The device checks the code before
    /// compiling it.
    ///
    /// `label` identifies the shader in wgpu's error messages and in GPU debugging tools.
    ///
    /// # Examples
    ///
    /// The example compiles the shader file that the build embeds in the binary.
    ///
    /// ```ignore
    /// // Embeds the shader file in the binary as a string, then compiles it into a shader
    /// let shader_source = include_str!("../shaders/flat_colour.wgsl");
    /// let shader = gpu.create_shader("Flat colour shader", shader_source);
    /// ```
    fn create_shader(&self, label: &str, source: &str) -> ShaderModule {
        let descriptor = ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(Cow::Borrowed(source)),
        };
        self.device.create_shader_module(descriptor)
    }

    /// Builds the pipeline that draws a `Mesh` in one flat colour.
    ///
    /// The pipeline runs two functions from `shader`. `vs_main` runs once for each vertex, and
    /// places the vertex in the window. `fs_main` runs once for each pixel that the mesh covers,
    /// and returns the pixel's colour. The pipeline writes into textures in the surface's pixel
    /// format, taking `SAMPLE_COUNT` samples for each pixel.
    ///
    /// # Examples
    ///
    /// The example builds the pipeline, then hands it to a render pass.
    ///
    /// ```ignore
    /// // Builds the pipeline, then selects it for the draws that follow in the render pass
    /// let pipeline = gpu.create_pipeline(&shader);
    /// render_pass.set_pipeline(&pipeline);
    /// ```
    fn create_pipeline(&self, shader: &ShaderModule) -> RenderPipeline {
        // Reads each vertex from buffer slot 0 through `Mesh::LAYOUT`, and passes it to `vs_main`
        let vertex_buffers = [Some(Mesh::LAYOUT)];
        let vertex = VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &vertex_buffers,
        };

        // Colours each covered pixel with `fs_main`, replacing what was there
        let targets = [Some(ColorTargetState {
            format: self.format,
            blend: Some(BlendState::REPLACE),
            write_mask: ColorWrites::ALL,
        })];
        let fragment = FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &targets,
        };

        // Joins every three vertices into one filled triangle
        let primitive = PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            ..Default::default()
        };

        let descriptor = RenderPipelineDescriptor {
            label: Some("Flat colour pipeline"),
            layout: None,
            vertex,
            primitive,
            depth_stencil: None,
            multisample: MultisampleState {
                count: Self::SAMPLE_COUNT,
                ..Default::default()
            },
            fragment: Some(fragment),
            multiview_mask: None,
            cache: None,
        };
        self.device.create_render_pipeline(&descriptor)
    }

    // ====================================================================================== \\
    //                                     Helper Methods                                     \\
    // ====================================================================================== \\
    /// Creates a multisampled texture to match the surface, and returns a view of it.
    ///
    /// The texture has the size and pixel format of the surface. It stores `SAMPLE_COUNT` samples
    /// for each pixel. A render pass draws into the texture through the returned view.
    ///
    /// # Examples
    ///
    /// `SurfaceContext::new` calls it once it has configured the surface.
    ///
    /// ```ignore
    /// // Configures the surface, then creates a multisampled texture to match it
    /// surface.configure(device, &config);
    /// let msaa_view = GpuContext::create_msaa_view(device, &config);
    /// ```
    fn create_msaa_view(device: &Device, config: &SurfaceConfiguration) -> TextureView {
        let size = Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let descriptor = TextureDescriptor {
            label: Some("MSAA colour"),
            size,
            mip_level_count: 1,
            sample_count: Self::SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format: config.format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let texture = device.create_texture(&descriptor);
        texture.create_view(&TextureViewDescriptor::default())
    }

    /// Creates a `SurfaceContext` for `window`, at `width` by `height` pixels.
    ///
    /// It creates the surface first. It then configures the surface in the pixel format that the
    /// pipeline was built for. Last, it creates a multisampled texture to match the surface.
    ///
    /// # Errors
    ///
    /// Returns `RendererError::Surface` where wgpu cannot create the surface. Returns
    /// `RendererError::UnsupportedSurface` where the GPU cannot draw to the surface in that pixel
    /// format.
    ///
    /// # Examples
    ///
    /// `recreate` and `resume` both call it to replace the surface context.
    ///
    /// ```ignore
    /// // Creates a surface context for the window, then stores it in place of the old one
    /// let surface_context = self.create_surface_context(window, width, height)?;
    /// self.surface_context = Some(surface_context);
    /// ```
    fn create_surface_context(
        &self,
        window: Arc<dyn DisplayAndWindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<SurfaceContext, RendererError> {
        let surface = self.instance.create_surface(Arc::clone(&window))?;
        let caps = surface.get_capabilities(&self.adapter);
        if !caps.formats.contains(&self.format) {
            return Err(RendererError::UnsupportedSurface);
        }

        let mut config = surface
            .get_default_config(&self.adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        config.format = self.format;

        Ok(SurfaceContext::new(window, surface, &self.device, config))
    }
}

/// The surface for one window, together with the parts built to match it.
///
/// The renderer replaces all of these parts together, whenever it replaces the surface.
struct SurfaceContext {
    /// The window that the surface belongs to. `GpuContext::recreate` creates a new surface for
    /// it when wgpu reports the surface lost.
    window: Arc<dyn DisplayAndWindowHandle>,
    /// The surface, which hands out one texture for each frame to draw into. It shows each
    /// finished texture in the window.
    surface: Surface<'static>,
    /// The size and pixel format of the surface's textures.
    config: SurfaceConfiguration,
    /// A view of the multisampled texture. The render pass draws each frame into this texture
    /// first. The GPU then averages each pixel's samples into the surface texture.
    msaa_view: TextureView,
}

impl SurfaceContext {
    /// Configures `surface` with `config`, and creates a multisampled texture of the same size
    /// and pixel format.
    ///
    /// # Examples
    ///
    /// The example builds a surface context from the surface's default configuration.
    ///
    /// ```ignore
    /// // Configures a new surface at the window's size, in the surface's preferred pixel format
    /// let config = surface
    ///     .get_default_config(&adapter, width, height)
    ///     .ok_or(RendererError::UnsupportedSurface)?;
    /// let surface_context = SurfaceContext::new(window, surface, &device, config);
    /// ```
    fn new(
        window: Arc<dyn DisplayAndWindowHandle>,
        surface: Surface<'static>,
        device: &Device,
        mut config: SurfaceConfiguration,
    ) -> Self {
        // Raises a width and/or height of zero to 1 pixel (since `configure` panics on a size of zero)
        config.width = config.width.max(1);
        config.height = config.height.max(1);
        config.present_mode = PresentMode::AutoVsync;
        // Configures the surface at that size, in the configuration's pixel format
        surface.configure(device, &config);
        // Creates the multisampled texture at the same size and pixel format
        let msaa_view = GpuContext::create_msaa_view(device, &config);
        // Returns the context, which keeps the configuration for later resizes
        Self {
            window,
            surface,
            config,
            msaa_view,
        }
    }

    /// Reconfigures the surface at `width` by `height` pixels.
    ///
    /// It also replaces the multisampled texture with one of the new size.
    ///
    /// # Examples
    ///
    /// The example resizes both for a window of 1920 by 1080 pixels.
    ///
    /// ```ignore
    /// // Reconfigures the surface at 1920 by 1080 pixels, and replaces the multisampled texture
    /// surface_context.resize(&device, 1920, 1080);
    /// ```
    fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.reconfigure(device);
        // Replaces the multisampled texture with one at the new size, since wgpu fixes a
        // texture's size when it creates the texture
        self.msaa_view = GpuContext::create_msaa_view(device, &self.config);
    }

    /// Rebuilds the surface's textures from the current configuration.
    ///
    /// # Examples
    ///
    /// `resize` stores the new size in the configuration, then calls `reconfigure`.
    ///
    /// ```ignore
    /// // Stores a new size, then rebuilds the surface's textures at that size
    /// self.config.width = width;
    /// self.config.height = height;
    /// self.reconfigure(device);
    /// ```
    fn reconfigure(&self, device: &Device) {
        self.surface.configure(device, &self.config);
    }
}
