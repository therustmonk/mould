use std::thread;
use std::sync::Arc;
use std::collections::HashMap;
use std::net::ToSocketAddrs;

use websocket::Server;
use session::{Session, SessionError, Output};
use handler::{Handler, Worker};

pub type BoxedHandler = Box<Handler + Send + Sync>;
pub type ServicesMap = HashMap<String, BoxedHandler>;


pub fn start<To: ToSocketAddrs>(addr: To, services: ServicesMap) {
    // CLIENTS HANDLING
    let server = Server::bind(addr).unwrap();
    let services = Arc::new(services);

    for connection in server {
        let services = services.clone();
    	thread::spawn(move || {
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
            
            let ip = client.get_mut_sender()
                .get_mut()
                .peer_addr()
                .unwrap();

            debug!("Connection from {}", ip);

            let mut session = Session::new(client);
            // TODO Determine handler by action name (refactoring handler needed)

            debug!("Start session for {}", ip);
            loop { // Session loop
                debug!("Begin new request workout for {}", ip);
                let result: Result<(), SessionError> = (|session: &mut Session| loop { // Request loop
                    let (service, request) = try!(session.recv_request());
                    let handler = match services.get(&service) {
                        Some(value) => value,
                        None => return Err(SessionError::ServiceNotFound),
                    };

                    let mut worker = handler.build(request);

                    loop {
                        match worker.next(session.borrow_mut_context()) {
                            Ok(Some(iter)) => {
                                try!(session.send(Output::Ready));
                                try!(session.recv_next());
                                for item in iter {
                                    try!(session.send(Output::Item(item)));
                                }
                            },
                            Ok(None) => break,
                            Err(reason) =>
                                return Err(SessionError::RejectedByHandler(reason)),
                        }
                    }

                    try!(session.send(Output::Done));

                })(&mut session);
                // Inform user if 
                if let Err(reason) = result {                    
                    let text = match reason {
                        SessionError::Canceled => continue,
                        SessionError::ConnectionBroken => break,
                        SessionError::ConnectionClosed => break,
                        SessionError::RejectedByHandler(data) => data,
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