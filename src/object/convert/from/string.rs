use std::sync::Arc;
use crate::value::Value;
use crate::object::{Object, ObjectInner};

impl From<String> for Object {

    fn from(value: String) -> Self {
        Object {
            inner: Arc::new(ObjectInner::Teon(Value::String(value)))
        }
    }
}