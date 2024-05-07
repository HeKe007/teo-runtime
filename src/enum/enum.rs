use maplit::btreemap;
use std::collections::BTreeMap;
use serde::Serialize;
use crate::comment::Comment;
use crate::r#enum::member::Member;
use crate::traits::documentable::Documentable;
use crate::traits::named::Named;
use crate::value::Value;

#[derive(Debug, Serialize)]
pub struct Enum {
    pub path: Vec<String>,
    pub comment: Option<Comment>,
    pub option: bool,
    pub interface: bool,
    pub members: Vec<Member>,
    pub data: BTreeMap<String, Value>,
    pub cache: Cache,
}

impl Enum {

    pub fn new() -> Self {
        Self {
            path: vec![],
            comment: None,
            option: false,
            interface: false,
            members: vec![],
            data: btreemap! {},
            cache: Cache {
                member_names: vec![]
            }
        }
    }

    pub fn path(&self) -> Vec<&str> {
        self.path.iter().map(AsRef::as_ref).collect()
    }

    pub fn finalize(&mut self) {
        self.cache.member_names = self.members.iter().map(|m| m.name.clone()).collect();
    }

    pub fn members(&self) -> &Vec<Member> {
        &self.members
    }
}

#[derive(Debug, Serialize)]
pub struct Cache {
    pub member_names: Vec<String>,
}

impl Named for Enum {

    fn name(&self) -> &str {
        self.path.last().unwrap().as_str()
    }
}

impl Documentable for Enum {

    fn comment(&self) -> Option<&Comment> {
        self.comment.as_ref()
    }

    fn kind(&self) -> &'static str {
        "enum"
    }
}