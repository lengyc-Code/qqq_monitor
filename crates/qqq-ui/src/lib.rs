#![forbid(unsafe_code)]

mod app;
mod components;
mod dock;
pub mod i18n;
mod mascot;
mod minibar;
mod pixel;
mod theme;
mod tray;

pub use app::*;
pub use dock::{Desktop, viewport};
