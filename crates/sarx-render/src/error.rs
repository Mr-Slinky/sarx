//! The error that the renderer returns when it cannot set up the GPU for a window.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError};

/// A failure to set up the GPU for a window.
///
/// [`Renderer::new`](crate::Renderer::new), [`Renderer::resume`](crate::Renderer::resume) and
/// [`Renderer::render`](crate::Renderer::render) return it.
///
/// Some variants wrap an error from wgpu. For those variants, [`Error::source`] returns the wgpu
/// error.
///
/// # Examples
///
/// The example prints the failure, then the wgpu error behind it, where there is one.
///
/// ```ignore
/// if let Err(error) = renderer.render() {
///     // Prints the failure
///     eprintln!("Error: {error}");
///
///     // Prints the wgpu error that `source` returns, where there is one
///     if let Some(cause) = error.source() {
///         eprintln!("Caused by: {cause}");
///     }
/// }
/// ```
#[derive(Debug)]
pub enum RendererError {
    /// wgpu failed to create a surface for the window.
    Surface(CreateSurfaceError),
    /// No GPU can draw to the window's surface.
    Adapter(RequestAdapterError),
    /// The GPU refused to open a device within the WebGPU baseline limits. A device is the open
    /// connection to the GPU that the renderer draws through.
    Device(RequestDeviceError),
    /// The GPU cannot draw to the window's surface in the way that the renderer needs.
    ///
    /// The renderer returns it in two cases:
    ///
    /// - The GPU offers no pixel format or no present mode for the surface. A present mode sets
    ///   when each finished frame appears in the window.
    /// - A new surface lacks the pixel format that the pipeline was built for.
    UnsupportedSurface,
    /// The GPU cannot anti-alias in the surface's pixel format.
    ///
    /// Anti-aliasing smooths the jagged edges of a shape. It needs the GPU to take several samples
    /// for each pixel. In the surface's pixel format, this GPU cannot take the number of samples
    /// that the renderer asks for.
    UnsupportedSampleCount,
}

impl Display for RendererError {
    /// Writes a one-line description of the failure, starting in lower case.
    ///
    /// The description covers the failure alone. `source` returns the wgpu error behind it.
    ///
    /// # Examples
    ///
    /// The description of `UnsupportedSurface` starts in lower case.
    ///
    /// ```
    /// use sarx_render::RendererError;
    ///
    /// // Writes one line, starting in lower case
    /// let message = RendererError::UnsupportedSurface.to_string();
    /// let first = message.chars().next().expect("the description should not be empty");
    /// assert!(first.is_lowercase());
    /// assert!(!message.contains('\n'));
    /// ```
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Surface(_) => "failed to create a surface for the window",
            Self::Adapter(_) => "failed to find a GPU that can draw to the window",
            Self::Device(_) => "failed to open a device on the GPU",
            Self::UnsupportedSurface => "the GPU cannot draw to the window's surface in the way that the renderer needs",
            Self::UnsupportedSampleCount => {
                "the GPU cannot anti-alias in the format for drawing to the window"
            }
        };
        f.write_str(message)
    }
}

impl Error for RendererError {
    /// Returns the wgpu error behind a `Surface`, `Adapter` or `Device` failure.
    ///
    /// The renderer detects the other failures itself, meaning `source` returns `None` for them.
    ///
    /// # Examples
    ///
    /// `UnsupportedSampleCount` is a failure that the renderer detects itself.
    ///
    /// ```
    /// use std::error::Error;
    ///
    /// use sarx_render::RendererError;
    ///
    /// // Returns `None`, since the renderer detected the failure itself
    /// let error = RendererError::UnsupportedSampleCount;
    /// assert!(error.source().is_none());
    /// ```
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Device(error) => Some(error),
            Self::UnsupportedSurface | Self::UnsupportedSampleCount => None,
        }
    }
}

impl From<CreateSurfaceError> for RendererError {
    /// Wraps `error` in `RendererError::Surface`.
    ///
    /// # Examples
    ///
    /// Inside a function that returns a `RendererError`, the `?` operator calls `from` to convert
    /// this wgpu error.
    ///
    /// ```ignore
    /// // Turns a failure to create the surface into `RendererError::Surface` through `?`
    /// let surface = instance.create_surface(window)?;
    /// ```
    fn from(error: CreateSurfaceError) -> Self {
        Self::Surface(error)
    }
}

impl From<RequestAdapterError> for RendererError {
    /// Wraps `error` in `RendererError::Adapter`.
    ///
    /// # Examples
    ///
    /// Inside a function that returns a `RendererError`, the `?` operator calls `from` to convert
    /// this wgpu error.
    ///
    /// ```ignore
    /// // Turns a failure to find a GPU into `RendererError::Adapter` through `?`
    /// let adapter = instance.request_adapter(&adapter_options).await?;
    /// ```
    fn from(error: RequestAdapterError) -> Self {
        Self::Adapter(error)
    }
}

impl From<RequestDeviceError> for RendererError {
    /// Wraps `error` in `RendererError::Device`.
    ///
    /// # Examples
    ///
    /// Inside a function that returns a `RendererError`, the `?` operator calls `from` to convert
    /// this wgpu error.
    ///
    /// ```ignore
    /// // Turns a failure to open the device into `RendererError::Device` through `?`
    /// let (device, queue) = adapter.request_device(&device_descriptor).await?;
    /// ```
    fn from(error: RequestDeviceError) -> Self {
        Self::Device(error)
    }
}
