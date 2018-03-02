use std::marker::PhantomData;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use futures::unsync::oneshot::{channel, Sender, Receiver};
use session::{Session, TaskId};
use worker::{self, Worker};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "action not found")]
    ActionNotFound,
}

pub type Result<T> = ::std::result::Result<T, Error>;

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
    pub perform: Box<FnMut(TaskId, &mut T, Value) -> worker::Result<Receiver<worker::Result<Value>>>>,
}

pub struct Resolver<T> {
    sender: Sender<worker::Result<Value>>,
    _data: PhantomData<T>,
}

impl<T: Serialize> Resolver<T> {
    fn resolve(self, data: worker::Result<T>) {
        let value = data.and_then(|value| {
            serde_json::to_value(value).map_err(worker::Error::from)
        });
        self.sender.send(value).expect("can't send a resolved value");
    }
}

impl<T: Session> Action<T> {
    pub fn from_worker<W, I, O>(mut worker: W) -> Self
    where
        for<'de> I: Deserialize<'de>,
        O: Serialize,
        W: Worker<T, In = I, Out = O> + 'static,
    {
        let perform = move |id: TaskId, session: &mut T, value: Value| {
            let (sender, receiver) = channel();
            let input = serde_json::from_value(value)?;
            let resolver = Resolver { sender, _data: PhantomData };
            worker.perform(session, input, resolver);
            Ok(receiver)
        };
        Action {
            perform: Box::new(perform),
        }
    }
}
