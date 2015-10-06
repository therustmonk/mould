#[macro_use]
extern crate log;
extern crate rustc_serialize;
extern crate websocket;

pub mod handlers;
pub mod workers;
pub mod session;


pub use rustc_serialize::json::{Json, Object};
pub use session::{Request, ContextMap};



#[test]
fn it_works() {
}
