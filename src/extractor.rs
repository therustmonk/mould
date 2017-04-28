use serde_json::{Value, Map};
use session::{Request, Array, Object};

error_chain! {
    errors {
        WrongField(t: String) {
            description("wrong field")
            display("wrong field: '{}'", t)
        }
    }
}

// TODO Remove this module, because it's wrong
// Also it cloned keys to prepare errors (((

/// Interface for access to payload of request.
pub trait Extractor<T> {
    // TODO Change to Result<T, String> to remove extract_field! macro
    fn extract<'a>(&mut self, key: &'a str) -> Result<T>;
}

impl Extractor<Object> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<Object> {
        self.payload.get(key).and_then(Value::as_object).map(Map::to_owned)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}

impl Extractor<Array> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<Array> {
        self.payload.get(key).and_then(Value::as_array).map(Array::to_owned)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}

impl Extractor<String> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<String> {
        self.payload.get(key).and_then(Value::as_str).map(str::to_owned)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}

impl Extractor<i64> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<i64> {
        self.payload.get(key).and_then(Value::as_i64)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}

impl Extractor<f64> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<f64> {
        self.payload.get(key).and_then(Value::as_f64)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}

impl Extractor<bool> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<bool> {
        self.payload.get(key).and_then(Value::as_bool)
            .ok_or(ErrorKind::WrongField(key.to_owned()).into())
    }
}


