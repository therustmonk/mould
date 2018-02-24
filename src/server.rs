use std::collections::HashMap;
use service::{self, Service};
use session::{self, Context, Input, Output, Builder, Session};
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

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "service not found")]
    ServiceNotFound,
    #[fail(display = "cannot suspend")]
    CannotSuspend,
    #[fail(display = "cannot resume")]
    CannotResume,
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

pub type Result<T> = ::std::result::Result<T, Error>;

pub fn process_session<T, R>(suite: &Suite<T>, rut: R)
where
    T: Session,
    R: Flow,
{

    let who = rut.who();

    debug!("Start session with {}", who);

    let mut session: Context<T, R> = Context::new(rut, suite.builder.build());

    loop {
        // Session loop
        debug!("Begin new request processing for {}", who);
        let result: Result<()> = (|session: &mut Context<T, R>| {
            loop {
                // Request loop
                let Input { service, action, payload } = session.recv()?;
                let service = suite.services.get(&service).ok_or(Error::ServiceNotFound)?;

                let mut worker = service.route(&action)?;
                let output = (worker.perform)(session, payload)?;
                session.send(Output::Item(output))?;
            }
        })(&mut session);
        // Inform user if
        if let Err(reason) = result {
            let output = match reason {
                // TODO Refactor cancel (rename to StopAll and add CancelWorker)
                Error::SessionFailed(session::Error::Canceled) => continue,
                Error::SessionFailed(session::Error::FlowBroken(_)) => break,
                Error::SessionFailed(session::Error::ConnectionClosed) => break,
                _ => {
                    warn!(
                        "Request processing {} have catch an error {:?}",
                        who,
                        reason
                    );
                    Output::Fail(reason.to_string())
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

    impl Flow for Client<TcpStream> {
        fn who(&self) -> String {
            let ip = self.peer_addr().unwrap();
            format!("WS IP {}", ip)
        }

        fn pull(&mut self) -> Result<Option<String>, flow::Error> {
            let mut last_ping = SystemTime::now();
            let ping_interval = Duration::from_secs(20);
            loop {
                let message = self.recv_message();
                match message {
                    Ok(message) => {
                        match message {
                            OwnedMessage::Text(content) => {
                                return Ok(Some(content));
                            }
                            OwnedMessage::Close(_) => {
                                return Ok(None);
                            }
                            OwnedMessage::Ping(payload) => {
                                self.send_message(&Message::pong(payload))?;
                            }
                            OwnedMessage::Pong(payload) => {
                                trace!("pong received: {:?}", payload);
                            }
                            OwnedMessage::Binary(_) => (),
                        }
                        // No need ping if interaction was successful
                        last_ping = SystemTime::now();
                    }
                    Err(WebSocketError::IoError(ref err))
                        if err.kind() == ErrorKind::WouldBlock => {
                        let elapsed = last_ping
                            .elapsed()
                            .map(|dur| dur > ping_interval)
                            .unwrap_or(false);
                        if elapsed {
                            // Reset time to stop ping flood
                            last_ping = SystemTime::now();
                            trace!("sending ping");
                            self.send_message(&Message::ping("mould-ping".as_bytes()))?;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        return Err(flow::Error::from(err));
                    }
                }
            }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.send_message(&Message::text(content)).map_err(
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
                debug!("Connection from {}", client.who());
                super::process_session(suite.as_ref(), client);
            });
        }
    }
}

#[cfg(feature = "iomould")]
pub mod iomould {
    use std::sync::Arc;
    use std::io::{self, Read, Write, BufRead, BufReader, BufWriter};
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

        fn pull(&mut self) -> Result<Option<String>, flow::Error> {
            let mut buf = String::new();
            let read = self.reader.read_line(&mut buf)?;
            if read > 0 { Ok(Some(buf)) } else { Ok(None) }
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
