use crate::value::Value;
use crate::database::r#type::DatabaseType;
use crate::model::field::Migration;
use crate::namespace::Namespace;
use crate::optionality::Optionality;
use crate::pipeline::pipeline::Pipeline;
use crate::readwrite::read::Read;
use crate::readwrite::write::Write;
use crate::stdlib::decorators::indexable_decorators::{id_decorator, index_decorator, unique_decorator};
use crate::value::interface_enum_variant::InterfaceEnumVariant;

pub(in crate::stdlib) fn load_model_field_decorators(namespace: &mut Namespace) {

    namespace.define_model_field_decorator("map", |arguments, field| {
        let column_name: String = arguments.get("columnName")?;
        field.column_name = column_name;
        Ok(())
    });

    namespace.define_model_field_decorator("db", |arguments, field| {
        let interface_enum_variant: &InterfaceEnumVariant = arguments.get("type")?;
        let database_type = DatabaseType::from_interface_enum_variant(interface_enum_variant, field.availability)?;
        field.database_type = database_type;
        Ok(())
    });

    namespace.define_model_field_decorator("readonly", |arguments, field| {
        field.write = Write::NoWrite;
        Ok(())
    });

    namespace.define_model_field_decorator("writeonly", |arguments, field| {
        field.read = Read::NoRead;
        Ok(())
    });

    namespace.define_model_field_decorator("internal", |arguments, field| {
        field.write = Write::NoWrite;
        field.read = Read::NoRead;
        Ok(())
    });

    namespace.define_model_field_decorator("writeOnCreate", |arguments, field| {
        field.write = Write::WriteOnCreate;
        Ok(())
    });

    namespace.define_model_field_decorator("writeOnce", |arguments, field| {
        field.write = Write::WriteOnce;
        Ok(())
    });

    namespace.define_model_field_decorator("writeNonNull", |arguments, field| {
        field.write = Write::WriteNonNull;
        Ok(())
    });

    namespace.define_model_field_decorator("readwrite", |arguments, field| {
        field.write = Write::Write;
        field.read = Read::Read;
        Ok(())
    });

    namespace.define_model_field_decorator("readIf", |arguments, field| {
        let cond: Pipeline = arguments.get("cond")?;
        field.read = Read::ReadIf(cond);
        Ok(())
    });

    namespace.define_model_field_decorator("writeIf", |arguments, field| {
        let cond: Pipeline = arguments.get("cond")?;
        field.write = Write::WriteIf(cond);
        Ok(())
    });

    namespace.define_model_field_decorator("presentWith", |arguments, field| {
        let fields: Value = arguments.get("fields")?;
        match fields {
            Value::String(e) => field.optionality = Optionality::PresentWith(vec![e.to_owned()]),
            Value::Array(a) => field.optionality = Optionality::PresentWith(a.iter().map(|d| d.as_enum_variant().unwrap().value.to_owned()).collect()),
            _ => panic!()
        }
        Ok(())
    });

    namespace.define_model_field_decorator("presentWithout", |arguments, field| {
        let fields: Value = arguments.get("fields")?;
        match fields {
            Value::String(e) => field.optionality = Optionality::PresentWithout(vec![e.to_owned()]),
            Value::Array(a) => field.optionality = Optionality::PresentWithout(a.iter().map(|d| d.as_enum_variant().unwrap().value.to_owned()).collect()),
            _ => panic!()
        }
        Ok(())
    });

    namespace.define_model_field_decorator("presentIf", |arguments, field| {
        let cond: Pipeline = arguments.get("cond")?;
        field.optionality = Optionality::PresentIf(cond);
        Ok(())
    });

    namespace.define_model_field_decorator("atomic", |arguments, field| {
        field.atomic = true;
        Ok(())
    });

    namespace.define_model_field_decorator("nonatomic", |arguments, field| {
        field.atomic = false;
        Ok(())
    });

    namespace.define_model_field_decorator("id", |arguments, field| {
        id_decorator(arguments, field)
    });

    namespace.define_model_field_decorator("index", |arguments, field| {
        index_decorator(arguments, field)
    });

    namespace.define_model_field_decorator("unique", |arguments, field| {
        unique_decorator(arguments, field)
    });

    namespace.define_model_field_decorator("virtual", |arguments, field| {
        field.r#virtual = true;
        Ok(())
    });

    namespace.define_model_field_decorator("inputOmissible", |arguments, field| {
        field.input_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("outputOmissible", |arguments, field| {
        field.output_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("auto", |arguments, field| {
        field.auto = true;
        field.input_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("autoIncrement", |arguments, field| {
        field.auto_increment = true;
        field.input_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("default", |arguments, field| {
        let value: Value = arguments.get("value")?;
        field.default = Some(value);
        field.input_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("foreignKey", |arguments, field| {
        field.foreign_key = true;
        field.input_omissible = true;
        Ok(())
    });

    namespace.define_model_field_decorator("onSet", |arguments, field| {
        let pipeline: Pipeline = arguments.get("pipeline")?;
        field.on_set = pipeline;
        Ok(())
    });

    namespace.define_model_field_decorator("onSave", |arguments, field| {
        let pipeline: Pipeline = arguments.get("pipeline")?;
        field.on_save = pipeline;
        Ok(())
    });

    namespace.define_model_field_decorator("onOutput", |arguments, field| {
        let pipeline: Pipeline = arguments.get("pipeline")?;
        field.on_output = pipeline;
        Ok(())
    });

    namespace.define_model_field_decorator("queryable", |arguments, field| {
        field.queryable = true;
        Ok(())
    });

    namespace.define_model_field_decorator("unqueryable", |arguments, field| {
        field.queryable = false;
        Ok(())
    });

    namespace.define_model_field_decorator("sortable", |arguments, field| {
        field.sortable = true;
        Ok(())
    });

    namespace.define_model_field_decorator("unsortable", |arguments, field| {
        field.sortable = false;
        Ok(())
    });

    namespace.define_model_field_decorator("canRead", |arguments, field| {
        let pipeline: Pipeline = arguments.get("pipeline")?;
        field.can_read = pipeline;
        Ok(())
    });

    namespace.define_model_field_decorator("canMutate", |arguments, field| {
        let pipeline: Pipeline = arguments.get("pipeline")?;
        field.can_mutate = pipeline;
        Ok(())
    });

    namespace.define_model_field_decorator("dropped", |arguments, field| {
        field.dropped = true;
        Ok(())
    });

    namespace.define_model_field_decorator("migration", |arguments, field| {
        let mut migration = Migration {
            renamed: vec![],
            version: None,
            default: None,
            priority: None,
        };
        let renamed: Option<Value> = arguments.get_optional("renamed")?;
        let version: Option<String> = arguments.get_optional("version")?;
        let default: Option<Value> = arguments.get_optional("default")?;
        let priority: Option<i32> = arguments.get_optional("priority")?;
        if let Some(renamed) = renamed {
            match renamed {
                Value::String(s) => migration.renamed.push(s),
                Value::Array(ss) => for s in ss {
                    migration.renamed.push(s.as_str().unwrap().to_string());
                },
                _ => unreachable!(),
            }
        }
        if let Some(version) = version {
            migration.version = Some(version);
        }
        if let Some(default) = default {
            migration.default = Some(default);
        }
        if let Some(priority) = priority {
            migration.priority = Some(priority as i64);
        }
        field.migration = Some(migration);
        Ok(())
    });
}
