//! Context module contains protocol implementation.
//!
//! Server can receive the following messages from clients:
//!
//! * {"event": "request", "data": {"action": "what_to_do", "payload": {...}}}
//! * {"event": "next"}
//! * {"event": "cancel"}
//!
//! Server responds to clients the following messages:
//!
//! * {"event": "ready"}
//! * {"event": "item"}
//! * {"event": "done"}
//! * {"event": "reject", "data": {"message": "text_of_message"}}

use std::str;
use std::default::Default;
use std::ops::{Deref, DerefMut};
use serde_json;
pub use serde_json::Value;
use futures::{Poll, Async};
use flow::{self, Flow};

/// Builds user's session and attaches resources like:
/// database connections, channels, counters.
pub trait Builder<T: Session>: Send + Sync + 'static {
    fn build(&self) -> T;
}

pub struct DefaultBuilder;

impl<T: Session + Default> Builder<T> for DefaultBuilder {
    fn build(&self) -> T {
        T::default()
    }
}

pub trait Session: 'static {}

/// Binds client connection instance to session
pub struct Context<T: Session, R: Flow> {
    client: R,
    session: T,
}

pub type Request = Value;

pub type TaskId = u64;

#[derive(Serialize, Deserialize)]
pub struct Input {
    pub id: TaskId,
    pub service: String,
    pub action: String,
    pub payload: Value,
}

#[derive(Serialize, Deserialize)]
pub struct Output {
    pub id: TaskId,
    pub result: TaskResult,
}

#[derive(Serialize, Deserialize)]
//#[serde(tag = "event", content = "data", rename_all = "lowercase")]
pub enum TaskResult {
    Item(Value),
    Fail(String),
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "connection closed")]
    ConnectionClosed,
    #[fail(display = "unexpected state")]
    UnexpectedState,
    #[fail(display = "flow error")]
    FlowBroken(#[cause] flow::Error),
    #[fail(display = "serde error")]
    SerdeFailed(#[cause] serde_json::Error),
}

impl From<flow::Error> for Error {
    fn from(cause: flow::Error) -> Self {
        Error::FlowBroken(cause)
    }
}

impl From<serde_json::Error> for Error {
    fn from(cause: serde_json::Error) -> Self {
        Error::SerdeFailed(cause)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

impl<T: Session, R: Flow> Deref for Context<T, R> {
    type Target = T;

    fn deref<'a>(&'a self) -> &'a T {
        &self.session
    }
}

impl<T: Session, R: Flow> DerefMut for Context<T, R> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        &mut self.session
    }
}

impl<T: Session, R: Flow> Context<T, R> {
    pub fn new(client: R, session: T) -> Self {
        Context {
            client: client,
            session: session,
        }
    }

    pub fn recv(
        &mut self,
    ) -> Poll<Input, Error> {
        let content = try_ready!(self.client.pull()).ok_or(Error::ConnectionClosed)?;
        debug!("Recv => {}", content);
        let input = serde_json::from_str(&content)?;
        Ok(Async::Ready(input))
    }

    pub fn send(&mut self, out: Output) -> Result<()> {
        let content = serde_json::to_string(&out)?;
        debug!("Send <= {}", content);
        self.client.push(content).map_err(Error::from)
    }
}
