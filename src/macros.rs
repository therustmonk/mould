//! This implementation was borrowed from https://github.com/DenisKolodin/json_macro

#[macro_export]
macro_rules! mould_json {
    ([$($val:tt),*]) => {{
        use $crate::rustc_serialize::json::{Json, Array};
        let mut array = Array::new();
        $( array.push(mould_json!($val)); )*
        Json::Array(array)
    }};
    ({ $($key:expr => $val:tt),* }) => {{
        use $crate::rustc_serialize::json::{Json, Object};
        let mut object = Object::new();
        $( object.insert($key.to_owned(), mould_json!($val)); )*
        Json::Object(object)
    }};
    ($val:expr) => {{
        use $crate::rustc_serialize::json::ToJson;
        $val.to_json()
    }};
}

#[macro_export]
macro_rules! mould_object {
    { $($key:expr => $val:tt),* } => {{
        use $crate::rustc_serialize::json::Object;
        let mut object = Object::new();
        $( object.insert($key.to_owned(), mould_json!($val)); )*
        object
    }};
}

#[macro_export]
macro_rules! extract_field {
    ($request:ident, $name:expr) => {{
        let opt = $request.extract($name);
        match opt {
            Some(value) => value,
            None => {
                let msg = format!("Field {} not provided.", $name);
                let error = $crate::worker::Error::Reject(msg);
                return Err(error);
            }
        }
    }};
}

mod test {
    #[test]
    fn test_json_plain() {
        use rustc_serialize::json::{Json};
        assert_eq!(Json::I64(1), mould_json!(1i64));
        assert_eq!(Json::U64(2), mould_json!(2u64));
        assert_eq!(Json::F64(3.1), mould_json!(3.1f64));
        assert_eq!(Json::String("string".to_string()), mould_json!("string"));
        assert_eq!(Json::Boolean(true), mould_json!(true));
        assert_eq!(Json::Null, mould_json!(Json::Null));
    }

    #[test]
    fn test_json_array() {
        use rustc_serialize::json::{Json, Array};
        let mut array = Array::new();
        array.push(Json::I64(1));
        array.push(Json::I64(2));
        array.push(Json::I64(3));
        array.push(Json::I64(4));
        array.push(Json::I64(5));
        assert_eq!(Json::Array(array), mould_json!([1i64,2,3,4,5]));
    }

    #[test]
    fn test_json_object() {
        use rustc_serialize::json::{Json, Object};
        let mut object = Object::new();
        object.insert("one".to_string(), Json::F64(3.1));
        let mut inner = Object::new();
        inner.insert("sub".to_string(), Json::String("string".to_string()));
        object.insert("two".to_string(), Json::Object(inner));
        assert_eq!(object, mould_object!{
            "one" => 3.1f64,
            "two" => (Json::Object(mould_object!{
                "sub" => "string"
            }))
        });
        assert_eq!(Json::Object(object), mould_json!({
            "one" => 3.1f64,
            "two" => (mould_json!({
                "sub" => "string"
            }))
        }));
    }

    /*
    #[test]
    fn test_extract_field() {
        use rustc_serialize::json::{Json, Object};
        let object = mould_json!{
            "one" => 3.1f64
        };
        let request = Request
        let clos = move || {
            extract_field!(request, "one")
        };
        let value: f64 = clos().unwrap();
    }
    */
}

