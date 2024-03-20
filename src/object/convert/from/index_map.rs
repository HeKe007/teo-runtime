use std::sync::Arc;
use indexmap::IndexMap;
use crate::value::Value;
use crate::object::{Object, ObjectInner};

impl From<IndexMap<String, Value>> for Object {

    fn from(value: IndexMap<String, Value>) -> Self {
        Object {
            inner: Arc::new(ObjectInner::Teon(Value::Dictionary(value)))
        }
    }
}