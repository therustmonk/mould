use std::fmt;
use std::error;
use serde_json::{Value, Map};
use session::{Request, Array, Object};

#[derive(Debug)]
pub struct ExtractError<'a> {
    key: &'a str,
}

impl<'a> error::Error for ExtractError<'a> {
    fn description(&self) -> &str {
        "field not found"
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl<'a> fmt::Display for ExtractError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Field {} not provided or have wrong format.", self.key)
    }
}

impl<'a> From<&'a str> for ExtractError<'a> {
    fn from(key: &'a str) -> Self {
        ExtractError {
            key: key,
        }
    }
}

/// Interface for access to payload of request.
pub trait Extractor<T> {
    // TODO Change to Result<T, String> to remove extract_field! macro
    fn extract<'a>(&mut self, key: &'a str) -> Result<T, ExtractError<'a>>;
}

impl Extractor<Object> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<Object, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_object).map(Map::to_owned)
            .ok_or(ExtractError::from(key))
    }
}

impl Extractor<Array> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<Array, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_array).map(Array::to_owned)
            .ok_or(ExtractError::from(key))
    }
}

impl Extractor<String> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<String, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_str).map(str::to_owned)
            .ok_or(ExtractError::from(key))
    }
}

impl Extractor<i64> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<i64, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_i64)
            .ok_or(ExtractError::from(key))
    }
}

impl Extractor<f64> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<f64, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_f64)
            .ok_or(ExtractError::from(key))
    }
}

impl Extractor<bool> for Request {
    fn extract<'a>(&mut self, key: &'a str) -> Result<bool, ExtractError<'a>> {
        self.payload.get(key).and_then(Value::as_bool)
            .ok_or(ExtractError::from(key))
    }
}


