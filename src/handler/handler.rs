use educe::Educe;
use serde::Serialize;
use teo_parser::ast::handler::HandlerInputFormat;
use teo_parser::r#type::Type;
use crate::middleware::next::Next;
use crate::model::Model;
use crate::traits::named::Named;

#[derive(Debug, Serialize, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Put,
    Delete,
    Options,
}

impl Method {
    pub fn capitalized_name(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Options => "OPTIONS",
        }
    }
}

#[derive(Educe)]
#[educe(Debug)]
#[derive(Serialize, Clone)]
pub struct Handler {
    pub path: Vec<String>,
    pub namespace_path: Vec<String>,
    pub input_type: Type,
    pub output_type: Type,
    pub nonapi: bool,
    pub format: HandlerInputFormat,
    pub method: Method,
    pub url: Option<String>,
    pub interface: Option<String>,
    pub ignore_prefix: bool,
    #[serde(skip)] #[educe(Debug(ignore))]
    pub call: &'static dyn Next,
}

impl Handler {

    pub fn has_custom_url_args(&self) -> bool {
        if self.url.is_some() {
            self.url.as_ref().unwrap().contains("*") || self.url.as_ref().unwrap().contains(":")
        } else {
            false
        }
    }

    pub fn has_body_input(&self) -> bool {
        !(self.method == Method::Get || self.method == Method::Delete)
    }

    pub fn custom_url_args_path(&self) -> Option<Vec<String>> {
        if let Some(interface) = &self.interface {
            let mut result = self.path.clone();
            result.push(interface.clone());
            Some(result)
        } else {
            None
        }
    }
}

impl Named for Handler {

    fn name(&self) -> &str {
        self.path.last().map(|s| s.as_str()).unwrap()
    }
}
