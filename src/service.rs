use session::Request;
use worker::Worker;

/// Service looks into session or request to build corresponding worker.
///
/// There aren't `Sync` and `Send` markers, because there isn't any
/// mutable operation. This object is used in read-only way.
pub trait Service<T>: 'static {
    /// Never return error, but rejecting Worker created
    fn route(&self, request: &Request) -> Box<Worker<T>>;
}

