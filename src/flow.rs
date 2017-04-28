
error_chain! {
    errors {
        ConnectionBroken
        BadMessageEncoding
    }
}

pub trait Flow {
    fn who(&self) -> String;
    fn pull(&mut self) -> Result<Option<String>>;
    fn push(&mut self, content: String) -> Result<()>;
}


