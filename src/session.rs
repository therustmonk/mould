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
use websocket::Message;
use websocket::Client as WSClient;
use websocket::message::Type;
use websocket::sender::Sender;
use websocket::receiver::Receiver;
use websocket::dataframe::DataFrame;
use websocket::stream::WebSocketStream;
use websocket::ws::receiver::Receiver as WSReceiver;

pub type Client = WSClient<DataFrame, Sender<WebSocketStream>, Receiver<WebSocketStream>>;

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

pub struct Context<T: Session> {
    client: Client,
    session: T,
}

pub struct Request {
    pub action: String,
    payload: Object,
}

/// Interface for access to payload of request.
pub trait Extractor<T> {
    fn extract(&mut self, key: &str) -> Option<T>;
}

macro_rules! extract_as {
    ($fort:ty, $from:path => $to:ty) => {
        impl Extractor<$to> for $fort {
            fn extract(&mut self, key: &str) -> Option<$to> {
                if let Some($from(data)) = self.payload.remove(key) {
                    Some(data)
                } else {
                    None
                }
            }
        }
    }
}

extract_as!(Request, Json::Object => Object);
extract_as!(Request, Json::String => String);
extract_as!(Request, Json::I64 => i64);
extract_as!(Request, Json::F64 => f64);


pub enum Input {
    Request(String, Request),
    Next(Option<Request>),
}

pub enum Output {
    Ready,
    Item(Object),
    Done,
    Reject(String),
    Fail(String),
}

#[derive(Debug)]
pub enum Error {
    IllegalJsonFormat,
    IllegalEventType,
    IllegalEventName(String),
    IllegalMessage,
    IllegalDataFormat,
    IllegalRequestFormat,
    BadMessageEncoding,
    ServiceNotFound,
    DataNotProvided,
    UnexpectedState,
    Canceled,
    ConnectionBroken,
    ConnectionClosed,
    WorkerFailed(Box<error::Error>),
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
            BadMessageEncoding => "wrong message encoding",
            ServiceNotFound => "service not found",
            DataNotProvided => "data not provided",
            UnexpectedState => "unexpected state",
            Canceled => "cancelled",
            ConnectionBroken => "connection broken",
            ConnectionClosed => "connection closed",
            WorkerFailed(ref cause) => cause.description(),
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
        use std::error::Error;
        write!(f, "Internal error: {}", self.description())
    }
}


impl From<Box<error::Error>> for Error {
    fn from(error: Box<error::Error>) -> Self {
        Error::WorkerFailed(error)
    }
}

impl<T: Session> Deref for Context<T> {
    type Target = T;

    fn deref<'a>(&'a self) -> &'a T {
        &self.session
    }
}

impl<T: Session> DerefMut for Context<T> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        &mut self.session
    }
}

impl<T: Session> Context<T> {
    pub fn new(client: Client, session: T) -> Self {
        Context {
            client: client,
            session: session,
        }
    }

    fn recv(&mut self) -> Result<Input, Error> {
        let message: Message = match self.client.get_mut_receiver().recv_message() {
            Ok(m) => m,
            Err(_) => return Err(Error::ConnectionBroken),
        };
        match message.opcode {
            Type::Text => {
                let content = match str::from_utf8(&*message.payload) {
                    Ok(data) => data,
                    Err(_) => return Err(Error::BadMessageEncoding),
                };
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
                                Some(_) =>
                                    Err(Error::IllegalDataFormat),
                                None =>
                                    Err(Error::DataNotProvided),
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
                                Some(Json::Null) =>
                                    None,
                                Some(_) =>
                                    return Err(Error::IllegalDataFormat),
                                None =>
                                    None,
                            };
                            Ok(Input::Next(request))
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
            Type::Ping => {
                match self.client.send_message(&Message::pong(message.payload)) {
                    Ok(_) => self.recv(),
                    Err(_) => Err(Error::ConnectionBroken),
                }
            },
            Type::Binary => Err(Error::IllegalMessage),
            Type::Pong => Err(Error::IllegalMessage), // we don't send pings!
            Type::Close => Err(Error::ConnectionClosed),
        }
    }

    pub fn recv_request(&mut self) -> Result<(String, Request), Error> {
        match self.recv() {
            Ok(Input::Request(service, request)) => Ok((service, request)),
            Ok(_) => Err(Error::UnexpectedState),
            Err(ie) => Err(ie),
        }
    }

    pub fn recv_next(&mut self) -> Result<Option<Request>, Error> {
        match self.recv() {
            Ok(Input::Next(req)) => Ok(req),
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
        };
        let content = json.to_string();
        debug!("Send <= {}", content);
        match self.client.send_message(&Message::text(content)) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::ConnectionBroken),
        }
    }

}
