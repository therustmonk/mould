use std::borrow::Cow;
use serde_json;
use session::Session;
use permission;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "app fault")]
    AppFault,
    #[fail(display = "sys fault")]
    SysFault,
    #[fail(display = "unimplemented perform")]
    Unimplemented,
    #[fail(display = "permission error")]
    PermissionWrong(#[cause] permission::Error),
    #[fail(display = "serde error")]
    SerdeFailed(#[cause] serde_json::Error),
    #[fail(display = "worker error: {}", _0)]
    Other(Cow<'static, str>),
}

impl From<permission::Error> for Error {
    fn from(cause: permission::Error) -> Self {
        Error::PermissionWrong(cause)
    }
}

impl From<serde_json::Error> for Error {
    fn from(cause: serde_json::Error) -> Self {
        Error::SerdeFailed(cause)
    }
}

impl From<&'static str> for Error {
    fn from(cause: &'static str) -> Self {
        Error::Other(Cow::Borrowed(cause))
    }
}

impl From<String> for Error {
    fn from(cause: String) -> Self {
        Error::Other(Cow::Owned(cause))
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub trait Worker<T: Session> {
    type In;
    type Out;

    fn perform(&mut self, _: &mut T, _: Self::In) -> Result<Self::Out>;
}
