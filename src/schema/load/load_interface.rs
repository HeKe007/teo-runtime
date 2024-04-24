use teo_parser::ast::schema::Schema;
use teo_parser::diagnostics::diagnostics::Diagnostics;
use teo_parser::traits::has_availability::HasAvailability;
use teo_parser::traits::identifiable::Identifiable;
use teo_parser::traits::info_provider::InfoProvider;
use teo_parser::traits::named_identifiable::NamedIdentifiable;
use teo_parser::traits::resolved::Resolve;
use crate::namespace::Namespace;
use teo_result::Result;
use crate::interface;
use crate::interface::Interface;
use crate::model::field::is_optional::{IsOptional};
use crate::schema::fetch::fetch_decorator_arguments::fetch_decorator_arguments;
use crate::schema::load::load_comment::load_comment;

pub fn load_interface(main_namespace: &mut Namespace, schema: &Schema, interface_declaration: &teo_parser::ast::interface::InterfaceDeclaration, diagnostics: &mut Diagnostics) -> Result<()> {
    let mut interface = Interface::new();
    interface.path = interface_declaration.string_path().clone();
    interface.parser_path = interface_declaration.path().clone();
    interface.comment = load_comment(interface_declaration.comment());
    if let Some(generics_declaration) = interface_declaration.generics_declaration() {
        for gen in generics_declaration.identifiers() {
            interface.generic_names.push(gen.name().to_owned().clone());
        }
    }
    for t in interface_declaration.extends() {
        interface.extends.push(t.resolved().clone());
    }
    for decorator in interface_declaration.decorators() {
        if let Some(decorator_declaration) = schema.find_top_by_path(decorator.resolved()).unwrap().as_decorator_declaration() {
            if let Some(decorator_implementation) = main_namespace.interface_decorator_at_path(&decorator_declaration.str_path()) {
                let args = fetch_decorator_arguments(decorator, schema, interface_declaration, main_namespace, diagnostics)?;
                decorator_implementation.call.call(args, &mut interface)?;
            }
        }
    }
    for field_declaration in interface_declaration.fields() {
        if field_declaration.is_available() {
            interface.fields.insert(
                field_declaration.identifier().name().to_owned(),
                load_interface_field(main_namespace, field_declaration, schema, diagnostics)?,
            );
        }
    }
    interface.shape = interface_declaration.resolved().shape().clone();
    let dest_namespace = main_namespace.namespace_mut_or_create_at_path(&interface_declaration.namespace_str_path());
    dest_namespace.interfaces.insert(interface_declaration.identifier().name().to_owned(), interface);

    Ok(())
}

fn load_interface_field(main_namespace: &mut Namespace, field_declaration: &teo_parser::ast::field::Field, schema: &Schema, diagnostics: &mut Diagnostics) -> Result<interface::Field> {
    let mut field = interface::Field::new();
    field.name = field_declaration.identifier().name().to_owned();
    field.comment = load_comment(field_declaration.comment());
    if field_declaration.type_expr().resolved().is_optional() {
        field.set_optional();
    } else {
        field.set_required();
    }
    field.r#type = field_declaration.type_expr().resolved().clone();
    Ok(field)
}