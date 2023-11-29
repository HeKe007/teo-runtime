use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct Object {
    inner: Arc<ObjectInner>
}

impl Object {

    pub fn new(struct_path: Vec<String>, fields: BTreeMap<String, crate::object::Object>) -> Self {
        Self {
            inner: Arc::new(ObjectInner {
                struct_path,
                fields: Mutex::new(fields),
            })
        }
    }

    pub fn struct_path(&self) -> Vec<&str> {
        self.inner.as_ref().struct_path.iter().map(AsRef::as_ref).collect()
    }
}

impl Serialize for Object {

    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_none()
    }
}

#[derive(Debug)]
struct ObjectInner {
    struct_path: Vec<String>,
    fields: Mutex<BTreeMap<String, crate::object::Object>>
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner.struct_path.join("."))
    }
}

