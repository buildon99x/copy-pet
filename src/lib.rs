//! DeskCat library: the platform-agnostic core (rendering, font, sound,
//! persistent state and the [`pet::Pet`] simulation) plus the per-OS shells
//! under [`platform`].
//!
//! The `deskcat` binary calls [`platform::run`]; `gen_icon` reuses
//! [`render::draw_icon_scaled`] to regenerate the embedded `.ico` asset.

pub mod font;
pub mod input;
pub mod pet;
pub mod platform;
pub mod render;
pub mod sound;
pub mod state;
