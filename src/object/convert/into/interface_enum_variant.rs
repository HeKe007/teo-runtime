use teo_result::Error;
use crate::interface_enum_variant::InterfaceEnumVariant;
use crate::object::Object;

impl<'a> TryFrom<&'a Object> for &'a InterfaceEnumVariant {

    type Error = Error;

    fn try_from(value: &'a Object) -> std::result::Result<Self, Self::Error> {
        if let Some(v) = value.as_interface_enum_variant() {
            Ok(v)
        } else {
            Err(Error::new(format!("object is not InterfaceEnumVariant: {:?}", value)))
        }
    }
}

impl<'a> TryFrom<&'a Object> for InterfaceEnumVariant {

    type Error = Error;

    fn try_from(value: &'a Object) -> std::result::Result<Self, Self::Error> {
        if let Some(v) = value.as_interface_enum_variant() {
            Ok(v.clone())
        } else {
            Err(Error::new(format!("object is not InterfaceEnumVariant: {:?}", value)))
        }
    }
}

impl TryFrom<Object> for InterfaceEnumVariant {

    type Error = Error;

    fn try_from(value: Object) -> std::result::Result<Self, Self::Error> {
        if let Some(v) = value.as_interface_enum_variant() {
            Ok(v.clone())
        } else {
            Err(Error::new(format!("object is not InterfaceEnumVariant: {:?}", value)))
        }
    }
}