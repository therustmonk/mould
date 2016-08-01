use std::error;
use std::result;
use std::fmt;
use rustc_serialize::json::Object;
use std::iter::Iterator;
use session::{Request};

pub type BoxedObjects = Box<Iterator<Item=Object>>;

#[derive(Debug)]
pub enum Error {
    Reject(String),
    Cause(Box<error::Error>),
}

impl Error {
    pub fn reject(msg: &str) -> Self {
        Error::Reject(msg.to_owned())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Reject(ref s) =>
                write!(f, "Rejected by worker {}", s),
            &Error::Cause(ref e) =>
                write!(f, "Rejected with cause {}", e),
        }
    }
}

impl<E: error::Error + 'static> From<E> for Error {
    fn from(e: E) -> Self {
        Error::Cause(Box::new(e))
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

pub type Result<T> = result::Result<T, Error>;

pub trait Worker<T> {
    fn prepare(&mut self, _: &mut T, _: Request) -> Result<Shortcut> {
        Ok(Shortcut::Tuned)
    }
    fn realize(&mut self, _: &mut T, _: Option<Request>) -> Result<Realize> {
        Err(Error::reject("Illegal worker state!"))
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

impl<T> Worker<T> for RejectWorker {
    fn realize(&mut self, _: &mut T, _: Option<Request>)
        -> Result<Realize> {
            Err(Error::Reject(self.reason.clone()))
    }
}

