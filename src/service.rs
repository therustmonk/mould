use session::Request;
use worker::Worker;

/// Service looks into session or request to build corresponding worker.
///
/// There is `Send` restriction, because reference to services sends through
/// thread boundaries to user's session (connection) routine.
/// It needs `Sync`, because service get access from multiple threads.
pub trait Service<T>: Send + Sync + 'static {
    /// Never return error, but rejecting Worker created
    fn route(&self, request: &Request) -> Box<Worker<T>>;
}

