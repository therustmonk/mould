use std::str::{self, Utf8Error};
use websocket::message::{Message, Type};
use websocket::client::Client as WSClient;
use websocket::dataframe::DataFrame;
use websocket::sender::Sender;
use websocket::receiver::Receiver;
use websocket::stream::WebSocketStream;
use websocket::result::WebSocketError;
use websocket::ws::receiver::Receiver as WSReceiver;

#[derive(Debug)]
pub enum Error {
    ConnectionBroken,
    BadMessageEncoding,
}

pub trait Flow {
    fn who(&self) -> String;
    fn pull(&mut self) -> Result<Option<String>, Error>;
    fn push(&mut self, content: String) -> Result<(), Error>;
}

impl From<Utf8Error> for Error {
    fn from(_: Utf8Error) -> Self {
        Error::BadMessageEncoding
    }
}

impl From<WebSocketError> for Error {
    fn from(_: WebSocketError) -> Self {
        Error::ConnectionBroken
    }
}

pub type Client = WSClient<DataFrame, Sender<WebSocketStream>, Receiver<WebSocketStream>>;

impl Flow for Client {
    fn who(&self) -> String {
        let ip = self.get_sender().get_ref().peer_addr().unwrap();
        format!("WS IP {}", ip)
    }
    fn pull(&mut self) -> Result<Option<String>, Error> {
        let message: Message = self.get_mut_receiver().recv_message()?;
        match message.opcode {
            Type::Text => {
                let content = str::from_utf8(&*message.payload)?;
                Ok(Some(content.to_owned()))
            },
            Type::Close => {
                Ok(None)
            },
            Type::Ping => {
                match self.send_message(&Message::pong(message.payload)) {
                    Ok(_) => self.pull(),
                    Err(_) => {
                        Err(Error::ConnectionBroken)
                    },
                }
            },
            Type::Binary | Type::Pong => {
                // continue
                self.pull()
            },
        }
    }

    fn push(&mut self, content: String) -> Result<(), Error> {
        self.send_message(&Message::text(content)).map_err(Error::from)
    }
}
