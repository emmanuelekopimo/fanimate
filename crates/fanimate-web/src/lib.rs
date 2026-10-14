//! Fanimate in the browser.
//!
//! The web build opens, plays and checks projects with the same engine
//! (`fanimate-core`), renderer (`fanimate-render`, through WebGPU) and
//! importers as the desktop app: a `.fnm` project packed as a ZIP, a Moho
//! project or an Animate `.fla`, picked or dropped on the page. It is for
//! reviewing and sharing work; drawing, rigging, saving and export are in the
//! desktop app. Its limits are listed in `docs/web.md`.
//!
//! Built by `scripts/build-web.sh`; on targets other than the browser this
//! crate is empty.

#[cfg(target_arch = "wasm32")]
mod player;
#[cfg(target_arch = "wasm32")]
mod theme;

#[cfg(target_arch = "wasm32")]
pub use player::start;
