//! Draws Sarx's frames on the GPU through wgpu.
//!
//! A caller creates a [`Renderer`] for its window and calls [`Renderer::render`] each time the
//! window needs redrawing. [`Renderer::new`] and [`Renderer::resume`] return a [`RendererError`]
//! where the GPU cannot be set up for the window.

mod error;
mod renderer;
mod mesh;

pub use error::RendererError;
pub use renderer::Renderer;
