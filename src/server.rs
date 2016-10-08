use std::thread;
use std::sync::Arc;
use std::collections::HashMap;
use std::net::ToSocketAddrs;

use websocket::Server;
use service::Service;
use session::{self, Context, Output, Builder, Session};
use worker::{Realize, Shortcut};

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

pub fn start<T, A, B>(addr: A, suite: Suite<T, B>)
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
            let mut client = response.send().unwrap(); // Send the response
            let ip = client.get_mut_sender().get_mut().peer_addr().unwrap();

            debug!("Connection from {}", ip);

            let mut session: Context<T> = Context::new(client, suite.builder.build());
            // TODO Determine handler by action name (refactoring handler needed)

            debug!("Start session for {}", ip);
            loop { // Session loop
                debug!("Begin new request processing for {}", ip);
                let result: Result<(), session::Error> = (|session: &mut Context<T>| loop { // Request loop
                    let (name, request) = try!(session.recv_request());
                    let service = match suite.services.get(&name) {
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

                    loop {
                        try!(session.send(Output::Ready));
                        let option_request = try!(session.recv_next());
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
                    }


                })(&mut session);
                // Inform user if
                if let Err(reason) = result {
                    let output = match reason {
                        session::Error::Canceled => continue,
                        session::Error::ConnectionBroken => break,
                        session::Error::ConnectionClosed => break,
                        _ => {
                            warn!("Request processing {} have catch an error {:?}", ip, reason);
                            Output::Fail(reason.to_string())
                        },
                    };
                    session.send(output).unwrap();
                }
            }
            debug!("Ends session for {}", ip);

            // Standard sequence! Only one task simultaneous!
            // Simple to debug, Simple to implement client, corresponds to websocket main principle!

        });
    }
}
