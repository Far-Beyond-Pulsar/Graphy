//! # Thread Pool Management
//!
//! Pre-configured thread pools for parallel graph processing.
//! Eliminates cold-start overhead by warming up threads in advance.

use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::OnceLock;

/// Global thread pool for graph analysis
static GRAPH_POOL: OnceLock<ThreadPool> = OnceLock::new();

/// Configuration for the graph processing thread pool
#[derive(Debug, Clone)]
pub struct ThreadPoolConfig {
    /// Number of threads to use (None = auto-detect)
    pub num_threads: Option<usize>,
    
    /// Stack size per thread in bytes
    pub stack_size: Option<usize>,
    
    /// Thread name prefix
    pub thread_name: String,
    
    /// Whether to use LIFO work stealing (better cache locality)
    pub breadth_first: bool,
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        Self {
            num_threads: None, // Auto-detect
            stack_size: Some(2 * 1024 * 1024), // 2MB per thread
            thread_name: "graphy-worker".to_string(),
            breadth_first: false, // LIFO for better cache locality
        }
    }
}

impl ThreadPoolConfig {
    /// Create a new config with automatic thread count
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the number of threads
    pub fn with_num_threads(mut self, num: usize) -> Self {
        self.num_threads = Some(num);
        self
    }
    
    /// Set the stack size per thread
    pub fn with_stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }
    
    /// Enable breadth-first work stealing
    pub fn with_breadth_first(mut self, enabled: bool) -> Self {
        self.breadth_first = enabled;
        self
    }
    
    /// Get the actual number of threads that will be used
    pub fn get_num_threads(&self) -> usize {
        self.num_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        })
    }
}

/// Initialize the global thread pool with custom configuration
///
/// This should be called early in your application (e.g., in main())
/// to pre-warm the thread pool and eliminate cold-start overhead.
///
/// # Example
///
/// ```rust,no_run
/// use graphy::parallel::{init_thread_pool, ThreadPoolConfig};
///
/// fn main() {
///     // Initialize with 8 threads
///     let config = ThreadPoolConfig::new().with_num_threads(8);
///     init_thread_pool(config).expect("Failed to initialize thread pool");
///     
///     // Now parallel operations have zero startup cost
/// }
/// ```
pub fn init_thread_pool(config: ThreadPoolConfig) -> Result<(), String> {
    let num_threads = config.get_num_threads();
    
    tracing::info!(
        "[THREADPOOL] Initializing with {} threads (stack: {:?}, breadth_first: {})",
        num_threads,
        config.stack_size,
        config.breadth_first
    );
    
    let mut builder = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .thread_name(move |idx| format!("{}-{}", config.thread_name, idx));
    
    if let Some(stack_size) = config.stack_size {
        builder = builder.stack_size(stack_size);
    }
    
    if config.breadth_first {
        // Note: breadth_first is deprecated, but we keep the option for compatibility
        #[allow(deprecated)]
        {
            builder = builder.breadth_first();
        }
    }
    
    let pool = builder.build().map_err(|e| format!("Failed to build thread pool: {}", e))?;
    
    // Warm up the pool by running a dummy task on each thread
    pool.install(|| {
        (0..num_threads).into_par_iter().for_each(|_| {
            // Touch the thread to ensure it's spawned
            std::hint::black_box(42);
        });
    });
    
    tracing::info!("[THREADPOOL] Thread pool warmed up and ready");
    
    GRAPH_POOL.set(pool).map_err(|_| "Thread pool already initialized".to_string())?;
    
    Ok(())
}

/// Get or initialize the global thread pool
///
/// If the pool hasn't been initialized, creates one with default settings.
pub fn get_thread_pool() -> &'static ThreadPool {
    GRAPH_POOL.get_or_init(|| {
        tracing::debug!("[THREADPOOL] Lazy initializing with defaults");
        
        let config = ThreadPoolConfig::default();
        let num_threads = config.get_num_threads();
        
        ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(move |idx| format!("graphy-worker-{}", idx))
            .stack_size(2 * 1024 * 1024)
            .build()
            .expect("Failed to build default thread pool")
    })
}

/// Check if the thread pool has been initialized
pub fn is_initialized() -> bool {
    GRAPH_POOL.get().is_some()
}

/// Get the number of threads in the pool
pub fn num_threads() -> usize {
    get_thread_pool().current_num_threads()
}

// Re-export rayon's parallel iterator for convenience
use rayon::prelude::*;
pub use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_thread_pool_config() {
        let config = ThreadPoolConfig::new()
            .with_num_threads(8)
            .with_stack_size(4 * 1024 * 1024);
        
        assert_eq!(config.get_num_threads(), 8);
        assert_eq!(config.stack_size, Some(4 * 1024 * 1024));
    }
}
