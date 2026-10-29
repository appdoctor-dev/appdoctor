//! The I/O runtime. gpui runs its own executors; the HTTP clients
//! (reqwest) need tokio, so network work runs on a small tokio runtime and
//! UI code awaits the result from gpui tasks. A tokio `JoinHandle` is a
//! plain future, so awaiting it from gpui's executor is fine.

use std::future::Future;
use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn init() {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("appdoctor-io")
            .enable_all()
            .build()
            .expect("the I/O runtime starts")
    });
}

/// Runs `future` on the I/O runtime. The returned future resolves on any
/// executor; a panic in the task comes back as an error, not a crash.
pub fn spawn<T, F>(future: F) -> impl Future<Output = anyhow::Result<T>> + Send + 'static
where
    T: Send + 'static,
    F: Future<Output = anyhow::Result<T>> + Send + 'static,
{
    let handle = RUNTIME.get().expect("runtime::init runs first").spawn(future);
    async move {
        handle
            .await
            .map_err(|error| anyhow::anyhow!("background task failed: {error}"))?
    }
}
