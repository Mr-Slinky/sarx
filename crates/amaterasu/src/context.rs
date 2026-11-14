//! The connection to the GPU, and the surface that it draws into.
//!
//! A [`GpuContext`] opens the connection once, when the [`Renderer`](crate::Renderer) is created,
//! and keeps it for as long as the renderer exists. It also creates every GPU object that the
//! renderer draws with, such as buffers, shaders and the pipeline.
//!
//! A [`SurfaceContext`] stores the surface, which is the part of a window that the GPU draws into.
//! Unlike the connection, a surface can be lost and replaced while the program runs. The
//! `GpuContext` replaces its `SurfaceContext` whenever that happens.
//!
//! # Examples
//!
//! `Renderer::new` connects to the GPU, then creates the objects that each frame draws with.
//!
//! ```ignore
//! // Connects to a GPU that can draw to the window, and configures the window's surface
//! let gpu = GpuContext::new(window, width, height).await?;
//!
//! // Compiles the shader, then builds the pipeline around it
//! let shader = gpu.create_shader("Flat colour shader", shader_source);
//! let pipeline = gpu.create_pipeline(&shader);
//! ```

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BlendState, Buffer, BufferUsages, ColorTargetState, ColorWrites, Device,
    DeviceDescriptor, DisplayAndWindowHandle, Extent3d, FragmentState, Instance,
    InstanceDescriptor, Limits, MultisampleState, PresentMode, PrimitiveState, PrimitiveTopology,
    Queue, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, Surface, SurfaceConfiguration, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    VertexState,
};

use crate::RendererError;
use crate::mesh::Mesh;
use crate::renderer::RenderWindow;

/// The connection to one GPU, together with the surface that the GPU draws into.
///
/// The connection lasts as long as the renderer. The surface is dropped while the app is
/// suspended, leaving `surface_context` as `None`.
pub(crate) struct GpuContext {
    /// The surface and the parts built to match it. It is `None` while the app is suspended.
    pub(crate) surface_context: Option<SurfaceContext>,
    /// The wgpu instance, which creates each surface. `new` also uses it to find the adapter.
    instance: Instance,
    /// The GPU that the renderer draws with. wgpu calls a GPU an adapter.
    adapter: Adapter,
    /// The open connection to the adapter. The device creates every buffer, texture, shader and
    /// pipeline.
    pub(crate) device: Device,
    /// The queue that sends recorded commands to the GPU.
    pub(crate) queue: Queue,
    /// The pixel format of the first surface. A pixel format sets how each pixel's colour is laid
    /// out in memory. `create_pipeline` builds the pipeline for this format, and every later
    /// surface therefore takes it too.
    format: TextureFormat,
    /// Stores `true` once the connection to the GPU has died, for example after a driver reset.
    /// The device-lost callback sets it, and wgpu can run that callback on another thread. The
    /// `Arc` shares the flag between this struct and that callback.
    pub(crate) device_lost: Arc<AtomicBool>,
}

impl GpuContext {
    /// The number of samples that the GPU takes for each pixel.
    ///
    /// A sample tests whether a shape covers one point inside the pixel. The GPU averages the
    /// samples into the pixel's colour, which smooths the jagged edges of a shape. This smoothing
    /// is called anti-aliasing.
    const SAMPLE_COUNT: u32 = 4;

    const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;
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
    /// - The GPU does not offer a pixel format or a present mode for the surface.
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
    pub(crate) async fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, RendererError>
    where
        W: RenderWindow,
    {
        // Boxes a second `Arc` to the window, which the instance keeps as its display handle
        let display = Box::new(Arc::clone(&window));
        // Creates the wgpu instance, and gives it that display handle
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(display));
        // Creates the surface, which keeps a third owner of the window
        let surface = instance.create_surface(Arc::clone(&window))?;

