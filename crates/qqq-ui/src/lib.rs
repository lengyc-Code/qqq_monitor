#![forbid(unsafe_code)]

mod app;
mod clips;
mod components;
mod desktop;
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
pub use desktop::{Desktop, Display, Input, Shell, System};
pub use dock::viewport;
