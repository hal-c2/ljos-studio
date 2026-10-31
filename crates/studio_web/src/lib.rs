//! Ljós Studio in a browser (DESIGN.md section 7, S5).
//!
//! The session and the MCP tools are the native ones, built for wasm32:
//! [`WebStore`] holds the project for the browser's storage, a
//! [`WebPreviewHost`] runs previews in a page over [`protocol`], and
//! [`Face`] is what JavaScript calls (`web/studio.js`).

pub mod face;
pub mod host;
pub mod protocol;
pub mod store;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use face::{Face, FaceError};
pub use host::{Port, PortError, WebPreviewHost};
pub use store::{Pending, WebStore};
