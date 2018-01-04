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

pub trait Worker<T: Session> {
    type In;
    type Out;

    fn perform(&mut self, _: &mut T, _: Self::In) -> Result<Self::Out>;
}
