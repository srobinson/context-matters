//! Panic capture for the MCP stdio loop.
//!
//! [`install_panic_hook`] registers a panic hook that records the
//! payload message, `file:line:column` location, and a captured
//! backtrace into a thread-local buffer. The stdio run loop wraps
//! each request handler in [`futures::FutureExt::catch_unwind`] and,
//! on a caught panic, calls [`take_last_panic`] to surface the
//! snapshot through a JSON-RPC `-32603` error response.
//!
//! Why a thread-local. The run loop is single-worker by construction
//! (see the docstring on `McpServer::run`). A panic fires the hook
//! synchronously on whatever thread the panic originated on, and the
//! `catch_unwind` site that reads the thread-local polls the wrapped
//! future on the same thread. The hook writes and the catch site
//! reads are therefore always colocated, so a cheap thread-local
//! beats a global `Mutex` without any correctness trade-off.

use std::cell::RefCell;
use std::panic;
use std::sync::Once;

/// Captured panic information surfaced to the MCP client.
#[derive(Debug, Clone)]
pub struct PanicSnapshot {
    /// The panic payload message (from `panic!("…")`), stringified.
    pub message: String,
    /// `file:line:column` of the panic site, when available.
    pub location: Option<String>,
    /// Captured backtrace (forced, so it does not depend on
    /// `RUST_BACKTRACE`). May contain many frames.
    pub backtrace: String,
}

thread_local! {
    static LAST_PANIC: RefCell<Option<PanicSnapshot>> = const { RefCell::new(None) };
}

static HOOK_INSTALLED: Once = Once::new();

/// Install the MCP panic hook exactly once for the current process.
///
/// Replaces the default hook with a wrapper that:
/// 1. Stringifies the panic payload (both `&'static str` and `String`
///    payloads are recognised; other types become a placeholder).
/// 2. Captures `file:line:column` from [`std::panic::PanicHookInfo`].
/// 3. Forces a [`std::backtrace::Backtrace`] capture regardless of the
///    `RUST_BACKTRACE` env var, so operators always get a trace on
///    the (rare) panic path.
/// 4. Logs a structured `tracing::error!` event.
/// 5. Stashes the snapshot into the thread-local for the catch site.
/// 6. Delegates to the previously-installed hook so any default
///    stderr output (e.g. the `note: run with RUST_BACKTRACE=1`
///    prompt) still appears.
pub fn install_panic_hook() {
    HOOK_INSTALLED.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_owned());
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();

            tracing::error!(
                panic.message = %message,
                panic.location = location.as_deref().unwrap_or("<unknown>"),
                "mcp handler panicked",
            );

            LAST_PANIC.with(|cell| {
                *cell.borrow_mut() = Some(PanicSnapshot {
                    message: message.clone(),
                    location: location.clone(),
                    backtrace,
                });
            });

            default_hook(info);
        }));
    });
}

/// Pop the most recent panic snapshot for the current thread, if any.
///
/// Called from the `catch_unwind` site in the run loop immediately
/// after an unwind is observed. Clears the slot so a subsequent
/// non-panic request does not accidentally inherit a stale snapshot.
pub fn take_last_panic() -> Option<PanicSnapshot> {
    LAST_PANIC.with(|cell| cell.borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_string_payload_and_location() {
        install_panic_hook();
        // Clear any stale state from prior tests in the same thread.
        let _ = take_last_panic();

        let result = std::panic::catch_unwind(|| {
            panic!("boom");
        });
        assert!(result.is_err());

        let snap = take_last_panic().expect("snapshot should be populated");
        assert_eq!(snap.message, "boom");
        // Location should at least mention this file.
        let loc = snap.location.expect("location should be captured");
        assert!(loc.contains("panic_guard.rs"), "unexpected loc: {loc}");
        // Backtrace is forced, so it should contain at least one frame name.
        assert!(!snap.backtrace.is_empty());
    }

    #[test]
    fn take_last_panic_clears_slot() {
        install_panic_hook();
        let _ = take_last_panic();

        let _ = std::panic::catch_unwind(|| {
            panic!("once");
        });
        assert!(take_last_panic().is_some());
        // Second take without a new panic is None.
        assert!(take_last_panic().is_none());
    }
}
