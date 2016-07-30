//! Session module contains protocol implementation.
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

use std::default::Default;
use std::ops::{Deref, DerefMut};
use std::str;
use rustc_serialize::json::{Json, Object};
use websocket::Message;
use websocket::Client as WSClient;
use websocket::message::Type;
use websocket::sender::Sender;
use websocket::receiver::Receiver;
use websocket::dataframe::DataFrame;
use websocket::stream::WebSocketStream;
use websocket::ws::receiver::Receiver as WSReceiver;
use workers::WorkerError;

pub type Client = WSClient<DataFrame, Sender<WebSocketStream>, Receiver<WebSocketStream>>;

pub trait SessionData: Default + 'static {}

pub struct Session<CTX: SessionData> {
    client: Client,
    context: CTX,
}

pub struct Request {
    pub action: String,
    payload: Object,
}

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


pub enum Input {
    Request(String, Request),
    Next(Option<Request>),
}

pub enum Output {
    Ready,
    Item(Object),
    Done,
    Reject(String),
}

#[derive(Debug)]
pub enum SessionError {
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
    RejectedByWorker(WorkerError),
}

impl From<WorkerError> for SessionError {
    fn from(error: WorkerError) -> Self {
        SessionError::RejectedByWorker(error)
    }
}

impl<CTX: SessionData> Deref for Session<CTX> {
    type Target = CTX;

    fn deref<'a>(&'a self) -> &'a CTX {
        &self.context
    }
}

impl<CTX: SessionData> DerefMut for Session<CTX> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut CTX {
        &mut self.context
    }
}

impl<CTX: SessionData> Session<CTX> {
    pub fn new(client: Client) -> Self {
        Session {
            client: client,
            context: CTX::default(),
        }
    }

    fn recv(&mut self) -> Result<Input, SessionError> {
        let message: Message = match self.client.get_mut_receiver().recv_message() {
            Ok(m) => m,
            Err(_) => return Err(SessionError::ConnectionBroken),
        };
        match message.opcode {
            Type::Text => {
                let content = match str::from_utf8(&*message.payload) {
                    Ok(data) => data,
                    Err(_) => return Err(SessionError::BadMessageEncoding),
                };
                debug!("Recv => {}", content);
                if let Ok(Json::Object(mut data)) = Json::from_str(&content) {
                    if let Some(Json::String(event)) = data.remove("event") {
                        if event == "request" {
                            match data.remove("data") {
                                Some(Json::Object(mut data)) => {
                                    let service = match data.remove("service") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(SessionError::IllegalRequestFormat),
                                    };
                                    let action = match data.remove("action") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(SessionError::IllegalRequestFormat),
                                    };
                                    let payload = match data.remove("payload") {
                                        Some(Json::Object(data)) => data,
                                        _ => return Err(SessionError::IllegalRequestFormat),
                                    };
                                    let request = Request {
                                        action: action,
                                        payload: payload,
                                    };
                                    Ok(Input::Request(service, request))
                                },
                                Some(_) =>
                                    Err(SessionError::IllegalDataFormat),
                                None =>
                                    Err(SessionError::DataNotProvided),
                            }
                        } else if event == "next" {
                            let request = match data.remove("data") {
                                Some(Json::Object(mut data)) => {
                                    let action = match data.remove("action") {
                                        Some(Json::String(data)) => data,
                                        _ => return Err(SessionError::IllegalRequestFormat),
                                    };
                                    let payload = match data.remove("payload") {
                                        Some(Json::Object(data)) => data,
                                        _ => return Err(SessionError::IllegalRequestFormat),
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
                                    return Err(SessionError::IllegalDataFormat),
                                None =>
                                    None,
                            };
                            Ok(Input::Next(request))
                        } else if event == "cancel" {
                           Err(SessionError::Canceled)
                        } else {
                            Err(SessionError::IllegalEventName(event))
                        }
                    } else {
                        Err(SessionError::IllegalEventType)
                    }

                } else {
                    Err(SessionError::IllegalJsonFormat)
                }
            },
            Type::Ping => {
                match self.client.send_message(&Message::pong(message.payload)) {
                    Ok(_) => self.recv(),
                    Err(_) => Err(SessionError::ConnectionBroken),
                }
            },
            Type::Binary => Err(SessionError::IllegalMessage),
            Type::Pong => Err(SessionError::IllegalMessage), // we don't send pings!
            Type::Close => Err(SessionError::ConnectionClosed),
        }
    }

    pub fn recv_request(&mut self) -> Result<(String, Request), SessionError> {
        match self.recv() {
            Ok(Input::Request(service, request)) => Ok((service, request)),
            Ok(_) => Err(SessionError::UnexpectedState),
            Err(ie) => Err(ie),
        }
    }

    pub fn recv_next(&mut self) -> Result<Option<Request>, SessionError> {
        match self.recv() {
            Ok(Input::Next(req)) => Ok(req),
            Ok(_) => Err(SessionError::UnexpectedState),
            Err(ie) => Err(ie),
        }
    }

    pub fn send(&mut self, out: Output) -> Result<(), SessionError> {
        let json = match out {
            Output::Item(data) =>
                mould_json!({"event" => "item", "data" => data}),
            Output::Ready =>
                mould_json!({"event" => "ready"}),
            Output::Done =>
                mould_json!({"event" => "done"}),
            Output::Reject(message) =>
                mould_json!({"event" => "reject", "data" => message}),
        };
        let content = json.to_string();
        debug!("Send <= {}", content);
        match self.client.send_message(&Message::text(content)) {
            Ok(_) => Ok(()),
            Err(_) => Err(SessionError::ConnectionBroken),
        }
    }

}
