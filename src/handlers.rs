use workers::{Worker};
//use workers::{RejectWorker, ContextDumpWorker};
use session::{Request};
//use session::{ContextMap};

pub trait Handler<CTX> {
	/// Never return error, but rejecting Worker created
	fn build(&self, request: Request) -> Box<Worker<CTX>>;
}

/* TODO Replace with trait ContextDumper

pub struct DebugHandler;

impl DebugHandler {
	pub fn new() -> Self {
		DebugHandler
	}
}

impl<CTX: ContextMap> Handler for DebugHandler {
	fn build(&self, request: Request) -> Box<Worker<CTX>> {
		if request.action == "context-dump" {
			Box::new(ContextDumpWorker::new())
		} else {
			Box::new(RejectWorker::new("Unknown debug action.".to_owned()))
		}		
	}
}
*/