
error_chain! {
}

pub trait Permission {
}

pub trait Have<T: Permission> {
    fn have_permissions(&self, permissions: &[T]) -> Result<()>;
}
