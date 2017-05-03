use serde_json;
use session::Session;
use permission;

error_chain! {
    links {
        Permission(permission::Error, permission::ErrorKind);
    }
    foreign_links {
        Serde(serde_json::Error);
    }
    errors {
        AppFault
        SysFault
        Unimplemented
    }
}

pub enum Realize<T> {
    OneItem(T),
    Reject(String),
    Empty,
    Done,
}

impl<'a, T> From<&'a str> for Realize<T> {
    fn from(s: &'a str) -> Self {
        Realize::Reject(s.to_owned())
    }
}

impl<T> From<String> for Realize<T> {
    fn from(s: String) -> Self {
        Realize::Reject(s)
    }
}

pub enum Shortcut<T> {
    OneItemAndDone(T),
    Reject(String),
    Tuned,
    Done,
}

impl<'a, T> From<&'a str> for Shortcut<T> {
    fn from(s: &'a str) -> Self {
        Shortcut::Reject(s.to_owned())
    }
}

impl<T> From<String> for Shortcut<T> {
    fn from(s: String) -> Self {
        Shortcut::Reject(s)
    }
}

pub trait Worker<T: Session> {
    type Request;
    type In;
    type Out;

    fn prepare(&mut self, _: &mut T, _: Self::Request) -> Result<Shortcut<Self::Out>> {
        Ok(Shortcut::Tuned)
    }
    fn realize(&mut self, _: &mut T, _: Self::In) -> Result<Realize<Self::Out>> {
        Err(ErrorKind::Unimplemented.into())
    }
}

