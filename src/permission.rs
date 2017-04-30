
error_chain! {
    errors {
        AccessDenied
    }
}

pub trait Rights {
}

pub trait Require<T: Rights> {
    fn require(&self, right: &T) -> Result<()>;
}
