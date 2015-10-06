use workers::{Worker, RejectWorker, ContextDumpWorker};
use session::Request;

pub trait Handler {
	/// Never return error, but rejecting Worker created
	fn build(&self, request: Request) -> Box<Worker>;
}

pub struct DebugHandler;

impl DebugHandler {
	pub fn new() -> Self {
		DebugHandler
	}
}

impl Handler for DebugHandler {
	fn build(&self, request: Request) -> Box<Worker> {
		if request.action == "context-dump" {
			Box::new(ContextDumpWorker::new())
		} else {
			Box::new(RejectWorker::new("Unknown debug action.".to_owned()))
		}		
	}
}
