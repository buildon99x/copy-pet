//! ClipCat library: the platform-agnostic core (rendering, font + vector
//! Hangul, sound, persistent state, the clipboard store/panel and the
//! [`pet::Pet`] simulation) plus the per-OS shells under [`platform`].
//!
//! The `clipcat` binary calls [`platform::run`]; `gen_icon` reuses
//! [`render::draw_icon_scaled`] to regenerate the embedded `.ico` asset.

pub mod clipboard;
pub mod font;
pub mod hangul;
pub mod hotkey;
pub mod i18n;
pub mod input;
pub mod menu;
pub mod panel;
pub mod pet;
pub mod platform;
pub mod render;
pub mod sound;
pub mod state;
pub mod sysfont;
pub mod update;
