//! Tests for ThreadPoolConfig and parallel thread pool management.

use graphy::parallel::*;

// ===========================================================================
// ThreadPoolConfig - Construction
// ===========================================================================

#[test]
fn config_default() {
    let config = ThreadPoolConfig::new();
    assert!(config.num_threads.is_none());
    assert_eq!(config.stack_size, Some(2 * 1024 * 1024));
    assert_eq!(config.thread_name, "graphy-worker");
    assert!(!config.breadth_first);
}

#[test]
fn config_with_num_threads() {
    let config = ThreadPoolConfig::new().with_num_threads(4);
    assert_eq!(config.num_threads, Some(4));
    assert_eq!(config.get_num_threads(), 4);
}

#[test]
fn config_with_stack_size() {
    let config = ThreadPoolConfig::new().with_stack_size(8 * 1024 * 1024);
    assert_eq!(config.stack_size, Some(8 * 1024 * 1024));
}

#[test]
fn config_with_breadth_first() {
    let config = ThreadPoolConfig::new().with_breadth_first(true);
    assert!(config.breadth_first);
}

#[test]
fn config_chained_builder() {
    let config = ThreadPoolConfig::new()
        .with_num_threads(16)
        .with_stack_size(4 * 1024 * 1024)
        .with_breadth_first(true);

    assert_eq!(config.get_num_threads(), 16);
    assert_eq!(config.stack_size, Some(4 * 1024 * 1024));
    assert!(config.breadth_first);
}

#[test]
fn config_get_num_threads_auto_detect() {
    let config = ThreadPoolConfig::new();
    // Auto-detect should return >= 1
    assert!(config.get_num_threads() >= 1);
}

#[test]
fn config_clone() {
    let config = ThreadPoolConfig::new().with_num_threads(8);
    let cloned = config.clone();
    assert_eq!(cloned.get_num_threads(), 8);
    assert_eq!(cloned.thread_name, "graphy-worker");
}

#[test]
fn config_debug() {
    let config = ThreadPoolConfig::new().with_num_threads(4);
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("4"));
    assert!(debug_str.contains("graphy-worker"));
}

// ===========================================================================
// Thread pool - Lazy init & queries
// ===========================================================================

#[test]
fn thread_pool_get_or_init() {
    // get_thread_pool() should always succeed (lazy init)
    let pool = get_thread_pool();
    assert!(pool.current_num_threads() >= 1);
}

#[test]
fn thread_pool_num_threads() {
    let n = num_threads();
    assert!(n >= 1);
}

#[test]
fn thread_pool_is_initialized_after_get() {
    let _ = get_thread_pool();
    assert!(is_initialized());
}
