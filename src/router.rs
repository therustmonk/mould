use workers::Worker;
use session::Request;

/// Router looks into session or request to build corresponding worker.
pub trait Router<CTX> {
    /// Never return error, but rejecting Worker created
    fn route(&self, context: &CTX, request: &Request) -> Box<Worker<CTX>>;
}

