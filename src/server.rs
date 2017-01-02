use std::collections::HashMap;
use slab::Slab;

use service::Service;
use session::{self, Alternative, Context, Output, Builder, Session};
use worker::{Realize, Shortcut};
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

pub fn process_session<T, B, R>(suite: &Suite<T, B>, rut: R)
    where B: Builder<T>, T: Session, R: Flow {

    let who = rut.who();

    debug!("Start session with {}", who);

    let mut session: Context<T, R> = Context::new(rut, suite.builder.build());
    // TODO Determine handler by action name (refactoring handler needed)

    let mut suspended_workers = Slab::with_capacity(10);
    loop { // Session loop
        debug!("Begin new request processing for {}", who);
        let result: Result<(), session::Error> = (|session: &mut Context<T, R>| {
            loop { // Request loop
                let mut worker = match try!(session.recv_request_or_resume()) {
                    Alternative::Usual((service_name, request)) => {
                        let service = match suite.services.get(&service_name) {
                            Some(value) => value,
                            None => return Err(session::Error::ServiceNotFound),
                        };

                        let mut worker = service.route(&request);

                        match try!(worker.prepare(session, request)) {
                            Shortcut::Done => {
                                try!(session.send(Output::Done));
                                continue;
                            },
                            Shortcut::Reject(reason) => {
                                try!(session.send(Output::Reject(reason)));
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
                                return Err(session::Error::WorkerNotFound);
                            },
                        }
                    },
                };
                loop {
                    try!(session.send(Output::Ready));
                    match try!(session.recv_next_or_suspend()) {
                        Alternative::Usual(option_request) => {
                            match try!(worker.realize(session, option_request)) {
                                Realize::OneItem(item) => {
                                    try!(session.send(Output::Item(item)));
                                },
                                Realize::OneItemAndDone(item) => {
                                    try!(session.send(Output::Item(item)));
                                    try!(session.send(Output::Done));
                                    break;
                                },
                                Realize::ManyItems(iter) => {
                                    for item in iter {
                                        try!(session.send(Output::Item(item)));
                                    }
                                },
                                Realize::ManyItemsAndDone(iter) => {
                                    for item in iter {
                                        try!(session.send(Output::Item(item)));
                                    }
                                    try!(session.send(Output::Done));
                                    break;
                                },
                                Realize::Reject(reason) => {
                                    try!(session.send(Output::Reject(reason)));
                                    break;
                                },
                                Realize::Done => {
                                    try!(session.send(Output::Done));
                                    break;
                                },
                            }
                        },
                        Alternative::Unusual(()) => {
                            match suspended_workers.insert(worker) {
                                Ok(task_id) => {
                                    try!(session.send(Output::Suspended(task_id)));
                                    break;
                                },
                                Err(_) => {
                                    // TODO Conside to continue worker (don't fail)
                                    return Err(session::Error::CannotSuspend);
                                },
                            }
                        },
                    }
                }
            }
        })(&mut session);
        // Inform user if
        if let Err(reason) = result {
            let output = match reason {
                // TODO Refactor cancel (rename to StopAll and add CancelWorker)
                session::Error::Canceled => continue,
                session::Error::ConnectorFail(_) => break,
                session::Error::ConnectionClosed => break,
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
    use std::sync::Arc;
    use std::net::ToSocketAddrs;
    use std::str::{self, Utf8Error};
    use websocket::Server;
    use websocket::message::{Message, Type};
    use websocket::client::Client as WSClient;
    use websocket::dataframe::DataFrame;
    use websocket::sender::Sender;
    use websocket::receiver::Receiver;
    use websocket::stream::WebSocketStream;
    use websocket::result::WebSocketError;
    use websocket::ws::receiver::Receiver as WSReceiver;
    use session::{Builder, Session};
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

    pub type Client = WSClient<DataFrame, Sender<WebSocketStream>, Receiver<WebSocketStream>>;

    impl Flow for Client {
        fn who(&self) -> String {
            let ip = self.get_sender().get_ref().peer_addr().unwrap();
            format!("WS IP {}", ip)
        }

        fn pull(&mut self) -> Result<Option<String>, flow::Error> {
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
                            Err(flow::Error::ConnectionBroken)
                        },
                    }
                },
                Type::Binary | Type::Pong => {
                    // continue
                    self.pull()
                },
            }
        }

        fn push(&mut self, content: String) -> Result<(), flow::Error> {
            self.send_message(&Message::text(content)).map_err(flow::Error::from)
        }
    }



    pub fn start<T, A, B>(addr: A, suite: super::Suite<T, B>)
        where A: ToSocketAddrs, B: Builder<T>, T: Session {
        // CLIENTS HANDLING
        // Fail if can't bind, safe to unwrap
        let server = Server::bind(addr).unwrap();
        let suite = Arc::new(suite);

        for connection in server {
            let suite = suite.clone();
            thread::spawn(move || {
                // Separate thread, safe to unwrap connection initialization
                let request = connection.unwrap().read_request().unwrap(); // Get the request
                //let headers = request.headers.clone(); // Keep the headers so we can check them
                request.validate().unwrap(); // Validate the request
                let /*mut*/ response = request.accept(); // Form a response
                /* TODO Protocols declaration
                if let Some(&WebSocketProtocol(ref protocols)) = headers.get() {
                    if protocols.contains(&("rust-websocket".to_string())) {
                        // We have a protocol we want to use
                        response.headers.set(WebSocketProtocol(vec!["rust-websocket".to_string()]));
                    }
                }
                */
                let client = response.send().unwrap(); // Send the response

                debug!("Connection from {}", client.who());

                super::process_session(suite.as_ref(), client);
            });
        }
    }
}

#[cfg(feature = "iomould")]
pub mod iomould {
    use std::sync::Arc;
    use std::io::{self, Read, Write, BufRead, BufReader, LineWriter};
    use session::{Builder, Session};
    use flow::{self, Flow};

    impl From<io::Error> for flow::Error {
        fn from(_: io::Error) -> Self {
            flow::Error::ConnectionBroken
        }
    }


    pub struct IoFlow<R: Read, W: Write> {
        who: String,
        reader: BufReader<R>,
        writer: LineWriter<W>,
    }

    // Can read from stdin, files, sockets, etc!
    // It's simpler to implemet async reactor with this flow
    impl<R: Read, W: Write> IoFlow<R, W> {
        pub fn new(who: &str, reader: R, writer: W) -> Self {
            IoFlow {
                who: who.to_owned(),
                reader: BufReader::new(reader),
                writer: LineWriter::new(writer),
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
            self.writer.write_all(content.as_bytes()).map_err(flow::Error::from)
        }
    }

    pub fn start<T, B>(suite: super::Suite<T, B>)
        where B: Builder<T>, T: Session {
        let client = IoFlow::stdio();
        // Use Arc to allow joining diferent start functions
        let suite = Arc::new(suite);
        debug!("Connection from {}", client.who());
        super::process_session(suite.as_ref(), client);
    }
}
