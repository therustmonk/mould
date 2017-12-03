use std::rc::Rc;
use std::cell::RefCell;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use session::Session;
use worker::{self, Worker, Shortcut, Realize};

error_chain! {
    errors {
        ActionNotFound
    }
}

/// Service looks into session or request to build corresponding worker.
///
/// There is `Send` restriction, because reference to services sends through
/// thread boundaries to user's session (connection) routine.
/// It needs `Sync`, because service get access from multiple threads.
pub trait Service<T: Session>: Send + Sync + 'static {
    /// Never return error, but rejecting Worker created
    fn route(&self, action: &str) -> Result<Action<T>>;
}

pub struct Action<T: 'static> {
    pub prepare: Box<FnMut(&mut T, Value) -> worker::Result<Shortcut<Value>>>,
    pub realize: Box<FnMut(&mut T, Value) -> worker::Result<Realize<Value>>>,
}

impl<T: Session> Action<T> {
    pub fn from_worker<W, R, I, O>(worker: W) -> Self
    where
        for<'de> R: Deserialize<'de>,
        for<'de> I: Deserialize<'de>,
        O: Serialize,
        W: Worker<T, Request = R, In = I, Out = O> + 'static,
    {
        let rcw = Rc::new(RefCell::new(worker));
        let worker = rcw.clone();
        let prepare = move |session: &mut T, value: Value| {
            let p = serde_json::from_value(value)?;
            let prepared = worker.borrow_mut().prepare(session, p)?;
            let result = match prepared {
                Shortcut::OneItemAndDone(t) => {
                    let t = serde_json::to_value(t)?;
                    Shortcut::OneItemAndDone(t)
                }
                Shortcut::Tuned => Shortcut::Tuned,
                Shortcut::Done => Shortcut::Done,
            };
            Ok(result)
        };
        let worker = rcw.clone();
        let realize = move |session: &mut T, value: Value| {
            let value = serde_json::from_value(value)?;
            let realized = worker.borrow_mut().realize(session, value)?;
            let result = match realized {
                Realize::OneItem(t) => {
                    let t = serde_json::to_value(t)?;
                    Realize::OneItem(t)
                }
                Realize::Empty => Realize::Empty,
                Realize::Done => Realize::Done,
            };
            Ok(result)
        };
        Action {
            prepare: Box::new(prepare),
            realize: Box::new(realize),
        }
    }
}
