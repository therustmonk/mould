use std::thread;
use std::collections::HashMap;
use slab::Slab;
use service::{self, Service};
use session::{self, Alternative, Context, Output, Builder, Session};
use worker::{self, Realize, Shortcut};
use flow::Flow;

// TODO Change services on the fly
pub struct Suite<T: Session, B: Builder<T>> {
    builder: B,
    services: HashMap<String, Box<Service<T>>>,
}

impl<T: Session, B: Builder<T>> Suite<T, B> {

    pub fn new(builder: B) -> Self {
        Suite {
            builder: builder,
            services: HashMap::new(),
        }
    }

    pub fn register<S: Service<T>>(&mut self, name: &str, service: S) {
        self.services.insert(name.to_owned(), Box::new(service));
    }

}

error_chain! {
    links {
        Session(session::Error, session::ErrorKind);
        Worker(worker::Error, worker::ErrorKind);
        Service(service::Error, service::ErrorKind);
    }
    foreign_links {
    }
    errors {
        ServiceNotFound
        CannotSuspend
        CannotResume
    }
}

pub fn process_session<T, B, R>(suite: &Suite<T, B>, rut: R)
    where B: Builder<T>, T: Session, R: Flow {

    let who = rut.who();

    debug!("Start session with {}", who);

    let mut session: Context<T, R> = Context::new(rut, suite.builder.build());
    // TODO Determine handler by action name (refactoring handler needed)

    let mut suspended_workers = Slab::with_capacity(10);
    loop { // Session loop
        debug!("Begin new request processing for {}", who);
        let result: Result<()> = (|session: &mut Context<T, R>| {
            loop { // Request loop
                let mut worker = match session.recv_request_or_resume()? {
                    Alternative::Usual((service_name, action, request)) => {
                        let service = suite.services.get(&service_name)
                            .ok_or(Error::from(ErrorKind::ServiceNotFound))?;

                        let mut worker = service.route(&action)?;

                        match (worker.prepare)(session, request)? {
                            Shortcut::Done => {
                                session.send(Output::Done)?;
                                continue;
                            },
                            Shortcut::OneItemAndDone(item) => {
                                session.send(Output::Item(item))?;
                                session.send(Output::Done)?;
                                continue;
                            },
                            Shortcut::Tuned => (),
                        }
                        worker
                    },
                    Alternative::Unusual(task_id) => {
                        match suspended_workers.remove(task_id) {
                            Some(worker) => {
                                worker
                            },
                            None => {
                                return Err(ErrorKind::CannotResume.into());
                            },
                        }
                    },
                };
                loop {
                    session.send(Output::Ready)?;
                    match session.recv_next_or_suspend()? {
                        Alternative::Usual(request) => {
                            match (worker.realize)(session, request)? {
                                Realize::OneItem(item) => {
                                    session.send(Output::Item(item))?;
                                },
                                Realize::Empty => {
                                    thread::yield_now();
                                },
                                Realize::Done => {
                                    session.send(Output::Done)?;
                                    break;
                                },
                            }
                        },
                        Alternative::Unusual(()) => {
                            match suspended_workers.insert(worker) {
                                Ok(task_id) => {
                                    session.send(Output::Suspended(task_id))?;
                                    break;
                                },
                                Err(_) => {
                                    // TODO Conside to continue worker (don't fail)
                                    return Err(ErrorKind::CannotSuspend.into());
                                },
                            }
                        },
                    }
                }
            }
        })(&mut session);
        // Inform user if
        if let Err(reason) = result {
            let output = match *reason.kind() {
                // TODO Refactor cancel (rename to StopAll and add CancelWorker)
                ErrorKind::Session(session::ErrorKind::Canceled) => continue,
                ErrorKind::Session(session::ErrorKind::Flow(_)) => break,
                ErrorKind::Session(session::ErrorKind::ConnectionClosed) => break,
                _ => {
                    warn!("Request processing {} have catch an error {:?}", who, reason);
                    Output::Fail(reason.to_string())
                },
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
    use std::str::{self, Utf8Error};
    use std::time::{SystemTime, Duration};
    use websocket::sync::Server;
    use websocket::message::{OwnedMessage, Message};
    use websocket::sync::Client;
    use websocket::result::WebSocketError;
    use session::{Builder, Session};
    use flow::{self, Flow};

    impl From<WebSocketError> for flow::Error {
        fn from(_: WebSocketError) -> Self {
            flow::ErrorKind::ConnectionBroken.into()
        }
    }

    impl From<Utf8Error> for flow::Error {
        fn from(_: Utf8Error) -> Self {
            flow::ErrorKind::BadMessageEncoding.into()
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
                            },
                            OwnedMessage::Close(_) => {
                                return Ok(None);
                            },
                            OwnedMessage::Ping(payload) => {
                                self.send_message(&Message::pong(payload))?;
                            },
                            OwnedMessage::Pong(payload) => {
                                trace!("pong received: {:?}", payload);
                            },
                            OwnedMessage::Binary(_) => (),
                        }
                        // No need ping if interaction was successful
                        last_ping = SystemTime::now();
                    },
                    Err(WebSocketError::IoError(ref err)) if err.kind() == ErrorKind::WouldBlock => {
                        let elapsed = last_ping.elapsed().map(|dur| dur > ping_interval).unwrap_or(false);
                        if elapsed {
                            // Reset time to stop ping flood
                            last_ping = SystemTime::now();
                            trace!("sending ping");
                            self.send_message(&Message::ping("mould-ping".as_bytes()))?;
                        }
                        thread::sleep(Duration::from_millis(50));
                    },
                    Err(err) => {
                        return Err(flow::Error::from(err));
                    },
                }
            }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.send_message(&Message::text(content)).map_err(flow::Error::from)
        }
    }



    pub fn start<T, A, B>(addr: A, suite: Arc<super::Suite<T, B>>)
        where A: ToSocketAddrs, B: Builder<T>, T: Session {
        // CLIENTS HANDLING
        // Fail if can't bind, safe to unwrap
        let server = Server::bind(addr).unwrap();

        for connection in server.filter_map(Result::ok) {
            let suite = suite.clone();
            thread::spawn(move || {
                let client = connection.accept().unwrap();
                client.set_nonblocking(true).expect("can't use non-blocking webosckets");
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
    use session::{Builder, Session};
    use flow::{self, Flow};

    impl From<io::Error> for flow::Error {
        fn from(_: io::Error) -> Self {
            flow::ErrorKind::ConnectionBroken.into()
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
            if read > 0 {
                Ok(Some(buf))
            } else {
                Ok(None)
            }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.writer.write_all(content.as_bytes())?;
            self.writer.write_all(&['\n' as u8])?;
            self.writer.flush().map_err(flow::Error::from)
        }
    }

    pub fn start<T, B>(suite: Arc<super::Suite<T, B>>)
        where B: Builder<T>, T: Session {
        let client = IoFlow::stdio();
        // Use Arc to allow joining diferent start functions
        debug!("Connection from {}", client.who());
        super::process_session(suite.as_ref(), client);
    }
}
