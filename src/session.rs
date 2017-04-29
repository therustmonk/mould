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
use serde_json::{self, Map, from_str, from_value};
pub use serde_json::Value;
use flow::{self, Flow};

pub type Object = Map<String, Value>;
pub type Array = Vec<Value>;

pub trait Builder<T: Session>: Send + Sync + 'static {
    fn build(&self) -> T;
}

pub struct DefaultBuilder { }

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

/*
pub struct Request {
    pub action: String,
    pub payload: Object,
}
*/

pub type TaskId = usize;

pub enum Input {
    Request(String, String, Value),
    Next(Option<Value>),
    Suspend,
    Resume(TaskId),
}

pub enum Output {
    Ready,
    Item(Value),
    Done,
    Reject(String),
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
        IllegalEventName(s: String)
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
        let value: Value = from_str(&content)?;
        let mut object: Object = from_value(value)?;
        let event: String = from_value(object.remove("event").unwrap_or_default())?;
        match event.as_ref() {
            "request" => {
                let mut data: Object = from_value(object.remove("data").unwrap_or_default())?;
                let service: String = from_value(data.remove("service").unwrap_or_default())?;
                let action: String = from_value(data.remove("action").unwrap_or_default())?;
                let payload: Value = from_value(data.remove("payload").unwrap_or_default())?;
                Ok(Input::Request(service, action, payload))
            },
            "next" => {
                let data: Option<Value> = from_value(object.remove("data").unwrap_or_default())?;
                Ok(Input::Next(data))
            },
            "resume" => {
                let task_id: u64 = from_value(object.remove("data").unwrap_or_default())?;
                Ok(Input::Resume(task_id as usize))
            },
            "suspend" => {
                Ok(Input::Suspend)
            },
            "cancel" => {
                Err(ErrorKind::Canceled.into())
            },
            event => {
                Err(ErrorKind::IllegalEventName(event.into()).into())
            },
        }
    }

    pub fn recv_request_or_resume(&mut self) -> Result<Alternative<(String, String, Request), TaskId>> {
        match self.recv() {
            Ok(Input::Request(service, action, request)) => Ok(Alternative::Usual((service, action, request))),
            Ok(Input::Resume(task_id)) => Ok(Alternative::Unusual(task_id)),
            Ok(_) => Err(ErrorKind::UnexpectedState.into()),
            Err(ie) => Err(ie),
        }
    }

    pub fn recv_next_or_suspend(&mut self) -> Result<Alternative<Option<Request>, ()>> {
        match self.recv() {
            Ok(Input::Next(req)) => Ok(Alternative::Usual(req)),
            Ok(Input::Suspend) => Ok(Alternative::Unusual(())),
            Ok(_) => Err(ErrorKind::UnexpectedState.into()),
            Err(ie) => Err(ie),
        }
    }

    pub fn send(&mut self, out: Output) -> Result<()> {
        let json = match out {
            // TODO Use Event & `serde` here
            Output::Item(data) =>
                json!({"event": "item", "data": data}),
            Output::Ready =>
                json!({"event": "ready"}),
            Output::Done =>
                json!({"event": "done"}),
            Output::Reject(message) =>
                json!({"event": "reject", "data": message}),
            Output::Fail(message) =>
                json!({"event": "fail", "data": message}),
            Output::Suspended(task_id) =>
                json!({"event": "suspended", "data": task_id}),
        };
        let content = json.to_string();
        debug!("Send <= {}", content);
        self.client.push(content).map_err(Error::from)
    }

}
