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
use std::fmt;
use std::error;
use std::default::Default;
use std::ops::{Deref, DerefMut};
use rustc_serialize::json::{Json, Object};
use flow::{self, Flow};

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

pub struct Request {
    pub action: String,
    pub payload: Object,
}

/// Interface for access to payload of request.
pub trait Extractor<T> {
    fn extract(&mut self, key: &str) -> Option<T>;
}

impl Extractor<Object> for Request {
    fn extract(&mut self, key: &str) -> Option<Object> {
        self.payload.remove(key).and_then(Json::into_object)
    }
}

impl Extractor<String> for Request {
    fn extract(&mut self, key: &str) -> Option<String> {
        self.payload.remove(key).as_ref().and_then(Json::as_string).map(str::to_owned)
    }
}

impl Extractor<i64> for Request {
    fn extract(&mut self, key: &str) -> Option<i64> {
        self.payload.remove(key).as_ref().and_then(Json::as_i64)
    }
}

impl Extractor<f64> for Request {
    fn extract(&mut self, key: &str) -> Option<f64> {
        self.payload.remove(key).as_ref().and_then(Json::as_f64)
    }
}

pub type TaskId = usize;

pub enum Input {
    Request(String, Request),
    Next(Option<Request>),
    Suspend,
    Resume(TaskId),
}

pub enum Output {
    Ready,
    Item(Object),
    Done,
    Reject(String),
    Fail(String),
    Suspended(TaskId),
}

pub enum Alternative<T, U> {
    Usual(T),
    Unusual(U),
}

#[derive(Debug)]
pub enum Error {
    IllegalJsonFormat,
    IllegalEventType,
    IllegalEventName(String),
    IllegalMessage,
    IllegalDataFormat,
    IllegalRequestFormat,
    ServiceNotFound,
    DataNotProvided,
    UnexpectedState,
    Canceled,
    ConnectionClosed,
    ConnectorFail(flow::Error),
    WorkerFailed(Box<error::Error>),
    WorkerNotFound,
    CannotSuspend,
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            IllegalJsonFormat => "illegal json format",
            IllegalEventType => "illegal event type",
            IllegalEventName(_) => "illegal event name",
            IllegalMessage => "illegal message",
            IllegalDataFormat => "illegal data format",
            IllegalRequestFormat => "illegal request format",
            ServiceNotFound => "service not found",
            DataNotProvided => "data not provided",
            UnexpectedState => "unexpected state",
            Canceled => "cancelled",
            ConnectionClosed => "connection closed",
            ConnectorFail(_) => "flow fail",
            WorkerFailed(ref cause) => cause.description(),
            WorkerNotFound => "task not found",
            CannotSuspend => "cannot suspend worker",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        if let Error::WorkerFailed(ref cause) = *self {
            Some(cause.as_ref())
        } else {
            None
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Error::WorkerFailed(ref cause) = *self {
            cause.fmt(f)
        } else {
            use std::error::Error;
            f.write_str(self.description())
        }
    }
}


impl From<Box<error::Error>> for Error {
    fn from(error: Box<error::Error>) -> Self {
        Error::WorkerFailed(error)
    }
}

impl From<flow::Error> for Error {
    fn from(error: flow::Error) -> Self {
        Error::ConnectorFail(error)
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

    fn recv(&mut self) -> Result<Input, Error> {
        match self.client.pull()? {
            Some(content) => {
                debug!("Recv => {}", content);
                if let Ok(Json::Object(mut data)) = Json::from_str(&content) {
                    if let Some(Json::String(event)) = data.remove("event") {
                        if event == "request" {
                            match data.remove("data") {
                                Some(Json::Object(mut data)) => {
                                    let service = match data.remove("service") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(Error::IllegalRequestFormat),
                                    };
                                    let action = match data.remove("action") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(Error::IllegalRequestFormat),
                                    };
                                    let payload = match data.remove("payload") {
                                        Some(Json::Object(data)) => data,
                                        _ => return Err(Error::IllegalRequestFormat),
                                    };
                                    let request = Request {
                                        action: action,
                                        payload: payload,
                                    };
                                    Ok(Input::Request(service, request))
                                },
                                Some(_) => Err(Error::IllegalDataFormat),
                                None => Err(Error::DataNotProvided),
                            }
                        } else if event == "next" {
                            let request = match data.remove("data") {
                                Some(Json::Object(mut data)) => {
                                    let action = match data.remove("action") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(Error::IllegalRequestFormat),
                                    };
                                    let payload = match data.remove("payload") {
                                        Some(Json::Object(data)) => data,
                                        _ => return Err(Error::IllegalRequestFormat),
                                    };
                                    let request = Request {
                                        action: action,
                                        payload: payload,
                                    };
                                    Some(request)
                                },
                                Some(Json::Null) => None,
                                Some(_) => {
                                    return Err(Error::IllegalDataFormat);
                                },
                                None => None,
                            };
                            Ok(Input::Next(request))
                        } else if event == "resume" {
                            if let Some(Json::U64(task_id)) = data.remove("data") {
                                Ok(Input::Resume(task_id as usize))
                            } else {
                                Err(Error::IllegalDataFormat)
                            }
                        } else if event == "suspend" {
                            Ok(Input::Suspend)
                        } else if event == "cancel" {
                            Err(Error::Canceled)
                        } else {
                            Err(Error::IllegalEventName(event))
                        }
                    } else {
                        Err(Error::IllegalEventType)
                    }

                } else {
                    Err(Error::IllegalJsonFormat)
                }
            },
            None => Err(Error::ConnectionClosed),
        }
    }

    pub fn recv_request_or_resume(&mut self) -> Result<Alternative<(String, Request), TaskId>, Error> {
        match self.recv() {
            Ok(Input::Request(service, request)) => Ok(Alternative::Usual((service, request))),
            Ok(Input::Resume(task_id)) => Ok(Alternative::Unusual(task_id)),
            Ok(_) => Err(Error::UnexpectedState),
            Err(ie) => Err(ie),
        }
    }

    pub fn recv_next_or_suspend(&mut self) -> Result<Alternative<Option<Request>, ()>, Error> {
        match self.recv() {
            Ok(Input::Next(req)) => Ok(Alternative::Usual(req)),
            Ok(Input::Suspend) => Ok(Alternative::Unusual(())),
            Ok(_) => Err(Error::UnexpectedState),
            Err(ie) => Err(ie),
        }
    }

    pub fn send(&mut self, out: Output) -> Result<(), Error> {
        let json = match out {
            Output::Item(data) =>
                mould_json!({"event" => "item", "data" => data}),
            Output::Ready =>
                mould_json!({"event" => "ready"}),
            Output::Done =>
                mould_json!({"event" => "done"}),
            Output::Reject(message) =>
                mould_json!({"event" => "reject", "data" => message}),
            Output::Fail(message) =>
                mould_json!({"event" => "fail", "data" => message}),
            Output::Suspended(task_id) =>
                mould_json!({"event" => "suspended", "data" => task_id}),
        };
        let content = json.to_string();
        debug!("Send <= {}", content);
        self.client.push(content).map_err(Error::from)
    }

}
