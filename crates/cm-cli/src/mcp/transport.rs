//! Stdio response writing for the MCP JSON-RPC transport.

use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::protocol::{JsonRpcError, JsonRpcResponse};

/// Outcome of a single response write. Used by [`write_response`] to
/// tell the run loop whether the transport is still usable.
pub(super) enum WriteOutcome {
    /// Response written, or the write was a non-fatal, logged no-op.
    Ok,
    /// Peer closed the pipe. The run loop should exit cleanly.
    BrokenPipe,
}

impl WriteOutcome {
    pub(super) fn is_broken_pipe(&self) -> bool {
        matches!(self, WriteOutcome::BrokenPipe)
    }
}

/// Write a JSON-RPC response to stdout with error-isolated framing.
///
/// Never propagates errors out of the run loop:
/// * Serialization failure (`serde_json::to_string` returning `Err`)
///   emits a fallback internal-error envelope keyed off the original
///   request id. If even the fallback fails to serialize, a static
///   last-resort JSON string is written instead.
/// * `BrokenPipe` on write returns [`WriteOutcome::BrokenPipe`] so the
///   loop can exit cleanly.
/// * Any other I/O error is logged via `tracing::error!` and swallowed;
///   the run loop continues on the next iteration.
pub(super) async fn write_response<W>(stdout: &mut W, resp: &JsonRpcResponse) -> WriteOutcome
where
    W: AsyncWrite + Unpin,
{
    let serialized = match serde_json::to_string(resp) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "response serialization failed, emitting fallback");
            let fallback = JsonRpcResponse {
                jsonrpc: "2.0".to_owned(),
                id: resp.id.clone(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Internal error: response serialization failed: {e}"),
                    data: None,
                }),
            };
            serde_json::to_string(&fallback).unwrap_or_else(|_| {
                r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"serialization failure"}}"#
                    .to_owned()
            })
        }
    };

    let write_result = async {
        stdout.write_all(serialized.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await
    }
    .await;

    match write_result {
        Ok(()) => WriteOutcome::Ok,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => WriteOutcome::BrokenPipe,
        Err(e) => {
            tracing::error!(error = %e, "stdout write failed, continuing");
            WriteOutcome::Ok
        }
    }
}
