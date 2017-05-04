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
    Empty,
    Done,
}

pub enum Shortcut<T> {
    OneItemAndDone(T),
    Tuned,
    Done,
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

