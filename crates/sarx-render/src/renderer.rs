use std::borrow::Cow;
use std::fmt::Debug;

use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BlendState, Buffer, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, CurrentSurfaceTexture, Device, DeviceDescriptor, Extent3d,
    FragmentState, Instance, InstanceDescriptor, Limits, LoadOp, MultisampleState, Operations,
    PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, StoreOp, Surface, SurfaceConfiguration,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, VertexState,
};

use crate::RendererError;
use crate::mesh::Mesh;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Draws frames on the GPU into one window.
///
/// A caller creates a `Renderer` with `new` once the window exists, then calls `render` each time
/// the window needs redrawing. The caller passes each new window size to `resize`. It also calls
/// `suspend` when the app loses its window and `resume` when the app gets one back. A platform with
/// an app lifecycle, such as a phone, does this each time the app leaves the screen and returns.
/// Windows never does, and the two methods keep a later port open.
pub struct Renderer {
    /// The connection to the GPU, and the surface for the window.
    gpu: GpuContext,
    /// The triangle's vertices, copied into GPU memory.
    vertex_buffer: Buffer,
    /// The number of vertices in `vertex_buffer`, which `render` passes to each draw call. `new`
    /// copies it from the mesh before dropping the mesh.
    vertex_count: u32,
    /// The GPU's settings for drawing the triangle, from reading its vertices to writing its
    /// colour.
    pipeline: RenderPipeline,
}

impl Renderer {
    /// The colour that every frame starts from, a dark blue.
    const CLEAR_COLOUR: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    /// Connects to a GPU that can draw to `window` at `width` by `height` pixels, copies the
    /// triangle into GPU memory, and builds the pipeline that draws it.
    ///
    /// Returns a `RendererError` where wgpu cannot create a surface for the window, where no GPU
    /// can draw to it, where the GPU refuses the device or the surface, or where the GPU cannot
    /// anti-alias in the surface's format.
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

    /// Resizes the textures that each frame draws into, to `width` by `height` pixels. A width or
    /// height of zero, which a minimised window reports, leaves the sizes as they are.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    /// Drops the window's surface, for when the app is suspended and loses its window. `render`
    /// skips every frame until `resume` gives the renderer a window again.
    pub fn suspend(&mut self) {
        self.gpu.suspend();
    }

    /// Creates a surface for `window` at `width` by `height` pixels, for when the app resumes,
    /// and draws into it from then on.
    ///
    /// Returns a `RendererError` where wgpu cannot create the surface, or where the GPU cannot
    /// draw to it in the format that the pipeline was built for.
    pub fn resume<W>(&mut self, window: W, width: u32, height: u32) -> Result<(), RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        self.gpu.resume(window, width, height)
    }

    /// Draws one frame, the triangle over a `CLEAR_COLOUR` background, and presents it in the
    /// window. Skips the frame while the app is suspended, or while the surface has no texture
    /// to hand out.
    pub fn render(&mut self) {
        // Skips the frame while the app is suspended and has no surface to draw into
        let Some(surface_context) = &self.gpu.surface_context else {
            return;
        };

        // Gets the surface's next texture to draw into. Reconfigures the surface when it is
        // outdated, and skips the frame whenever no texture comes back
        let (frame, suboptimal) = match surface_context.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => (frame, false),
            CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            CurrentSurfaceTexture::Outdated => {
                self.gpu.reconfigure();
                return;
            }
            // TODO: recreate the surface on Lost
            CurrentSurfaceTexture::Timeout
            | CurrentSurfaceTexture::Occluded
            | CurrentSurfaceTexture::Lost
            | CurrentSurfaceTexture::Validation => return,
        };
        // Creates a view of the whole surface texture, the form in which a render pass takes it
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        // Creates the command encoder that records this frame's commands for the GPU
        let encoder_descriptor = CommandEncoderDescriptor {
            label: Some("Sarx frame"),
        };
        let mut encoder = self.gpu.device.create_command_encoder(&encoder_descriptor);

        // Points the pass at the multisampled texture, which the pass clears and draws into. The
        // pass then averages each pixel's samples into the surface texture
        let colour_attachment = RenderPassColorAttachment {
            view: &surface_context.msaa_view,
            depth_slice: None,
            resolve_target: Some(&view),
            ops: Operations {
                load: LoadOp::Clear(Self::CLEAR_COLOUR),
                // Throws the samples away once averaged, skipping a write to memory
                store: StoreOp::Discard,
            },
        };
        let pass_descriptor = RenderPassDescriptor {
            label: Some("Triangle"),
            color_attachments: &[Some(colour_attachment)],
            ..Default::default()
        };
        // Records a render pass that draws the triangle into the colour attachment
        let mut render_pass = encoder.begin_render_pass(&pass_descriptor);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        // Runs the pipeline over every vertex in the buffer, drawing one copy of the mesh
        render_pass.draw(0..self.vertex_count, 0..1);
        // Ends the pass, which hands the encoder back for `finish`
        drop(render_pass);

        // Sends the recorded commands to the GPU, then presents the frame's texture in the window
        let queue = &self.gpu.queue;
        queue.submit([encoder.finish()]);
        queue.present(frame);

        // Reconfigures the surface after `present` has taken the frame, since `configure` panics
        // while a texture from the surface is still alive
        if suboptimal {
            self.gpu.reconfigure();
        }
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

