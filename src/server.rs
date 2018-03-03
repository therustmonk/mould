use std::collections::HashMap;
use std::borrow::Cow;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use futures::Async;
use serde_json::Value;
use service::{self, Service};
use session::{self, Context, Input, Output, Builder, Session, TaskId};
use worker;
use flow::Flow;

pub struct Suite<T: Session> {
    builder: Box<Builder<T>>,
    services: HashMap<String, Box<Service<T>>>,
}

impl<T: Session> Suite<T> {
    pub fn new<B: Builder<T>>(builder: B) -> Self {
        Suite {
            builder: Box::new(builder),
            services: HashMap::new(),
        }
    }

    pub fn register<S: Service<T>>(&mut self, name: &str, service: S) {
        self.services.insert(name.to_owned(), Box::new(service));
    }
}

// TODO Move part of errors into `Output` of service
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "service not found")]
    ServiceNotFound,
    #[fail(display = "channel broken")]
    ChannelBroken,
    #[fail(display = "service error")]
    ServiceFailed(#[cause] service::Error),
    #[fail(display = "worker error")]
    WorkerFailed(#[cause] worker::Error),
    #[fail(display = "session error")]
    SessionFailed(#[cause] session::Error),
}

impl From<service::Error> for Error {
    fn from(cause: service::Error) -> Self {
        Error::ServiceFailed(cause)
    }
}

impl From<worker::Error> for Error {
    fn from(cause: worker::Error) -> Self {
        Error::WorkerFailed(cause)
    }
}

impl From<session::Error> for Error {
    fn from(cause: session::Error) -> Self {
        Error::SessionFailed(cause)
    }
}

pub struct TaskResolver {
    id: TaskId,
    sender: Sender<Output>,
}

impl TaskResolver {
    pub fn resolve(self, result: ::std::result::Result<Value, Cow<'static, str>>) {
        let id = self.id;
        let (result, error) = {
            match result {
                Ok(result) => (Some(result), None),
                Err(error) => (None, Some(error.into())),
            }
        };
        let output = Output { id, result, error };
        self.sender.send(output).expect("can't send a resolved value");
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub fn process_session<T, R>(suite: &Suite<T>, rut: R)
where
    T: Session,
    R: Flow,
{

    let who = rut.who();

    debug!("Start session with {}", who);

    let mut session: Context<T, R> = Context::new(rut, suite.builder.build());
    let mut chan = channel();

    loop {
        // Session loop
        debug!("Begin new request processing for {}", who);
        let result: Result<()> = (|session: &mut Context<T, R>, &mut (ref mut tx, ref mut rx): &mut (Sender<Output>, Receiver<Output>)| {
            loop {
                // Request loop
                match session.recv()? {
                    Async::Ready(data) => {
                        let Input { id, service, action, payload } = data;
                        let service = suite.services.get(&service).ok_or(Error::ServiceNotFound)?;

                        let mut worker = service.route(&action)?;
                        let sender = tx.clone();
                        let task_resolver = TaskResolver { id, sender };
                        (worker.perform)(task_resolver, session, payload)?;
                    }
                    Async::NotReady => {
                        match rx.try_recv() {
                            Ok(output) => {
                                session.send(output)?;
                            }
                            Err(TryRecvError::Empty) => {
                            }
                            Err(TryRecvError::Disconnected) => {
                                return Err(Error::ChannelBroken);
                            }
                        }
                    }
                }
            }
        })(&mut session, &mut chan);
        // Inform user if
        if let Err(reason) = result {
            let output = match reason {
                Error::SessionFailed(session::Error::FlowBroken(_)) => break,
                Error::SessionFailed(session::Error::ConnectionClosed) => break,
                Error::ChannelBroken => break,
                _ => {
                    warn!(
                        "Request processing {} have catch an error {:?}",
                        who,
                        reason
                    );
                    Output {
                        id: 0,
                        result: None,
                        error: Some(reason.to_string()),
                    }
                }
            };
            session.send(output).unwrap();
        }
    }
    debug!("Ends session with {}", who);

    // Standard sequence! Only one task simultaneous!
    // Simple to debug, Simple to implement client, corresponds to websocket main principle!

}

#[cfg(feature = "wsmould")]
pub mod wsmould {
    use std::thread;
    use std::io::ErrorKind;
    use std::sync::Arc;
    use std::net::{ToSocketAddrs, TcpStream};
    use std::str::Utf8Error;
    use std::time::{SystemTime, Duration};
    use futures::{Poll, Async};
    use websocket::sync::Server;
    use websocket::message::{OwnedMessage, Message};
    use websocket::sync::Client;
    use websocket::result::WebSocketError;
    use session::Session;
    use flow::{self, Flow};

    impl From<WebSocketError> for flow::Error {
        fn from(_: WebSocketError) -> Self {
            flow::Error::ConnectionBroken
        }
    }

    impl From<Utf8Error> for flow::Error {
        fn from(_: Utf8Error) -> Self {
            flow::Error::BadMessageEncoding
        }
    }

    pub struct WsFlow {
        client: Client<TcpStream>,
        last_ping: SystemTime,
    }

    impl Flow for WsFlow {
        fn who(&self) -> String {
            let ip = self.client.peer_addr().unwrap();
            format!("WS IP {}", ip)
        }

        fn pull(&mut self) -> Poll<Option<String>, flow::Error> {
            let ping_interval = Duration::from_secs(20);
            loop {
                let message = self.client.recv_message();
                match message {
                    Ok(message) => {
                        match message {
                            OwnedMessage::Text(content) => {
                                return Ok(Async::Ready(Some(content)));
                            }
                            OwnedMessage::Close(_) => {
                                return Ok(Async::Ready(None));
                            }
                            OwnedMessage::Ping(payload) => {
                                self.client.send_message(&Message::pong(payload))?;
                            }
                            OwnedMessage::Pong(payload) => {
                                trace!("pong received: {:?}", payload);
                            }
                            OwnedMessage::Binary(_) => (),
                        }
                        // No need ping if interaction was successful
                        self.last_ping = SystemTime::now();
                    }
                    Err(WebSocketError::IoError(ref err))
                        if err.kind() == ErrorKind::WouldBlock => {
                        // This service pings the client, because not every client
                        // supports ping generating like browsers)
                        let elapsed = self.last_ping
                            .elapsed()
                            .map(|dur| dur > ping_interval)
                            .unwrap_or(false);
                        if elapsed {
                            // Reset time to stop ping flood
                            self.last_ping = SystemTime::now();
                            trace!("sending ping");
                            self.client.send_message(&Message::ping("mould-ping".as_bytes()))?;
                        }
                        return Ok(Async::NotReady);
                    }
                    Err(err) => {
                        return Err(flow::Error::from(err));
                    }
                }
            }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.client.send_message(&Message::text(content)).map_err(
                flow::Error::from,
            )
        }
    }



    pub fn start<T, A>(addr: A, suite: Arc<super::Suite<T>>)
    where
        A: ToSocketAddrs,
        T: Session,
    {
        // CLIENTS HANDLING
        // Fail if can't bind, safe to unwrap
        let server = Server::bind(addr).unwrap();

        for connection in server.filter_map(Result::ok) {
            let suite = suite.clone();
            thread::spawn(move || {
                let client = connection.accept().unwrap();
                client.set_nonblocking(true).expect(
                    "can't use non-blocking webosckets",
                );
                let last_ping = SystemTime::now();
                let flow = WsFlow { client, last_ping };
                debug!("Connection from {}", flow.who());
                super::process_session(suite.as_ref(), flow);
            });
        }
    }
}

#[cfg(feature = "iomould")]
pub mod iomould {
    use std::sync::Arc;
    use std::io::{self, Read, Write, BufRead, BufReader, BufWriter};
    use futures::{Poll, Async};
    use session::Session;
    use flow::{self, Flow};

    impl From<io::Error> for flow::Error {
        fn from(_: io::Error) -> Self {
            flow::Error::ConnectionBroken
        }
    }


    pub struct IoFlow<R: Read, W: Write> {
        who: String,
        reader: BufReader<R>,
        writer: BufWriter<W>,
    }

    // Can read from stdin, files, sockets, etc!
    // It's simpler to implemet async reactor with this flow
    impl<R: Read, W: Write> IoFlow<R, W> {
        pub fn new(who: &str, reader: R, writer: W) -> Self {
            IoFlow {
                who: who.to_owned(),
                reader: BufReader::new(reader),
                writer: BufWriter::new(writer),
            }
        }
    }

    impl IoFlow<io::Stdin, io::Stdout> {
        pub fn stdio() -> Self {
            IoFlow::new("STDIO", io::stdin(), io::stdout())
        }
    }


    impl<R: Read, W: Write> Flow for IoFlow<R, W> {
        fn who(&self) -> String {
            self.who.clone()
        }

        fn pull(&mut self) -> Poll<Option<String>, flow::Error> {
            let mut buf = String::new();
            let read = self.reader.read_line(&mut buf)?;
            if read > 0 { Ok(Async::Ready(Some(buf))) } else { Ok(Async::Ready(None)) }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.writer.write_all(content.as_bytes())?;
            self.writer.write_all(&['\n' as u8])?;
            self.writer.flush().map_err(flow::Error::from)
        }
    }

    pub fn start<T>(suite: Arc<super::Suite<T>>)
    where
        T: Session,
    {
        let client = IoFlow::stdio();
        // Use Arc to allow joining diferent start functions
        debug!("Connection from {}", client.who());
        super::process_session(suite.as_ref(), client);
    }
}
