#![forbid(unsafe_code)]

mod app;
mod clips;
mod components;
mod dock;
pub mod i18n;
mod markdown;
mod mascot;
mod minibar;
mod note;
mod notes;
mod pixel;
mod shots;
mod theme;
mod tray;

pub use app::*;
pub use dock::{Desktop, viewport};
