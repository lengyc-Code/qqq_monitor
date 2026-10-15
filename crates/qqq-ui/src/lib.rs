#![forbid(unsafe_code)]

mod app;
mod clips;
mod components;
mod dock;
pub mod i18n;
mod mascot;
mod minibar;
mod pixel;
mod shots;
mod theme;
mod tray;

pub use app::*;
pub use dock::{Desktop, viewport};
