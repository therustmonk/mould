pub use rustc_serialize::json::{Json, Object};

pub use session::{Request, ContextMap};

pub type WorkerResult = Result<Option<Box<Iterator<Item=Object>>>, String>;

pub trait Worker {
    fn next(&mut self, context: &mut ContextMap) -> WorkerResult;
}

pub trait Handler {
	/// Never return error, but rejecting Worker created
	fn build(&self, request: Request) -> Box<Worker>;
}

pub mod workers {
	use super::*;

	pub struct RejectWorker {
		reason: String,
	}

	impl RejectWorker {
		pub fn new(reason: String) -> Self {
			RejectWorker {reason: reason}
		}
	}

	impl Worker for RejectWorker {
	    fn next(&mut self, _: &mut ContextMap) -> WorkerResult {
	        Err(self.reason.clone())
	    }
	}


	pub struct KeyCleaner {
		keys: Vec<String>,
	}

	impl KeyCleaner {
		pub fn new(keys: Vec<String>) -> Self {
			KeyCleaner {keys: keys}
		}
	}

	impl Worker for KeyCleaner {
	    fn next(&mut self, context: &mut ContextMap) -> WorkerResult {
	 		for key in &self.keys {
	 			context.remove(key);
	 		}
	        Ok(None)
	    }
	}
}