
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "connection broken")]
    ConnectionBroken,
    #[fail(display = "bad message encoding")]
    BadMessageEncoding,
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub trait Flow {
    fn who(&self) -> String;
    fn pull(&mut self) -> Result<Option<String>>;
    fn push(&mut self, content: String) -> Result<()>;
}
