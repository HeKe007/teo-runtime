pub mod serde;
pub mod convert;
pub mod traits;
pub mod error_ext;
pub mod cast;

use std::fmt::{Display, Formatter};
use std::sync::Arc;
use teo_parser::r#type::Type;
use crate::value::Value;
use teo_result::Error;
use crate::model;
use crate::pipeline::pipeline::Pipeline;
use crate::r#struct;
use teo_result::Result;
use crate::namespace::Namespace;
use crate::object::cast::TeonCast;
use crate::value::interface_enum_variant::InterfaceEnumVariant;

#[derive(Debug, Clone)]
pub struct Object {
    pub inner: Arc<ObjectInner>,
}

#[derive(Debug)]
pub enum ObjectInner {
    Teon(Value),
    ModelObject(model::Object),
    StructObject(r#struct::Object),
    Pipeline(Pipeline),
    InterfaceEnumVariant(InterfaceEnumVariant),
    Array(Vec<Object>),
}

impl AsRef<Object> for Object {

    fn as_ref(&self) -> &Object {
        self
    }
}

impl Object {

    pub fn is_teon(&self) -> bool {
        self.as_teon().is_some()
    }

    pub fn as_teon(&self) -> Option<&Value> {
        match self.inner.as_ref() {
            ObjectInner::Teon(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_model_object(&self) -> bool {
        self.as_model_object().is_some()
    }

    pub fn as_model_object(&self) -> Option<&model::Object> {
        match self.inner.as_ref() {
            ObjectInner::ModelObject(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_struct_object(&self) -> bool {
        self.as_struct_object().is_some()
    }

    pub fn as_struct_object(&self) -> Option<&r#struct::Object> {
        match self.inner.as_ref() {
            ObjectInner::StructObject(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_pipeline(&self) -> bool {
        self.as_pipeline().is_some()
    }

    pub fn as_pipeline(&self) -> Option<&Pipeline> {
        match self.inner.as_ref() {
            ObjectInner::Pipeline(p) => Some(p),
            _ => None,
        }
    }

    pub fn is_interface_enum_variant(&self) -> bool {
        self.as_interface_enum_variant().is_some()
    }

    pub fn as_interface_enum_variant(&self) -> Option<&InterfaceEnumVariant> {
        match self.inner.as_ref() {
            ObjectInner::InterfaceEnumVariant(n) => Some(n),
            _ => None
        }
    }

    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    pub fn as_array(&self) -> Option<&Vec<Object>> {
        match self.inner.as_ref() {
            ObjectInner::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        self.is_teon() && self.as_teon().unwrap().is_null()
    }

    pub fn try_into_err_prefix<T, E>(self, prefix: impl AsRef<str>) -> Result<T> where Error: From<E>, T: TryFrom<Object, Error = E> {
        let result: std::result::Result<T, E> = self.try_into();
        match result {
            Ok(t) => Ok(t),
            Err(e) => Err(Error::new(format!("{}: {}", prefix.as_ref(), Error::from(e)))),
        }
    }

    fn try_into_err_message_inner<T, E>(self) -> Result<T> where Error: From<E>, T: TryFrom<Object, Error = E> {
        Ok(self.try_into()?)
    }

    pub fn try_into_err_message<T, E>(self, message: impl AsRef<str>) -> Result<T> where Error: From<E>, T: TryFrom<Object, Error = E> {
        let result: Result<T> = self.try_into_err_message_inner();
        match result {
            Ok(t) => Ok(t),
            Err(_) => Err(Error::new(message.as_ref())),
        }
    }

    pub fn try_ref_into_err_prefix<'a, T: 'a, E>(&'a self, prefix: impl AsRef<str>) -> Result<T> where Error: From<E>, T: TryFrom<&'a Object, Error = E> {
        let result: std::result::Result<T, E> = self.try_into();
        match result {
            Ok(t) => Ok(t),
            Err(e) => Err(Error::new(format!("{}: {}", prefix.as_ref(), Error::from(e)))),
        }
    }

    pub fn try_ref_into_err_message<'a, T: 'a, E>(&'a self, message: impl AsRef<str>) -> Result<T> where Error: From<E>, T: TryFrom<&'a Object, Error = E> {
        let result: std::result::Result<T, E> = self.try_into();
        match result {
            Ok(t) => Ok(t),
            Err(_) => Err(Error::new(message.as_ref())),
        }
    }

    pub fn cast(&self, target: Option<&Type>, namespace: &Namespace) -> Self {
        if let Some(teon) = self.as_teon() {
            Object::from(teon.cast(target, namespace))
        } else {
            self.clone()
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.inner.as_ref() {
            ObjectInner::Teon(teon) => Display::fmt(teon, f),
            ObjectInner::ModelObject(m) => Display::fmt(m, f),
            ObjectInner::StructObject(s) => Display::fmt(s, f),
            ObjectInner::Pipeline(p) => Display::fmt(p, f),
            ObjectInner::InterfaceEnumVariant(i) => Display::fmt(i, f),
            ObjectInner::Array(objects) => {
                f.write_str("[")?;
                objects.iter().map(|o| format!("{}", o)).collect::<Vec<String>>().join(", ");
                f.write_str("]")
            }
        }
    }
}