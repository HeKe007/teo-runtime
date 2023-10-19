use std::ops::Deref;
use teo_parser::ast::info_provider::InfoProvider;
use teo_parser::ast::schema::Schema;
use teo_parser::ast::span::Span;
use teo_parser::diagnostics::diagnostics::{Diagnostics, DiagnosticsError};
use crate::namespace::Namespace;
use crate::result::Result;
use crate::schema::load::load_server::load_server;

pub fn load_schema(main_namespace: &mut Namespace, schema: &Schema) -> Result<()> {

    // diagnostics for schema loading
    let mut diagnostics = Diagnostics::new();

    // some of these are just load from schema, while some are validate and load

    // load server
    let mut server_loaded = false;
    if let Some(server) = schema.server() {
        if server.is_available() {
            let dest_namespace = main_namespace.namespace_mut_or_create_at_path(&server.namespace_str_path());
            load_server(dest_namespace, schema, server, &mut diagnostics)?;
            server_loaded = true;
        }
    }
    if !server_loaded {
        let source = schema.main_source();
        diagnostics.insert(DiagnosticsError::new(Span::default(), "server config is not found", source.file_path.clone()));
    }

    // load connectors

    // load_connectors(main_namespace, schema);

    //
    // pub fn server(&self) -> Option<&Config> {
    //     self.references.server.as_ref().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap())
    // }
    //
    // pub fn debug(&self) -> Option<&Config> {
    //     self.references.debug.as_ref().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap())
    // }
    //
    // pub fn test(&self) -> Option<&Config> {
    //     self.references.test.as_ref().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap())
    // }
    //
    // pub fn connectors(&self) -> Vec<&Config> {
    //     self.references.connectors.iter().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap()).collect()
    // }
    //
    // pub fn entities(&self) -> Vec<&Config> {
    //     self.references.entities.iter().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap()).collect()
    // }
    //
    // pub fn clients(&self) -> Vec<&Config> {
    //     self.references.clients.iter().map(|path| self.find_top_by_path(path).unwrap().as_config().unwrap()).collect()
    // }
    //
    // pub fn enums(&self) -> Vec<&Enum> {
    //     self.references.enums.iter().map(|path| self.find_top_by_path(path).unwrap().as_enum().unwrap()).collect()
    // }
    //
    // pub fn models(&self) -> Vec<&Model> {
    //     self.references.models.iter().map(|path| self.find_top_by_path(path).unwrap().as_model().unwrap()).collect()
    // }
    //
    // pub fn data_sets(&self) -> Vec<&DataSet> {
    //     self.references.data_sets.iter().map(|path| self.find_top_by_path(path).unwrap().as_data_set().unwrap()).collect()
    // }
    //
    // pub fn interfaces(&self) -> Vec<&InterfaceDeclaration> {
    //     self.references.interfaces.iter().map(|path| self.find_top_by_path(path).unwrap().as_interface_declaration().unwrap()).collect()
    // }
    //
    // pub fn namespaces(&self) -> Vec<&Namespace> {
    //     self.references.namespaces.iter().map(|path| self.find_top_by_path(path).unwrap().as_namespace().unwrap()).collect()
    // }
    //
    // pub fn config_declarations(&self) -> Vec<&ConfigDeclaration> {
    //     self.references.config_declarations.iter().map(|path| self.find_top_by_path(path).unwrap().as_config_declaration().unwrap()).collect()
    // }
    //
    // pub fn decorator_declarations(&self) -> Vec<&DecoratorDeclaration> {
    //     self.references.decorator_declarations.iter().map(|path| self.find_top_by_path(path).unwrap().as_decorator_declaration().unwrap()).collect()
    // }
    //
    // pub fn pipeline_item_declarations(&self) -> Vec<&PipelineItemDeclaration> {
    //     self.references.pipeline_item_declarations.iter().map(|path| self.find_top_by_path(path).unwrap().as_pipeline_item_declaration().unwrap()).collect()
    // }
    //
    // pub fn middleware_declarations(&self) -> Vec<&MiddlewareDeclaration> {
    //     self.references.middlewares.iter().map(|path| self.find_top_by_path(path).unwrap().as_middleware_declaration().unwrap()).collect()
    // }
    //
    // pub fn handler_group_declarations(&self) -> Vec<&HandlerGroupDeclaration> {
    //     self.references.handler_groups.iter().map(|path| self.find_top_by_path(path).unwrap().as_handler_group_declaration().unwrap()).collect()
    // }
    //
    // pub fn struct_declarations(&self) -> Vec<&StructDeclaration> {
    //     self.references.struct_declarations.iter().map(|path| self.find_top_by_path(path).unwrap().as_struct_declaration().unwrap()).collect()
    // }
    Ok(())
}