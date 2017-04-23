// TODO Remove it!

#[macro_export]
macro_rules! extract_field {
    ($request:ident, $name:expr) => {{
        $request.extract($name)?
    }};
}

#[macro_export]
macro_rules! ensure_it {
    ($check:expr, $reason:expr) => {{
        if !$check {
            return Ok(::std::convert::From::from($reason.to_string()));
        }
    }};
}

