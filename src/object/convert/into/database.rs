use teo_teon::types::enum_variant::EnumVariant;
use teo_teon::Value;
use crate::config::connector::Database;
use crate::error::Error;
use crate::object::Object;

impl TryFrom<Object> for Database {

    type Error = Error;

    fn try_from(ref value: Object) -> std::result::Result<Self, Self::Error> {
        let teon: Value = value.try_into()?;
        let enum_variant: EnumVariant = teon.try_into()?;
        if !enum_variant.value.is_string() {
            Err(Error::new(format!("object is not enum variant: {:?}", value)))?
        }
        match enum_variant.value.as_str().unwrap() {
            "mysql" => Ok(Database::MySQL),
            "postgres" => Ok(Database::PostgreSQL),
            "mongo" => Ok(Database::MongoDB),
            "sqlite" => Ok(Database::SQLite),
            _ => Err(Error::new(format!("invalid database name: {:?}", value)))
        }
    }
}