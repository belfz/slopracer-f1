//! Slopracer F1 — a retro pseudo-3D Formula racer for the terminal and the browser.
//! Everything but the two front ends (`term` and `web`) is platform-neutral.

// The modules are public only so the binary and the wasm entry point can
// reach them; this is not a general-purpose API.
#![allow(clippy::new_without_default)]

pub mod app;
pub mod audio;
pub mod boot;
pub mod gfx;
pub mod input;
#[cfg(not(target_arch = "wasm32"))]
pub mod kitty;
pub mod music;
pub mod pad;
pub mod race;
pub mod render;
pub mod settings;
pub mod sprites;
#[cfg(not(target_arch = "wasm32"))]
pub mod term;
pub mod track;
pub mod ui;
#[cfg(target_arch = "wasm32")]
mod web;
