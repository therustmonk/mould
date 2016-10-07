use std::error;
use std::result;
use rustc_serialize::json::Object;
use std::iter::Iterator;
use session::{Request};

pub type BoxedObjects = Box<Iterator<Item=Object>>;

pub enum Realize {
    ManyItems(BoxedObjects),
    ManyItemsAndDone(BoxedObjects),
    OneItem(Object),
    OneItemAndDone(Object),
    Reject(String),
    Done,
}

pub enum Shortcut {
    Tuned,
    Reject(String),
    Done,
}

pub type Result<T> = result::Result<T, Box<error::Error>>;

pub trait Worker<T> {
    fn prepare(&mut self, _: &mut T, _: Request) -> Result<Shortcut> {
        Ok(Shortcut::Tuned)
    }
    fn realize(&mut self, _: &mut T, _: Option<Request>) -> Result<Realize> {
        unimplemented!();
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
            Ok(Realize::Reject(self.reason.clone()))
    }
}

