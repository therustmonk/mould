use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use session::Session;
use worker::{self, Worker};

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
    pub perform: Box<FnMut(&mut T, Value) -> worker::Result<Value>>,
}

impl<T: Session> Action<T> {
    pub fn from_worker<W, I, O>(mut worker: W) -> Self
    where
        for<'de> I: Deserialize<'de>,
        O: Serialize,
        W: Worker<T, In = I, Out = O> + 'static,
    {
        let perform = move |session: &mut T, value: Value| {
            let input = serde_json::from_value(value)?;
            let output = worker.perform(session, input)?;
            let result = serde_json::to_value(output)?;
            Ok(result)
        };
        Action {
            perform: Box::new(perform),
        }
    }
}
