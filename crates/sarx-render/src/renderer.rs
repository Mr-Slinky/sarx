use std::borrow::Cow;
use std::fmt::Debug;

use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BlendState, Buffer, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, CurrentSurfaceTexture, Device, DeviceDescriptor, Extent3d,
    FragmentState, Instance, InstanceDescriptor, Limits, LoadOp, MultisampleState, Operations,
    PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, StoreOp, Surface, SurfaceConfiguration,
    TextureDescriptor, TextureDimension, TextureUsages, TextureView, TextureViewDescriptor,
    VertexState,
};

use crate::RendererError;
use crate::mesh::Mesh;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Draws frames on the GPU into one window.
pub struct Renderer {
    gpu: GpuContext,
    vertex_buffer: Buffer,
    /// How many vertices `vertex_buffer` holds, kept since the mesh itself is gone once uploaded.
    vertex_count: u32,
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

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    /// Draws one frame, the triangle over a `CLEAR_COLOUR` background, and presents it in the
    /// window.
    pub fn render(&mut self) {
        // Gets the next image to draw into, or skips the frame when the surface has none
        let (frame, suboptimal) = match self.gpu.surface.get_current_texture() {
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
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        // Records a render pass that clears the image to one colour, then draws the triangle.
        // The pass draws into the multisampled texture, then averages each pixel's samples into
        // the surface's image.
        let encoder_descriptor = CommandEncoderDescriptor {
            label: Some("Sarx frame"),
        };
        let mut encoder = self.gpu.device.create_command_encoder(&encoder_descriptor);
        let colour_attachment = RenderPassColorAttachment {
            view: &self.gpu.msaa_view,
            depth_slice: None,
            resolve_target: Some(&view),
            ops: Operations {
                load: LoadOp::Clear(Self::CLEAR_COLOUR),
                // Throws the samples away once averaged, saving a write to memory
                store: StoreOp::Discard,
            },
        };
        let pass_descriptor = RenderPassDescriptor {
            label: Some("Triangle"),
            color_attachments: &[Some(colour_attachment)],
            ..Default::default()
        };
        let mut render_pass = encoder.begin_render_pass(&pass_descriptor);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
        // Ends the pass, which hands the encoder back for `finish`
        drop(render_pass);

        // Sends the recorded commands to the GPU, then shows the finished image
        let queue = &self.gpu.queue;
        queue.submit([encoder.finish()]);
        queue.present(frame);

        // Rebuilds the surface's images once the frame is out, since configure panics while one is held
        if suboptimal {
            self.gpu.reconfigure();
        }
    }
}

// ========================================================================================== \\
//                                          Helpers                                           \\
// ========================================================================================== \\

struct GpuContext {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    /// The texture that every frame draws into, at `SAMPLE_COUNT` samples per pixel, before
    /// they are averaged into the surface's image.
    msaa_view: TextureView,
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
        // Creates the wgpu instance, giving it the display that OpenGL ES needs on Android
        let display = Box::new(window.clone());
        let instance = Instance::new(InstanceDescriptor::new_with_display_handle(display));
        let surface = instance.create_surface(window)?;

        // Chooses a GPU that can draw to the surface
        let adapter_options = RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        };
        let adapter = instance.request_adapter(&adapter_options).await?;

        // Opens the device and its queue, limited to what phones support
        let device_descriptor = DeviceDescriptor {
            label: Some("Sarx device"),
            required_limits: Limits::downlevel_defaults(),
            ..Default::default()
        };
        let (device, queue) = adapter.request_device(&device_descriptor).await?;

        // Configures the surface with the GPU's preferred format at the window's size
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RendererError::UnsupportedSurface)?;
        surface.configure(&device, &config);

        // Checks that the GPU can anti-alias in the surface's format, then creates the texture
        // that frames draw into
        let format_features = adapter.get_texture_format_features(config.format);
        let flags = format_features.flags;
        if !flags.sample_count_supported(Self::SAMPLE_COUNT) {
            return Err(RendererError::UnsupportedSampleCount);
        }
        let msaa_view = Self::create_msaa_view(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            msaa_view,
        })
    }

    // ====================================================================================== \\
    //                                      API Methods                                       \\
    // ====================================================================================== \\

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.reconfigure();
        // A texture's size is fixed, so the multisampled one is replaced at the new size
        self.msaa_view = Self::create_msaa_view(&self.device, &self.config);
    }

    /// Rebuilds the surface's images from the current configuration.
    fn reconfigure(&self) {
        self.surface.configure(&self.device, &self.config);
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

    fn create_pipeline(&self, shader: &ShaderModule) -> RenderPipeline {
        // Vertex stage: reads each vertex from buffer slot 0 and passes it to `vs_main`
        let vertex_buffers = [Some(Mesh::LAYOUT)];
        let vertex = VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &vertex_buffers,
        };

        // Fragment stage: colours each covered pixel with `fs_main`, replacing what was there
        let targets = [Some(ColorTargetState {
            format: self.config.format,
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

    /// Creates a texture the size and format of the surface, holding `SAMPLE_COUNT` samples per
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