/// The wgpu objects that connect Sarx to one GPU and to the window that it draws into.
struct GpuContext {
    /// The surface and the parts built to match it, or `None` while the app is suspended and
    /// has no window to draw into.
    surface_context: Option<SurfaceContext>,
    /// The wgpu instance, which creates each surface. `new` also uses it to find the adapter.
    instance: Instance,
    /// The GPU that Sarx draws with.
    adapter: Adapter,
    /// The connection to the adapter, which creates buffers, textures, shaders and pipelines.
    device: Device,
    /// The queue that sends recorded commands to the GPU.
    queue: Queue,
    /// The pixel format of the first surface. `create_pipeline` builds the pipeline for it, and
    /// `resume` therefore configures every later surface with it.
    format: TextureFormat,
}

impl GpuContext {
    /// How many points per pixel the GPU tests for coverage, for anti-aliasing.
    const SAMPLE_COUNT: u32 = 4;

    // ====================================================================================== \\
    //                                      Constructors                                      \\
    // ====================================================================================== \\

    /// Connects to a GPU that can draw to `window`, and configures the window's surface at
    /// `width` by `height` pixels.
    async fn new<W>(window: W, width: u32, height: u32) -> Result<Self, RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle + Debug + Clone + Send + Sync + 'static,
    {
        // Creates the wgpu instance, giving it the window's display connection. Only the OpenGL
        // backend uses it, to present frames, and DirectX 12 and Vulkan ignore it. Passing it keeps
        // a later port open
        let display = Box::new(window.clone());
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(display));
        let surface = instance.create_surface(window)?;

