use mould::prelude::*;

pub enum Permission {
    CanDoIt,
}

impl Rights for Permission { }

pub struct HelloService;

impl<T> service::Service<T> for HelloService
    where
        T: Session + Require<Permission>,
    {
    fn route(&self, action: &str) -> service::Result<Action<T>> {
        match action {
            "do-it" => {
                Ok(Action::from_worker(do_it::DoItWorker))
            }
            _ => {
                Err(service::ErrorKind::ActionNotFound.into())
            }
        }
    }
}


mod do_it {
    use super::*;

    pub struct DoItWorker;

    impl<T: Session> worker::Worker<T> for DoItWorker {
        type In = ();
        type Out = ();

        fn perform(&mut self, _: &mut T, _: Self::In) -> worker::Result<Self::Out> {
            Ok(())
        }
    }
}
