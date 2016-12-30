
#[derive(Debug)]
pub enum Error {
    ConnectionBroken,
    BadMessageEncoding,
}

pub trait Flow {
    fn who(&self) -> String;
    fn pull(&mut self) -> Result<Option<String>, Error>;
    fn push(&mut self, content: String) -> Result<(), Error>;
}


