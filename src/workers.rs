pub use rustc_serialize::json::{Json, Object};
use std::iter::Iterator;


pub type BoxedObjects = Box<Iterator<Item=Object>>;

pub enum WorkerResult {
    ManyItems(BoxedObjects),
    ManyItemsAndDone(BoxedObjects),
    OneItem(Object),
    OneItemAndDone(Object),
    Reject(String),
    Done,
}

pub trait Worker<CTX> {
    fn realize(&mut self, context: &mut CTX) -> WorkerResult;
}

impl<F, CTX> Worker<CTX> for F where F: FnMut(&mut CTX) -> WorkerResult {
    fn realize(&mut self, context: &mut CTX) -> WorkerResult {
        self(context)
    }
}

pub struct RejectWorker {
	reason: String,
}

impl RejectWorker {
	pub fn new(reason: String) -> Self {
		RejectWorker {reason: reason}
	}
}

impl<CTX> Worker<CTX> for RejectWorker {
    fn realize(&mut self, _: &mut CTX) -> WorkerResult {
        WorkerResult::Reject(self.reason.clone())
    }
}


/*
pub struct KeyDropWorker {
	keys: Vec<String>,
}

impl KeyDropWorker {
	pub fn new(keys: Vec<String>) -> Self {
		KeyDropWorker {keys: keys}
	}
}

impl Worker for KeyDropWorker {
    fn realize(&mut self, context: &mut ContextMap) -> WorkerResult {
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
    fn realize(&mut self, context: &mut ContextMap) -> WorkerResult {
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
*/