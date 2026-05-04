//! Compiler logging with external subscriber support.
//!
//! This module keeps Graphy's internal diagnostics stream available through a
//! simple subscription API so host applications can render line-by-line build
//! progress in their own UI.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Severity level for compiler log lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// A single compiler diagnostic line emitted from Graphy internals.
#[derive(Debug, Clone)]
pub struct CompilerLogLine {
    pub level: CompilerLogLevel,
    pub scope: String,
    pub message: String,
}

type CompilerLogSubscriber = Arc<dyn Fn(&CompilerLogLine) + Send + Sync + 'static>;

struct CompilerLogRegistry {
    next_id: AtomicU64,
    subscribers: Mutex<HashMap<u64, CompilerLogSubscriber>>,
}

fn registry() -> &'static CompilerLogRegistry {
    static REGISTRY: OnceLock<CompilerLogRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| CompilerLogRegistry {
        next_id: AtomicU64::new(1),
        subscribers: Mutex::new(HashMap::new()),
    })
}

/// Opaque handle returned when subscribing to compiler logs.
///
/// Dropping this handle automatically unsubscribes the callback.
pub struct CompilerLogSubscription {
    id: u64,
}

impl Drop for CompilerLogSubscription {
    fn drop(&mut self) {
        let _ = unsubscribe_compiler_logs(self.id);
    }
}

/// Subscribe to detailed compiler log lines emitted by Graphy internals.
///
/// The callback is invoked for each emitted line while the subscription is
/// active. Drop the returned handle to unsubscribe.
pub fn subscribe_compiler_logs<F>(callback: F) -> CompilerLogSubscription
where
    F: Fn(&CompilerLogLine) + Send + Sync + 'static,
{
    let reg = registry();
    let id = reg.next_id.fetch_add(1, Ordering::Relaxed);

    if let Ok(mut subscribers) = reg.subscribers.lock() {
        subscribers.insert(id, Arc::new(callback));
    }

    CompilerLogSubscription { id }
}

/// Explicitly unsubscribe a compiler log subscription by ID.
///
/// Returns `true` if a subscriber was removed.
pub fn unsubscribe_compiler_logs(subscription_id: u64) -> bool {
    let reg = registry();
    if let Ok(mut subscribers) = reg.subscribers.lock() {
        return subscribers.remove(&subscription_id).is_some();
    }
    false
}

fn emit_compiler_log(level: CompilerLogLevel, scope: &str, message: impl Into<String>) {
    let line = CompilerLogLine {
        level,
        scope: scope.to_string(),
        message: message.into(),
    };

    let callbacks = {
        let reg = registry();
        if let Ok(subscribers) = reg.subscribers.lock() {
            subscribers.values().cloned().collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    };

    for callback in callbacks {
        callback(&line);
    }
}

pub fn compiler_trace(scope: &str, message: impl Into<String>) {
    let message = message.into();
    tracing::trace!("[{}] {}", scope, message);
    emit_compiler_log(CompilerLogLevel::Trace, scope, message);
}

pub fn compiler_debug(scope: &str, message: impl Into<String>) {
    let message = message.into();
    tracing::debug!("[{}] {}", scope, message);
    emit_compiler_log(CompilerLogLevel::Debug, scope, message);
}

pub fn compiler_info(scope: &str, message: impl Into<String>) {
    let message = message.into();
    tracing::info!("[{}] {}", scope, message);
    emit_compiler_log(CompilerLogLevel::Info, scope, message);
}

pub fn compiler_warn(scope: &str, message: impl Into<String>) {
    let message = message.into();
    tracing::warn!("[{}] {}", scope, message);
    emit_compiler_log(CompilerLogLevel::Warn, scope, message);
}

pub fn compiler_error(scope: &str, message: impl Into<String>) {
    let message = message.into();
    tracing::error!("[{}] {}", scope, message);
    emit_compiler_log(CompilerLogLevel::Error, scope, message);
}
