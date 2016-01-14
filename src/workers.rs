use std::error::Error;
pub use rustc_serialize::json::{Json, Object};
use std::iter::Iterator;
use session::{Request};

pub type BoxedObjects = Box<Iterator<Item=Object>>;

pub enum WorkerError {
    Reject(String)
}

pub type WorkerResult<T> = Result<T, WorkerError>;

impl<E: Error> From<E> for WorkerError {
    fn from(e: E) -> Self {
        WorkerError::Reject(e.description().to_string())
    }
}

pub enum Realize {
    ManyItems(BoxedObjects),
    ManyItemsAndDone(BoxedObjects),
    OneItem(Object),
    OneItemAndDone(Object),
    Done,
}

pub enum Shortcut {
    Tuned,
    Done,
}

pub trait Worker<CTX> {
    fn shortcut(&mut self, _: &mut CTX)
        -> WorkerResult<Shortcut> {
            Ok(Shortcut::Tuned)
    }
    fn realize(&mut self, _: &mut CTX, _: Option<Request>)
        -> WorkerResult<Realize> {
            Err(WorkerError::Reject("Worker unreachable state.".to_string()))
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
    fn shortcut(&mut self, _: &mut CTX)
        -> WorkerResult<Shortcut> {
            Err(WorkerError::Reject(self.reason.clone()))
    }
}

