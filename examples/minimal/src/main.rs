extern crate mould;

mod session;
mod services;

use std::sync::Arc;
use mould::session::DefaultBuilder;
use mould::server::{wsmould, Suite};
use session::UserSession;
use services::hello::HelloService;

fn main() {
    let mut suite: Suite<UserSession> = Suite::new(DefaultBuilder);
    suite.register("hello", HelloService);

    let host = "localhost";
    let port = 5891;
    let suite = Arc::new(suite);
    wsmould::start((host.as_ref(), port), suite)
}
