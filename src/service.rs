use std::borrow::Cow;
use std::marker::PhantomData;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use session::Session;
use server::TaskResolver;
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
    pub perform: Box<FnMut(TaskResolver, &mut T, Value) -> worker::Result<()>>,
}

pub struct Resolver<T> {
    task_resolver: TaskResolver,
    _data: PhantomData<T>,
}

impl<T: Serialize> Resolver<T> {
    /// It uses string for errors to force map every error to a clear error message
    pub fn resolve(self, data: ::std::result::Result<T, Cow<'static, str>>) {
        let value = data.and_then(|value| {
            serde_json::to_value(value).map_err(|err| err.to_string().into())
        });
        self.task_resolver.resolve(value);
    }
}

impl<T: Session> Action<T> {
    pub fn from_worker<W, I, O>(mut worker: W) -> Self
    where
        for<'de> I: Deserialize<'de>,
        O: Serialize,
        W: Worker<T, In = I, Out = O> + 'static,
    {
        let perform = move |task_resolver: TaskResolver, session: &mut T, value: Value| {
            let input = serde_json::from_value(value)?;
            let resolver = Resolver { task_resolver, _data: PhantomData };
            // TODO Use sender instead of the direct result
            worker.perform(session, input, resolver)?;
            Ok(())
        };
        Action {
            perform: Box::new(perform),
        }
    }
}
