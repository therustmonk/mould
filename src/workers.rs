use std::error::Error;
use std::fmt;
use rustc_serialize::json::Object;
use std::iter::Iterator;
use session::{Request};

pub type BoxedObjects = Box<Iterator<Item=Object>>;

#[derive(Debug)]
pub enum WorkerError {
    Reject(String),
    Cause(Box<Error>),
}

impl WorkerError {
    pub fn reject(msg: &str) -> Self {
        WorkerError::Reject(msg.to_owned())
    }
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &WorkerError::Reject(ref s) =>
                write!(f, "Rejected by worker {}", s),
            &WorkerError::Cause(ref e) =>
                write!(f, "Rejected with cause {}", e),
        }
    }
}

impl<E: Error + 'static> From<E> for WorkerError {
    fn from(e: E) -> Self {
        WorkerError::Cause(Box::new(e))
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

pub type WorkerResult<T> = Result<T, WorkerError>;

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

