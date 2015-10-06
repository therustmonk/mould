pub use rustc_serialize::json::{Json, Object};
use session::ContextMap;

pub type WorkerResult = Result<Option<Box<Iterator<Item=Object>>>, String>;

pub trait Worker {
    fn next(&mut self, context: &mut ContextMap) -> WorkerResult;
}


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


pub struct KeyDropWorker {
	keys: Vec<String>,
}

impl KeyDropWorker {
	pub fn new(keys: Vec<String>) -> Self {
		KeyDropWorker {keys: keys}
	}
}

impl Worker for KeyDropWorker {
    fn next(&mut self, context: &mut ContextMap) -> WorkerResult {
 		for key in &self.keys {
 			context.remove(key);
 		}
        Ok(None)
    }
}


pub struct ContextDumpWorker {
	done: bool,
}

impl ContextDumpWorker {
	pub fn new() -> Self {
		ContextDumpWorker {done: false}
	}
}

impl Worker for ContextDumpWorker {
    fn next(&mut self, context: &mut ContextMap) -> WorkerResult {
    	if self.done {
    		Ok(None)	
    	} else {
			let mut object = Object::new();
			for (key, val) in context.iter() {
			    object.insert(key.clone(), Json::String(val.clone()));
			}
			self.done = true;
			Ok(Some(Box::new(vec![object].into_iter())))
    	}
    }
}

