use std::fmt::Debug;

use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};
use wgpu::{
    Color, CommandEncoderDescriptor, CurrentSurfaceTexture, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, Limits, LoadOp, Operations, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration,
    TextureViewDescriptor,
};

use crate::RendererError;

// ========================================================================================== \\
//                                         Public API                                         \\
// ========================================================================================== \\

/// Draws frames on the GPU into one window.
pub struct Renderer {
    gpu: GpuContext,
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
        Ok(Self { gpu })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    /// Draws one frame, cleared to `CLEAR_COLOUR`, and presents it in the window.
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

        // Records a render pass that clears the image to one colour
        let encoder_descriptor = CommandEncoderDescriptor {
            label: Some("Sarx frame"),
        };
        let mut encoder = self.gpu.device.create_command_encoder(&encoder_descriptor);
        let colour_attachment = RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Self::CLEAR_COLOUR),
                store: StoreOp::Store,
            },
        };
        let pass_descriptor = RenderPassDescriptor {
            label: Some("Clear"),
            color_attachments: &[Some(colour_attachment)],
            ..Default::default()
        };
        let render_pass = encoder.begin_render_pass(&pass_descriptor);
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
}

impl GpuContext {
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

        Ok(Self {
            surface,
            device,
            queue,
            config,
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
    }

    /// Rebuilds the surface's images from the current configuration.
    fn reconfigure(&self) {
        self.surface.configure(&self.device, &self.config);
    }
}
