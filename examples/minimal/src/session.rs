use mould::{self, permission};
use services::*;

pub struct UserSession {
}

impl Default for UserSession {
    fn default() -> Self {
        UserSession { }
    }
}

impl mould::Session for UserSession { }

impl permission::HasRight<hello::Permission> for UserSession {
    fn has_right(&self, p: &hello::Permission) -> bool {
        use self::hello::Permission;
        match *p {
            Permission::CanDoIt => true,
        }
    }
}

