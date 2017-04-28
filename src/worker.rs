use std::error;
use std::result;
use session::{Request, Object};

pub enum Realize {
    OneItem(Object),
    OneItemAndDone(Object),
    Reject(String),
    Empty,
    Done,
}

impl<'a> From<&'a str> for Realize {
    fn from(s: &'a str) -> Self {
        Realize::Reject(s.to_owned())
    }
}

impl From<String> for Realize {
    fn from(s: String) -> Self {
        Realize::Reject(s)
    }
}

pub enum Shortcut {
    Tuned,
    Reject(String),
    Done,
}

impl<'a> From<&'a str> for Shortcut {
    fn from(s: &'a str) -> Self {
        Shortcut::Reject(s.to_owned())
    }
}

impl From<String> for Shortcut {
    fn from(s: String) -> Self {
        Shortcut::Reject(s)
    }
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

