use workers::Worker;
use session::Request;

pub trait Handler<CTX> {
    /// Never return error, but rejecting Worker created
    fn build(&self, request: Request) -> Box<Worker<CTX>>;
}

