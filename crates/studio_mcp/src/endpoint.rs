//! The running Studio's tools on loopback (DESIGN.md, "Guarding the loopback
//! endpoint"): an agent works with the person on the session the window shows,
//! and each sees the other's changes.
//!
//! `ljos_mcp_core` keeps the guard, as it does for a Ljós inspector: the
//! address, a token per start, `Origin` and `Host`, the capabilities.

use std::{path::Path, sync::Arc};

use ljos_mcp_core::{
    CapSet,
    glue::http::{HttpConfig, HttpError, HttpServer},
};

use crate::StudioServer;

/// The name of the token file in Studio's state directory.
pub const TOKEN_FILE: &str = "mcp.json";

/// Serves `server` with the capabilities granted at launch until the answer
/// is dropped, which also removes the token file.
///
/// # Errors
/// The address, or the token file in `state`.
pub fn serve_agents(
    server: &Arc<StudioServer>,
    caps: CapSet,
    state: &Path,
) -> Result<HttpServer, HttpError> {
    let config = HttpConfig::loopback(caps).token_file(state.join(TOKEN_FILE));
    // Not `clean_up_on_signals`: that ends the process at once, and the
    // program has a preview to close first. It removes the token with the
    // state directory.
    HttpServer::start(Arc::clone(server), config)
}
