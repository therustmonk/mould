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

//use std::iter::Iterator;
use std::default::Default;

use websocket::Message;
use websocket::client::Client as WSClient;
use websocket::dataframe::DataFrame;
use websocket::server::sender::Sender;
use websocket::server::receiver::Receiver;
use websocket::stream::WebSocketStream;
pub use rustc_serialize::json::{Json, ToJson, Object};
//use std::collections::HashMap;

pub type Client = WSClient<DataFrame, Sender<WebSocketStream>, Receiver<WebSocketStream>>;
//pub type ContextMap = HashMap<String, String>;

pub trait SessionData: Default + 'static {}

pub struct Session<CTX> {
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
    ServiceNotFound,
    DataNotProvided,
    UnexpectedState,
    Canceled,
    ConnectionBroken,
    ConnectionClosed,
    RejectedByHandler(String),
}

impl<CTX: SessionData> Session<CTX> {
    pub fn new(client: Client) -> Self {
        Session {
            client: client,
            context: CTX::default(),
        }
    }

    pub fn borrow_mut_context(&mut self) -> &mut CTX {
        &mut self.context
    }

    fn recv(&mut self) -> Result<Input, SessionError> {
        match self.client.recv_message() {
            Ok(Message::Text(ref content)) => {
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
            Ok(Message::Ping(data)) => {
                match self.client.send_message(Message::Pong(data)) {
                    Ok(_) => self.recv(),
                    Err(_) => Err(SessionError::ConnectionBroken),
                }                
            },
            Ok(Message::Binary(_)) => Err(SessionError::IllegalMessage),
            Ok(Message::Pong(_)) => Err(SessionError::IllegalMessage), // we don't send pings!
            Ok(Message::Close(_)) => Err(SessionError::ConnectionClosed),
            Err(_) => Err(SessionError::ConnectionBroken),
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
                json!({"event" => "item", "data" => data}),
            Output::Ready =>
                json!({"event" => "ready"}),
            Output::Done =>
                json!({"event" => "done"}),
            Output::Reject(message) =>
                json!({"event" => "reject", "data" => message}),
        };
        let content = json.to_string();
        debug!("Send <= {}", content);
        match self.client.send_message(Message::Text(content)) {
            Ok(_) => Ok(()),
            Err(_) => Err(SessionError::ConnectionBroken),
        }
    }

}
