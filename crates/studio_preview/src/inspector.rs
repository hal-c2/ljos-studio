//! The child's inspector, through `ljos_mcp_core`'s remote client.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use ljos_mcp_core::remote::{BlockingClient, Client, ToolError, Trust, wait_for_token_blocking};
use serde_json::Value;

/// How long one look for the token file lasts before the child is asked
/// whether it has ended: a child that fails to compile its entry ends at
/// once, and its error should not wait out the whole start limit.
const LOOK: Duration = Duration::from_millis(250);

pub(crate) struct Inspector(BlockingClient);

pub(crate) struct Answer {
    pub structured: Value,
    pub text: Option<String>,
    pub images: Vec<(String, Vec<u8>)>,
}

pub(crate) enum Fault {
    /// A tool's refusal, with its code.
    Refused { code: String, message: String },
    /// No answer, or one that is not an answer.
    Transport(String),
}

impl Inspector {
    /// Waits for the child `pid` to write its token file below `dir`, then
    /// connects; gives up at `deadline` or when `ended` says the child is
    /// gone.
    pub fn connect(
        dir: &Path,
        pid: u32,
        deadline: Instant,
        mut ended: impl FnMut() -> bool,
    ) -> Result<Inspector, String> {
        let began = Instant::now();
        let path = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match wait_for_token_blocking(dir, pid, left.min(LOOK)) {
                Ok(path) => break path,
                // Not yet: look again unless the child is gone.
                Err(_) if left > LOOK && !ended() => {}
                Err(e) => {
                    // `e` tells of the last look only; say how long the
                    // whole wait was.
                    return Err(if ended() {
                        "it ended before it answered".to_owned()
                    } else {
                        format!("{e} (waited {:.1} s in all)", began.elapsed().as_secs_f64())
                    });
                }
            }
        };
        let client = Client::from_token_file(&path, Trust::FOUND)
            .map_err(|e| e.to_string())?
            .with_client_info("ljos-studio", env!("CARGO_PKG_VERSION"))
            .blocking();
        client.initialize().map_err(|e| e.to_string())?;
        Ok(Inspector(client))
    }

    pub fn call(&self, tool: &str, args: Value) -> Result<Answer, Fault> {
        match self.0.call_tool(tool, args) {
            Ok(result) => Ok(Answer {
                structured: result.structured,
                text: result.text,
                images: result
                    .images
                    .into_iter()
                    .map(|image| (image.mime, image.data))
                    .collect(),
            }),
            Err(ToolError::Tool { code, message, .. }) => Err(Fault::Refused {
                code: code.unwrap_or_else(|| "error".to_owned()),
                message,
            }),
            Err(e @ ToolError::Protocol { .. }) => Err(Fault::Refused {
                code: e
                    .code()
                    .map_or_else(|| "invalid_argument".to_owned(), |c| c.as_str().to_owned()),
                message: e.to_string(),
            }),
            Err(e) => Err(Fault::Transport(e.to_string())),
        }
    }
}
