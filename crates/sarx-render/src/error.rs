use std::error::Error;
use std::fmt::{self, Display, Formatter};

use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError};

/// A failure to set up the renderer on the GPU.
#[derive(Debug)]
pub enum RendererError {
    /// wgpu failed to create a surface for the window.
    Surface(CreateSurfaceError),
    /// No GPU can draw to the window's surface.
    Adapter(RequestAdapterError),
    /// The GPU refused to open a device within the WebGPU standard's baseline limits.
    Device(RequestDeviceError),
    /// The GPU supports no format or present mode for drawing to the window, or the window's new
    /// surface lacks the format that the pipeline was built for.
    UnsupportedSurface,
    /// The GPU cannot take the number of samples per pixel that anti-aliasing needs, in the
    /// surface's format.
    UnsupportedSampleCount,
}

impl Display for RendererError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Surface(_) => "failed to create a surface for the window",
            Self::Adapter(_) => "failed to find a GPU that can draw to the window",
            Self::Device(_) => "failed to open a device on the GPU",
            Self::UnsupportedSurface => "the GPU offers no format in which the renderer can draw to the window",
            Self::UnsupportedSampleCount => {
                "the GPU cannot anti-alias in the format for drawing to the window"
            }
        };
        f.write_str(message)
    }
}

impl Error for RendererError {
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
    fn from(error: CreateSurfaceError) -> Self {
        Self::Surface(error)
    }
}

impl From<RequestAdapterError> for RendererError {
    fn from(error: RequestAdapterError) -> Self {
        Self::Adapter(error)
    }
}

impl From<RequestDeviceError> for RendererError {
    fn from(error: RequestDeviceError) -> Self {
        Self::Device(error)
    }
}
