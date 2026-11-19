//! The error that the renderer returns when it cannot set up the GPU for a window, or cannot keep
//! drawing to it.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError};

/// A failure to set up the GPU for a window, or to keep drawing to it.
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
/// if let Err(error) = renderer.render(&camera, hand_transform) {
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
    /// - The GPU does not offer a pixel format or a present mode for the surface. A present mode
    ///   sets when each finished frame appears in the window.
    /// - A new surface lacks the pixel format that the pipelines were built for.
    UnsupportedSurface,
    /// The GPU cannot anti-alias in the surface's pixel format.
    ///
    /// Anti-aliasing smooths the jagged edges of a shape. It needs the GPU to take several samples
    /// for each pixel. In the surface's pixel format, this GPU cannot take the number of samples
    /// that the renderer asks for.
    UnsupportedSampleCount,
    /// The connection to the GPU has died, for example after a driver reset.
    ///
    /// From then on, `Renderer::render` returns this error on every call made while a surface
    /// exists, since nothing made from the dead connection works again.
    DeviceLost,
    /// wgpu rejected a call as invalid while handing out the texture for a frame. This points to a
    /// bug in the renderer.
    Validation,
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
    /// use amaterasu::RendererError;
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
            Self::UnsupportedSurface => {
                "the GPU cannot draw to the window's surface in the way that the renderer needs"
            }
            Self::UnsupportedSampleCount => {
                "the GPU cannot anti-alias in the format for drawing to the window"
            }
            Self::DeviceLost => "the connection to the GPU has died",
            Self::Validation => "wgpu rejected a call as invalid while preparing a frame",
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
    /// use amaterasu::RendererError;
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
            Self::UnsupportedSurface
            | Self::UnsupportedSampleCount
            | Self::DeviceLost
            | Self::Validation => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns each variant that wraps no wgpu error, with its name
    fn build_own_errors() -> [(&'static str, RendererError); 4] {
        [
            ("UnsupportedSurface", RendererError::UnsupportedSurface),
            (
                "UnsupportedSampleCount",
                RendererError::UnsupportedSampleCount,
            ),
            ("DeviceLost", RendererError::DeviceLost),
            ("Validation", RendererError::Validation),
        ]
    }

    /// Checks that every description is one non-empty line, starting in lower case. `Adapter`
    /// stands in for the variants that wrap a wgpu error, being the only one of them that code
    /// outside wgpu can build
    #[test]
    fn fmt_with_every_buildable_variant_writes_one_lower_case_line() {
        let mut cases = Vec::from(build_own_errors());
        cases.push((
            "Adapter",
            RendererError::Adapter(RequestAdapterError::EnvNotSet),
        ));

        for (name, error) in cases {
            let message = error.to_string();

            let starts_lower_case = message.chars().next().is_some_and(char::is_lowercase);
            assert!(
                !message.is_empty(),
                "description of {name} should not be empty"
            );
            assert!(
                starts_lower_case,
                "description of {name} should start in lower case"
            );
            assert!(
                !message.contains('\n'),
                "description of {name} should be one line"
            );
        }
    }

    /// Checks that a failure that the renderer detects itself has no wgpu error behind it
    #[test]
    fn source_with_own_errors_returns_none() {
        let cases = build_own_errors();

        for (name, error) in cases {
            let source = error.source();

            assert!(source.is_none(), "source of {name}");
        }
    }

    /// Checks that `Adapter` returns the wgpu error that it wraps
    #[test]
    fn source_with_adapter_error_returns_wgpu_error() {
        let error = RendererError::Adapter(RequestAdapterError::EnvNotSet);

        let source = error.source();

        let wrapped = source.and_then(|source| source.downcast_ref::<RequestAdapterError>());
        assert!(
            matches!(wrapped, Some(RequestAdapterError::EnvNotSet)),
            "source of Adapter"
        );
    }

    /// Checks that the `?` operator turns a `RequestAdapterError` into the `Adapter` variant
    #[test]
    fn from_with_request_adapter_error_returns_adapter() {
        let wgpu_error = RequestAdapterError::EnvNotSet;

        let error = RendererError::from(wgpu_error);

        assert!(
            matches!(
                error,
                RendererError::Adapter(RequestAdapterError::EnvNotSet)
            ),
            "variant for a `RequestAdapterError`"
        );
    }
}
