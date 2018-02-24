
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "access denied")]
    AccessDenied,
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub trait Rights {}

pub trait Require<R: Rights> {
    fn require(&self, right: &R) -> Result<()>;
}

impl<T: HasRight<R>, R: Rights> Require<R> for T {
    fn require(&self, right: &R) -> Result<()> {
        if self.has_right(right) {
            Ok(())
        } else {
            Err(Error::AccessDenied)
        }
    }
}

pub trait HasRight<R: Rights> {
    fn has_right(&self, right: &R) -> bool;
}
