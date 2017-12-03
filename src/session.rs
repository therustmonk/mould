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
use flow::{self, Flow};

pub trait Builder<T: Session>: Send + Sync + 'static {
    fn build(&self) -> T;
}

pub struct DefaultBuilder {}

impl<T: Session + Default> Builder<T> for DefaultBuilder {
    fn build(&self) -> T {
        T::default()
    }
}

pub trait Session: 'static {}

pub struct Context<T: Session, R: Flow> {
    client: R,
    session: T,
}

pub type Request = Value;

pub type TaskId = usize;

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "lowercase")]
pub enum Input {
    Request {
        service: String,
        action: String,
        payload: Value,
    },
    Next(Value),
    Suspend,
    Resume(TaskId),
    Cancel,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "lowercase")]
pub enum Output {
    Ready,
    Item(Value),
    Done,
    Fail(String),
    Suspended(TaskId),
}

pub enum Alternative<T, U> {
    Usual(T),
    Unusual(U),
}

error_chain! {
    links {
        Flow(flow::Error, flow::ErrorKind);
    }
    foreign_links {
        Serde(serde_json::Error);
    }
    errors {
        ConnectionClosed
        UnexpectedState
        Canceled
    }
}

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

    fn recv(&mut self) -> Result<Input> {
        let content = self.client.pull()?.ok_or(ErrorKind::ConnectionClosed)?;
        debug!("Recv => {}", content);
        let input = serde_json::from_str(&content)?;
        if let Input::Cancel = input {
            Err(ErrorKind::Canceled.into())
        } else {
            Ok(input)
        }
    }

    pub fn recv_request_or_resume(
        &mut self,
    ) -> Result<Alternative<(String, String, Request), TaskId>> {
        match self.recv()? {
            Input::Request {
                service,
                action,
                payload,
            } => Ok(Alternative::Usual((service, action, payload))),
            Input::Resume(task_id) => Ok(Alternative::Unusual(task_id)),
            _ => Err(ErrorKind::UnexpectedState.into()),
        }
    }

    pub fn recv_next_or_suspend(&mut self) -> Result<Alternative<Request, ()>> {
        match self.recv()? {
            Input::Next(req) => Ok(Alternative::Usual(req)),
            Input::Suspend => Ok(Alternative::Unusual(())),
            _ => Err(ErrorKind::UnexpectedState.into()),
        }
    }

    pub fn send(&mut self, out: Output) -> Result<()> {
        let content = serde_json::to_string(&out)?;
        debug!("Send <= {}", content);
        self.client.push(content).map_err(Error::from)
    }
}