        // Chooses a GPU that can draw to the surface
        let adapter_options = RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        };
        let adapter = instance.request_adapter(&adapter_options).await?;

        // Opens the device and its queue, within the WebGPU baseline limits that ADR 0003 settles on
        let device_descriptor = DeviceDescriptor {
            label: Some("Sarx device"),
            required_limits: Limits::default(),
            ..Default::default()
        };
        let (device, queue) = adapter.request_device(&device_descriptor).await?;

        // Creates a flag to record a lost device, and gives the callback its own handle to it
        let device_lost = Arc::new(AtomicBool::new(false));
        let callback_flag = Arc::clone(&device_lost);
        // Sets the flag when the device dies. `move` hands the handle to the callback, which owns
        // it from then on, as `'static` requires. `Relaxed` suffices, since no other data depends
        // on the flag
        device.set_device_lost_callback(move |_reason, _message| {
            callback_flag.store(true, Ordering::Relaxed);
        });

        // Gets the surface's default configuration at the window's size
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        // Prefers the first sRGB format that the GPU can draw to the surface in, and falls back to
        // the default format. An sRGB format makes the GPU convert each colour that the shader
        // writes into the sRGB encoding that screens display. The pipeline and every later surface
        // take the chosen format
        let caps = surface.get_capabilities(&adapter);
        let srgb_format = caps.formats.iter().copied().find(|format| format.is_srgb());
        config.format = srgb_format.unwrap_or(config.format);
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
            device_lost,
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
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
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
    pub(crate) fn reconfigure(&self) {
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
    pub(crate) fn recreate(&mut self) -> Result<(), RendererError> {
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
    pub(crate) fn suspend(&mut self) {
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
    pub(crate) fn resume<W>(&mut self, window: Arc<W>, width: u32, height: u32) -> Result<(), RendererError>
    where
        W: RenderWindow,
    {
        // Drops the old surface first (where one exists) since Vulkan can refuse a second surface
        // for a window that already has one
        self.surface_context = None;
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
    /// let mesh = Mesh::build_triangle();
    /// let vertex_buffer = gpu.create_vertex_buffer(&mesh);
    /// render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    /// ```
    pub(crate) fn create_vertex_buffer(&self, mesh: &Mesh) -> Buffer {
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
    pub(crate) fn create_shader(&self, label: &str, source: &str) -> ShaderModule {
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
    pub(crate) fn create_pipeline(&self, shader: &ShaderModule) -> RenderPipeline {
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
    /// Creates a `SurfaceContext` for `window`, at `width` by `height` pixels.
    ///
    /// It creates the surface, checks that the surface supports the pixel format that the pipeline
    /// was built for, and configures it in that format. Last, it creates a multisampled texture to
    /// match the surface.
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
pub(crate) struct SurfaceContext {
    window: Arc<dyn DisplayAndWindowHandle>,
    pub(crate) surface: Surface<'static>,
    config: SurfaceConfiguration,
    pub(crate) msaa_view: TextureView,
    pub(crate) depth_view: TextureView,
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
        // Raises a width or height of zero to 1 pixel, since `configure` panics on a size of zero
        config.width = config.width.max(1);
        config.height = config.height.max(1);
        // Syncs each frame to the screen's refresh, which caps the frame rate at the refresh rate
        config.present_mode = PresentMode::AutoVsync;
        // Configures the surface at that size, in the configuration's pixel format
        surface.configure(device, &config);
        let msaa_view = Self::create_msaa_view(device, &config);
        let depth_view = Self::create_depth_view(device, &config);

        Self {
            window,
            surface,
            config,
            msaa_view,
            depth_view,
        }
    }

    // ====================================================================================== \\
    //                                     Helper Methods                                     \\
    // ====================================================================================== \\
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
        self.msaa_view = Self::create_msaa_view(device, &self.config);
        self.depth_view = Self::create_depth_view(device, &self.config);
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

    /// Creates a multisampled texture to match the surface, and returns a view of it.
    ///
    /// The texture has the size and pixel format of the surface. It stores
    /// `GpuContext::SAMPLE_COUNT` samples for each pixel. A render pass draws into the texture
    /// through the returned view.
    ///
    /// # Examples
    ///
    /// `SurfaceContext::new` calls it once it has configured the surface.
    ///
    /// ```ignore
    /// // Configures the surface, then creates a multisampled texture to match it
    /// surface.configure(device, &config);
    /// let msaa_view = Self::create_msaa_view(device, &config);
    /// ```
    fn create_msaa_view(device: &Device, config: &SurfaceConfiguration) -> TextureView {
        let size = Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        // Describes a two-dimensional texture that a render pass draws into, with several samples
        // for each pixel
        let descriptor = TextureDescriptor {
            label: Some("MSAA colour"),
            size,
            mip_level_count: 1,
            sample_count: GpuContext::SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format: config.format,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let texture = device.create_texture(&descriptor);
        // Returns a view of the whole texture, which the render pass draws through
        texture.create_view(&TextureViewDescriptor::default())
    }

    fn create_depth_view(device: &Device, config: &SurfaceConfiguration) -> TextureView {
        // size texture to match surface
        let size = Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let descr = TextureDescriptor {
            label: Some("Depth"),
            size,
            mip_level_count: 1,
            sample_count: GpuContext::SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format: GpuContext::DEPTH_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture = device.create_texture(&descr);
        texture.create_view(&TextureViewDescriptor::default())
    }
}
