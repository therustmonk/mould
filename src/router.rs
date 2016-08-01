use session::Request;
use worker::Worker;

/// Router looks into session or request to build corresponding worker.
pub trait Router<T> {
    /// Never return error, but rejecting Worker created
    fn route(&self, request: &Request) -> Box<Worker<T>>;
}

