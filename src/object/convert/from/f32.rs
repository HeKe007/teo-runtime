use std::sync::Arc;
use crate::value::Value;
use crate::object::{Object, ObjectInner};

impl From<f32> for Object {

    fn from(value: f32) -> Self {
        Object {
            inner: Arc::new(ObjectInner::Teon(Value::Float32(value)))
        }
    }
}