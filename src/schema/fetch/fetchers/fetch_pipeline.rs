use teo_parser::ast::info_provider::InfoProvider;
use teo_parser::ast::schema::Schema;
use teo_parser::ast::top::Top;
use teo_parser::ast::unit::Unit;
use teo_parser::r#type::Type;
use teo_parser::utils::top_filter::top_filter_for_pipeline;
use teo_result::{Error, Result};
use crate::namespace::Namespace;
use crate::object::Object;
use crate::pipeline::item::BoundedItem;
use crate::pipeline::Pipeline;
use crate::schema::fetch::fetch_argument_list::fetch_argument_list_or_empty;
use crate::schema::fetch::fetchers::fetch_identifier::fetch_identifier_path;

pub fn fetch_pipeline<I>(pipeline: &teo_parser::ast::pipeline::Pipeline, schema: &Schema, info_provider: &I, expect: &Type, namespace: &Namespace) -> Result<Object> where I: InfoProvider {
    fetch_pipeline_unit(pipeline.unit.as_ref(), schema, info_provider, expect, namespace)
}

fn fetch_pipeline_unit<I>(unit: &Unit, schema: &Schema, info_provider: &I, expect: &Type, namespace: &Namespace) -> Result<Object> where I: InfoProvider {
    let mut pipeline = Pipeline::new();
    let mut current_space: Option<&teo_parser::ast::namespace::Namespace> = None;
    for (index, expression) in unit.expressions.iter().enumerate() {
        if let Some(identifier) = expression.kind.as_identifier() {
            if let Some(this_top) = if current_space.is_some() {
                current_space.unwrap().find_top_by_name(identifier.name(), &top_filter_for_pipeline(), info_provider.availability())
            } else {
                let path = fetch_identifier_path(identifier, schema, info_provider, expect, namespace, &top_filter_for_pipeline()).unwrap();
                schema.find_top_by_path(&path)
            } {
                match this_top {
                    Top::Namespace(namespace) => {
                        current_space = Some(namespace);
                    }
                    Top::PipelineItemDeclaration(pipeline_item_declaration) => {
                        let argument_list = unit.expressions.get(index + 1).map(|e| e.kind.as_argument_list()).flatten();
                        let arguments = fetch_argument_list_or_empty(argument_list, schema, info_provider, namespace)?;
                        let pipeline_item = namespace.pipeline_item_at_path(&pipeline_item_declaration.str_path()).unwrap();
                        pipeline.items.push(BoundedItem {
                            path: pipeline_item.path.clone(),
                            arguments,
                            call: pipeline_item.call.clone(),
                        });
                        current_space = None;
                    }
                    _ => unreachable!()
                }
            } else {
                Err(Error::new("pipeline item not found"))?
            }
        }
    }
    Ok(Object::from(pipeline))
}