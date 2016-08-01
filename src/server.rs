use std::thread;
use std::sync::Arc;
use std::collections::HashMap;
use std::net::ToSocketAddrs;

use websocket::Server;
use session::{self, Session, Output, SessionData};
use router::Router;
use worker::{self, Realize, Shortcut};

pub type BoxedRouter<CTX> = Box<Router<CTX> + Send + Sync>;
pub type ServicesMap<CTX> = HashMap<String, BoxedRouter<CTX>>;


pub fn start<To: ToSocketAddrs, CTX: SessionData>(addr: To, services: ServicesMap<CTX>) {
    // CLIENTS HANDLING
    // Fail if can't bind, safe to unwrap
    let server = Server::bind(addr).unwrap();
    let services = Arc::new(services);

    for connection in server {
        let services = services.clone();
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

            let mut session: Session<CTX> = Session::new(client);
            // TODO Determine handler by action name (refactoring handler needed)

            debug!("Start session for {}", ip);
            loop { // Session loop
                debug!("Begin new request workout for {}", ip);
                let result: Result<(), session::Error> = (|session: &mut Session<CTX>| loop { // Request loop
                    let (service, request) = try!(session.recv_request());
                    let router = match services.get(&service) {
                        Some(value) => value,
                        None => return Err(session::Error::ServiceNotFound),
                    };

                    let mut worker = router.route(&request);

                    match try!(worker.prepare(session, request)) {
                        Shortcut::Done => {
                            try!(session.send(Output::Done));
                            continue
                        },
                        Shortcut::Tuned => (),
                    }

                    loop {
                        try!(session.send(Output::Ready));
                        let option_request = try!(session.recv_next());
                        match try!(worker.realize(session, option_request)) {
                            Realize::Done => break,
                            Realize::OneItem(item) => {
                                try!(session.send(Output::Item(item)));
                            },
                            Realize::OneItemAndDone(item) => {
                                try!(session.send(Output::Item(item)));
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
                                break;
                            },
                        }
                    }

                    try!(session.send(Output::Done));

                })(&mut session);
                // Inform user if
                if let Err(reason) = result {
                    let text = match reason {
                        session::Error::Canceled => continue,
                        session::Error::ConnectionBroken => break,
                        session::Error::ConnectionClosed => break,
                        session::Error::RejectedByWorker(worker::Error::Reject(reason)) => {
                            debug!("Request rejected by worker {}", &reason);
                            reason
                        },
                        session::Error::RejectedByWorker(worker::Error::Cause(cause)) => {
                            warn!("Request rejected by cause {}", cause);
                            "Internal error.".to_string()
                        },
                        _ => {
                            warn!("Request workout {} have catch an error {:?}", ip, reason);
                            format!("Rejected with {:?}", reason)
                        },
                    };
                    // Not need this connection. Safe to unwrap.
                    session.send(Output::Reject(text)).unwrap();
                }
            }
            debug!("Ends session for {}", ip);

            // Standard sequence! Only one task simultaneous!
            // Simple to debug, Simple to implement client, corresponds to websocket main principle!

        });
    }
}
