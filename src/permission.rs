
error_chain! {
    errors {
        AccessDenied
    }
}

pub trait Rights {}

pub trait Require<R: Rights> {
    fn require(&self, right: &R) -> Result<()>;
}

impl<T: HasRight<R>, R: Rights> Require<R> for T {
    fn require(&self, right: &R) -> Result<()> {
        if self.has_right(right) {
            Ok(())
        } else {
            Err(ErrorKind::AccessDenied.into())
        }
    }
}

pub trait HasRight<R: Rights> {
    fn has_right(&self, right: &R) -> bool;
}
