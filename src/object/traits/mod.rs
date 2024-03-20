use teo_result::{Error, Result};
use crate::value::Value;

pub trait PrimitiveStruct {
    fn default_struct_path(&self) -> Result<Vec<&'static str>>;
}

impl PrimitiveStruct for Value {

    fn default_struct_path(&self) -> Result<Vec<&'static str>> {
        Ok(match self {
            Value::Null => vec!["std", "Null"],
            Value::Bool(_) => vec!["std", "Bool"],
            Value::Int(_) => vec!["std", "Int"],
            Value::Int64(_) => vec!["std", "Int64"],
            Value::Float32(_) => vec!["std", "Float32"],
            Value::Float(_) => vec!["std", "Float"],
            Value::Decimal(_) => vec!["std", "Decimal"],
            Value::ObjectId(_) => vec!["std", "ObjectId"],
            Value::String(_) => vec!["std", "String"],
            Value::Date(_) => vec!["std", "Date"],
            Value::DateTime(_) => vec!["std", "DateTime"],
            Value::Array(_) => vec!["std", "Array"],
            Value::Dictionary(_) => vec!["std", "Dictionary"],
            Value::Range(_) => vec!["std", "Range"],
            Value::Tuple(_) => Err(Error::new("tuple struct is not supported"))?,
            Value::EnumVariant(_) => Err(Error::new("enum variant struct is not supported"))?,
            Value::Regex(_) => vec!["std", "Regex"],
            Value::File(_) => vec!["std", "File"],
            Value::OptionVariant(_) => Err(Error::new("option variant struct is not supported"))?,
        })
    }
}