#![forbid(unsafe_code)]

mod app;
mod components;
mod dock;
mod mascot;
mod minibar;
mod pixel;
mod theme;

pub use app::*;
pub use dock::{Desktop, viewport};