        // Chooses a GPU that can draw to the surface
        let adapter_options = RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        };
        let adapter = instance.request_adapter(&adapter_options).await?;

        // Opens the device and its queue, within the WebGPU standard's baseline limits, which the
        // integrated GPU in a mid-level laptop meets
        let device_descriptor = DeviceDescriptor {
            label: Some("Sarx device"),
            required_limits: Limits::default(),
            ..Default::default()
        };
        let (device, queue) = adapter.request_device(&device_descriptor).await?;

        // Chooses the surface's configuration: its preferred format and present mode, at the
        // window's size
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        let format = config.format;

        // Checks that the GPU can anti-alias in the surface's format
        let format_features = adapter.get_texture_format_features(format);
        let flags = format_features.flags;
        if !flags.sample_count_supported(Self::SAMPLE_COUNT) {
            return Err(RendererError::UnsupportedSampleCount);
        }

        // Configures the surface and creates the texture that frames draw into
        let surface_context = SurfaceContext::new(surface, &device, config);

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

    /// Resizes the surface and the multisampled texture to `width` by `height` pixels, while a
    /// surface exists. A width or height of zero leaves both as they are, since
    /// `Surface::configure` panics on a width or height of zero.
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(surface_context) = self.surface_context.as_mut() {
            surface_context.resize(&self.device, width, height);
        }
    }

    /// Rebuilds the surface's textures from the current configuration, while a surface exists.
    fn reconfigure(&self) {
        if let Some(surface_context) = &self.surface_context {
            surface_context.reconfigure(&self.device);
        }
    }

    /// Drops the surface, its configuration and the multisampled texture.
    fn suspend(&mut self) {
        self.surface_context = None;
    }

    /// Creates a surface for `window` at `width` by `height` pixels, in the format that the
    /// pipeline is built for.
    ///
    /// Returns `RendererError::Surface` where wgpu cannot create the surface, and
    /// `RendererError::UnsupportedSurface` where the GPU cannot draw to it in that format.
    fn resume<W>(&mut self, window: W, width: u32, height: u32) -> Result<(), RendererError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        // Drops any surface still held, since a driver can refuse a second surface on one window
        self.surface_context = None;

        // Creates the surface, and checks that it can take the format that the pipeline is built
        // for
        let surface = self.instance.create_surface(window)?;
        let capabilities = surface.get_capabilities(&self.adapter);
        if !capabilities.formats.contains(&self.format) {
            return Err(RendererError::UnsupportedSurface);
        }

        // Configures the surface at the window's size, in the pipeline's format
        let mut config = surface
            .get_default_config(&self.adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        config.format = self.format;
        let surface_context = SurfaceContext::new(surface, &self.device, config);
        self.surface_context = Some(surface_context);
        Ok(())
    }

    /// Copies the vertices of `mesh` into a new buffer in GPU memory, marked for use as a vertex
    /// buffer in a draw.
    fn create_vertex_buffer(&self, mesh: &Mesh) -> Buffer {
        let contents = mesh.to_bytes(); // flattens the mesh into a vector of bytes
        let descriptor = BufferInitDescriptor {
            label: Some("Mesh vertices"),
            contents: &contents,
            usage: BufferUsages::VERTEX,
        };
        self.device.create_buffer_init(&descriptor)
    }

    /// Hands the WGSL code in `source` to the device, which checks it and compiles it for the GPU.
    fn create_shader(&self, label: &str, source: &str) -> ShaderModule {
        let descriptor = ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(Cow::Borrowed(source)),
        };
        self.device.create_shader_module(descriptor)
    }

    /// Builds the pipeline that draws a `Mesh` in one flat colour, from the `vs_main` and
    /// `fs_main` functions in `shader`, into textures in the surface's format.
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

    /// Creates a texture the size and format of the surface, storing `SAMPLE_COUNT` samples per
    /// pixel, and returns a view of it for a render pass to draw into.
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
}

/// The parts of the renderer that belong to one window, which are replaced together whenever the
/// window is.
struct SurfaceContext {
    /// Hands out the textures that frames draw into, and shows each finished one in the window.
    surface: Surface<'static>,
    /// The size and format of the surface's textures.
    config: SurfaceConfiguration,
    /// A view of the multisampled texture that each frame draws into, before the GPU averages
    /// each pixel's samples into the surface texture.
    msaa_view: TextureView,
}

impl SurfaceContext {
    /// Configures `surface` with `config`, and creates a multisampled texture of the same size
    /// and format.
    fn new(surface: Surface<'static>, device: &Device, config: SurfaceConfiguration) -> Self {
        surface.configure(device, &config);
        let msaa_view = GpuContext::create_msaa_view(device, &config);
        Self {
            surface,
            config,
            msaa_view,
        }
    }

    /// Reconfigures the surface at `width` by `height` pixels, and replaces the multisampled
    /// texture with one of the new size.
    fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.reconfigure(device);
        // Replaces the multisampled texture with one at the new size, since wgpu fixes a
        // texture's size when it creates the texture
        self.msaa_view = GpuContext::create_msaa_view(device, &self.config);
    }

    /// Rebuilds the surface's textures from the current configuration.
    fn reconfigure(&self, device: &Device) {
        self.surface.configure(device, &self.config);
    }
}
