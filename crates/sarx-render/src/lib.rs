//! Draws frames into a window.
//!
//! The drawing runs on the GPU. The renderer reaches the GPU through wgpu, a Rust library for
//! graphics.
//!
//! A caller creates a [`Renderer`] for its window. It then calls [`Renderer::render`] each time the
//! window needs redrawing.
//!
//! Setting up the GPU for the window can fail. [`Renderer::new`] and [`Renderer::resume`] then
//! return a [`RendererError`].
//!
//! # Examples
//!
//! The example creates a renderer for a winit window, then draws one frame.
//!
//! Setting up the GPU is asynchronous, meaning `Renderer::new` returns a future. In the example,
//! `block_on` stands for any function that runs a future until it finishes.
//!
//! ```ignore
//! // Creates a renderer for the window, at the window's current size
//! let size = window.inner_size();
//! let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height);
//! let mut renderer = block_on(renderer_setup)?;
//!
//! // Draws one frame, as the caller does each time the window needs redrawing
//! renderer.render()?;
//! ```
//!
//! Most examples in these docs need a window from a windowing library, such as winit. As a result,
//! rustdoc leaves them uncompiled, and marks each one as untested.

mod error;
mod renderer;
mod mesh;

pub use error::RendererError;
pub use renderer::Renderer;
