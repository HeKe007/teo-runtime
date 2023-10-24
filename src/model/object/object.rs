use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Serialize, Serializer};
use teo_teon::Value;
use teo_result::{Error, Result};
use tokio::sync::Mutex as TokioMutex;
use crate::action::Action;
use crate::connection::transaction;
use crate::model::{Field, Model};
use key_path::{path, KeyPath};
use crate::model::field::named::Named;
use async_recursion::async_recursion;
use maplit::hashmap;
use indexmap::IndexMap;
use itertools::Itertools;
use teo_teon::teon;
use crate::action::action::*;
use crate::model::object::input::Input;
use crate::model::object::input::Input::{AtomicUpdater, SetValue};
use crate::model::relation::Relation;
use crate::{object, pipeline, request};
use crate::model::relation::delete::Delete;
use crate::namespace::Namespace;
use crate::optionality::Optionality;
use crate::readwrite::write::Write;
use crate::utils::ContainsStr;

#[derive(Debug, Clone)]
pub struct Object {
    inner: Arc<ObjectInner>
}

impl Object {

    pub fn new(request_ctx: Option<request::Ctx>, transaction_ctx: transaction::Ctx, model: &'static Model, action: Action) -> Object {
        Object {
            inner: Arc::new(ObjectInner {
                request_ctx,
                transaction_ctx,
                model,
                action,
                is_initialized: AtomicBool::new(false),
                is_new: AtomicBool::new(true),
                is_modified: AtomicBool::new(false),
                is_partial: AtomicBool::new(false),
                is_deleted: AtomicBool::new(false),
                inside_before_save_callback: AtomicBool::new(false),
                inside_after_save_callback: AtomicBool::new(false),
                selected_fields: Arc::new(Mutex::new(Vec::new())),
                modified_fields: Arc::new(Mutex::new(BTreeSet::new())),
                previous_value_map: Arc::new(Mutex::new(BTreeMap::new())),
                value_map: Arc::new(Mutex::new(BTreeMap::new())),
                atomic_updater_map: Arc::new(Mutex::new(BTreeMap::new())),
                relation_query_map: Arc::new(Mutex::new(BTreeMap::new())),
                relation_mutation_map: Arc::new(TokioMutex::new(BTreeMap::new())),
                cached_property_map: Arc::new(Mutex::new(BTreeMap::new())),
                object_set_map: Arc::new(TokioMutex::new(BTreeMap::new())),
                object_set_many_map: Arc::new(TokioMutex::new(BTreeMap::new())),
                object_connect_map: Arc::new(TokioMutex::new(BTreeMap::new())),
                object_disconnect_map: Arc::new(TokioMutex::new(BTreeMap::new())),
                ignore_relation: None,
            })
        }
    }

    pub fn transaction_ctx(&self) -> transaction::Ctx {
        self.inner.transaction_ctx.clone()
    }

    pub fn request_ctx(&self) -> Option<request::Ctx> {
        self.inner.request_ctx.clone()
    }

    fn model(&self) -> &'static Model {
        self.inner.model
    }

    fn namespace(&self) -> &'static Namespace {
        self.inner.transaction_ctx.namespace()
    }

    fn pipeline_ctx_for_path_and_value(&self, path: KeyPath, value: Value) -> pipeline::Ctx {
        pipeline::Ctx::new(object::Object::from(value), self.clone(), path, self.action(), self.transaction_ctx(), self.request_ctx())
    }

    pub async fn set_teon(&self, value: &Value) -> Result<()> {
        self.set_teon_with_path_and_user_mode(value, &path![], true).await
    }

    pub async fn update_teon(&self, value: &Value) -> Result<()> {
        check_user_json_keys(value.as_dictionary().unwrap(), &self.model().cache.input_keys.iter().map(|k| k.as_str()).collect(), self.model())?;
        for (key, value) in value.as_dictionary().unwrap() {
            if self.model().field(key).is_some() {
                self.set_value(key, value.clone())?;
            } else if self.model().property(key).is_some() {
                self.set_property(key, value).await?;
            }
        }
        self.inner.is_initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) async fn set_teon_with_path(&self, json_value: &Value, path: &KeyPath) -> Result<()> {
        self.set_teon_with_path_and_user_mode(json_value, path, false).await
    }

    pub(crate) async fn set_teon_with_path_and_user_mode(&self, value: &Value, path: &KeyPath, bypass_permission_check: bool) -> Result<()> {
        let model = self.model();
        // permission
        if !bypass_permission_check {
            self.check_model_write_permission(path).await?;
        }
        // get value map
        let value_map = value.as_dictionary().unwrap();
        let value_map_keys: Vec<&str> = value_map.keys().map(|k| k.as_str()).collect();
        // check keys
        if bypass_permission_check {
            check_user_json_keys(value_map, &model.cache.input_keys.iter().map(|k| k.as_str()).collect(), model)?;
        }
        // find keys to iterate
        let initialized = self.inner.is_initialized.load(Ordering::SeqCst);
        let keys = if initialized {
            self.model().all_keys().iter().filter(|k| value_map_keys.contains(*k)).map(|k| *k).collect::<Vec<&str>>()
        } else {
            self.model().all_keys().clone()
        };
        // assign values
        for key in keys {
            let path = path + key;
            if let Some(field) = self.model().field(key) {
                let need_to_trigger_default_value = if initialized { false } else {
                    !value_map_keys.contains(&key)
                };
                if need_to_trigger_default_value {
                    // apply default values
                    if let Some(argument) = &field.default {
                        if let Some(pipeline) = argument.as_pipeline() {
                            let ctx = self.pipeline_ctx_for_path_and_value(path.clone(), Value::Null);
                            let value: Value = ctx.run_pipeline(pipeline).await?.try_into()?;
                            self.set_value_to_value_map(key, value);
                        } else if let Some(value) = argument.as_teon() {
                            self.set_value_to_value_map(key, value.clone());
                        }
                    }
                } else {
                    if !bypass_permission_check {
                        self.check_field_write_permission(field, &path).await?;
                    }
                    // set_value_to_value_map
                    let value = value_map.get(key).unwrap();
                    match Input::decode_field(value) {
                        AtomicUpdater(updator) => self.set_value_to_atomic_updator_map(key, updator),
                        SetValue(value) => {
                            // on set pipeline
                            let ctx = self.pipeline_ctx_for_path_and_value(path.clone(), value);
                            let value: Value = ctx.run_pipeline(&field.on_set).await?.try_into()?;
                            self.check_write_rule(key, &value, &path).await?;
                            self.set_value_to_value_map(key, value.clone());
                        }
                    }
                }
            } else if let Some(_) = self.model().relation(key) {
                let manipulation = match value_map.get(&key.to_string()) {
                    Some(value) => value,
                    None => continue,
                };
                self.set_value_to_relation_manipulation_map(key, manipulation).await;
            } else if let Some(property) = self.model().property(key) {
                if value_map_keys.contains(&key) {
                    if let Some(setter) = property.setter.as_ref() {
                        let value = value_map.get(&key.to_string()).unwrap();
                        let input_result = Input::decode_field(value);
                        let value = match input_result {
                            SetValue(v) => v,
                            _ => return Err(Error::unexpected_input_type("value", &(path + key))),
                        };
                        let ctx = self.pipeline_ctx_for_path_and_value(path.clone(), value);
                        let _ = ctx.run_pipeline(setter).await?;
                    }
                }
            }
        };
        // set flag
        self.inner.is_initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn check_model_write_permission<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let ctx = self.pipeline_ctx_for_path_and_value(path.as_ref().clone(), Value::Null);
        ctx.run_pipeline(&self.model().can_mutate).await?;
        Ok(())
    }

    async fn check_model_read_permission<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let ctx = self.pipeline_ctx_for_path_and_value(path.as_ref().clone(), Value::Null);
        ctx.run_pipeline(&self.model().can_read).await?;
        Ok(())
    }

    async fn check_field_write_permission<'a>(&self, field: &Field, path: impl AsRef<KeyPath>) -> Result<()> {
        let ctx = self.pipeline_ctx_for_path_and_value(path.as_ref().clone(), Value::Null);
        ctx.run_pipeline(&field.can_mutate).await?;
        Ok(())
    }

    async fn check_field_read_permission<'a>(&self, field: &Field, path: impl AsRef<KeyPath>) -> Result<()> {
        let ctx = self.pipeline_ctx_for_path_and_value(path.as_ref().clone(), Value::Null);
        ctx.run_pipeline(&field.can_read).await?;
        Ok(())
    }

    fn record_previous_value_for_field_if_needed(&self, key: &str) {
        let field = self.model().field(key).unwrap();
        if !self.is_new() && field.previous.is_keep() {
            if self.inner.previous_value_map.lock().unwrap().get(field.name()).is_none() {
                self.inner.previous_value_map.lock().unwrap().insert(field.name().to_string(), self.get_value(field.name()).unwrap());
            }
        }
    }

    async fn check_write_rule(&self, key: impl AsRef<str>, value: &Value, path: &KeyPath) -> crate::path::Result<()> {
        let field = self.model().field(key.as_ref()).unwrap();
        let is_new = self.is_new();
        let valid = match &field.write {
            Write::NoWrite => false,
            Write::Write => true,
            Write::WriteOnCreate => is_new,
            Write::WriteOnce => if is_new { true } else { self.get_value(key.as_ref()).unwrap().is_null() },
            Write::WriteNonNull => if is_new { true } else { !value.is_null() },
            Write::WriteIf(pipeline) => {
                let ctx = self.pipeline_ctx_for_path_and_value(path + key.as_ref(), value.clone());
                ctx.run_pipeline(pipeline).await.is_ok()
            }
        };
        if !valid {
            Err(crate::path::Error::value_error(path + key.as_ref(), "unexpected key"))
        } else {
            Ok(())
        }
    }

    fn set_modified_field(&self, key: &str) {
        if !self.is_new() {
            self.inner.is_modified.store(true, Ordering::SeqCst);
            self.inner.modified_fields.lock().unwrap().insert(key.to_string());
        }
    }

    fn set_value_to_atomic_updator_map(&self, key: &str, value: Value) {
        self.inner.atomic_updater_map.lock().unwrap().insert(key.to_string(), value);
        self.set_modified_field(key);
    }

    async fn set_value_to_relation_manipulation_map(&self, key: &str, value: &Value) {
        self.inner.relation_mutation_map.lock().await.insert(key.to_string(), value.clone());
        self.set_modified_field(key);
    }

    pub fn set(&self, key: impl AsRef<str>, value: impl Into<Value>) -> Result<()> {
        self.set_value(key, value.into())
    }

    pub fn set_value(&self, key: impl AsRef<str>, value: Value) -> Result<()> {
        let model_keys = &self.model().cache.save_keys_and_virtual_keys;
        if !model_keys.contains_str(key.as_ref()) {
            return Err(Error::new(format!("invalid key {}", key.as_ref())));
        }
        self.set_value_to_value_map(key.as_ref(), value);
        Ok(())
    }

    pub async fn set_property(&self, key: &str, value: impl Into<Value>) -> Result<()> {
        let property = self.model().property(key).unwrap();
        let setter = property.setter.as_ref().unwrap();
        let ctx = self.pipeline_ctx_for_path_and_value(path![key], value.into());
        ctx.run_pipeline(setter).await?;
        Ok(())
    }

    pub(crate) fn set_from_database_result_value(&self, value: &Value, select: Option<&Value>, include: Option<&Value>) {
        let model = self.model();
        for (k, v) in value.as_dictionary().unwrap() {
            if let Some(_) = model.field(k) {
                self.set_value_to_value_map(k, v.clone());
            } else if let Some(relation) = model.relation(k) {
                self.inner.relation_query_map.lock().unwrap().insert(k.to_owned(), vec![]);
                let include_arg = include.unwrap().get(k).unwrap();
                let inner_select = include_arg.as_dictionary().map(|m| m.get("select")).flatten();
                let inner_include = include_arg.as_dictionary().map(|m| m.get("include")).flatten();
                for v in v.as_array().unwrap() {
                    let action = Action::from_u32(FIND | (if relation.is_vec { MANY } else { SINGLE }) | NESTED );
                    let object = self.graph().new_object(relation.model(), action, self.action_source().clone(), self.connection()).unwrap();
                    object.set_from_database_result_value(v, inner_select, inner_include);
                    self.inner.relation_query_map.lock().unwrap().get_mut(k).unwrap().push(object);
                }
            } else if let Some(_property) = model.property(k) {
                self.inner.cached_property_map.lock().unwrap().insert(k.to_owned(), v.clone());
            }
        }
        self.set_select(select).unwrap();
        self.inner.is_new.store(false, Ordering::SeqCst);
        self.inner.is_modified.store(false, Ordering::SeqCst);
    }

    fn set_value_to_value_map(&self, key: &str, value: Value) {
        // record previous value if needed
        self.record_previous_value_for_field_if_needed(key);

        if value.is_null() {
            self.inner.value_map.lock().unwrap().remove(key);
        } else {
            self.inner.value_map.lock().unwrap().insert(key.to_string(), value);
        }
        if !self.is_new() {
            self.inner.is_modified.store(true, Ordering::SeqCst);
            self.inner.modified_fields.lock().unwrap().insert(key.to_string());
            if let Some(properties) = self.model().field_property_map().get(key) {
                for property in properties {
                    self.inner.modified_fields.lock().unwrap().insert(property.to_string());
                    self.inner.cached_property_map.lock().unwrap().remove(&property.to_string());
                }
            }
        }
    }

    pub fn get_query_relation_object(&self, key: impl AsRef<str>) -> Result<Option<Object>> {
        let key = key.as_ref();
        let model_keys = self.model().all_keys();
        if !model_keys.contains(&key) {
            return Err(Error::invalid_key(key, self.model()));
        }
        match self.inner.relation_query_map.lock().unwrap().get(key) {
            Some(list) => Ok(list.get(0).cloned()),
            None => Ok(None)
        }
    }

    pub fn get_mutation_relation_object(&self, key: impl AsRef<str>) -> Result<Option<Object>> {
        let key = key.as_ref();
        let model_keys = self.model().all_keys();
        if !model_keys.contains(&key) {
            return Err(Error::invalid_key(key, self.model()));
        }
        match self.inner.relation_query_map.lock().unwrap().get(key) {
            Some(list) => Ok(list.get(0).cloned()),
            None => Ok(None)
        }
    }

    pub fn has_query_relation_fetched(&self, key: impl AsRef<str>) -> bool {
        self.inner.relation_query_map.lock().unwrap().contains_key(key.as_ref())
    }

    pub fn has_mutation_relation_fetched(&self, key: impl AsRef<str>) -> bool {
        self.inner.relation_query_map.lock().unwrap().contains_key(key.as_ref())
    }

    pub fn get_relation_vec(&self, key: impl AsRef<str>) -> Result<Vec<Object>> {
        let key = key.as_ref();
        let model_keys = self.model().all_keys();
        if !model_keys.contains(&key) {
            return Err(Error::invalid_key(key, self.model()));
        }
        match self.inner.relation_query_map.lock().unwrap().get(key) {
            Some(list) => Ok(list.clone()),
            None => Ok(vec![]),
        }
    }

    pub async fn get_property<T>(&self, key: &str) -> Result<T> where T: From<Value> {
        let property = self.model().property(key.as_ref()).unwrap();
        if property.cached {
            if let Some(value) = self.inner.cached_property_map.lock().unwrap().get(key) {
                return Ok(value.clone().into());
            }
        }
        let getter = property.getter.as_ref().unwrap();
        let ctx = self.pipeline_ctx_for_path_and_value(path![key], Value::Null);
        let value: Value = ctx.run_pipeline(getter).await?.try_into()?;
        if property.cached {
            self.inner.cached_property_map.lock().unwrap().insert(key.to_string(), value.clone());
        }
        Ok(value.into())
    }

    pub fn get<T>(&self, key: impl AsRef<str>) -> Result<T> where T: From<Value> {
        match self.get_value(key) {
            Ok(optional_value) => {
                Ok(optional_value.into())
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    pub(crate) fn get_previous_value(&self, key: impl AsRef<str>) -> Result<Value> {
        let key = key.as_ref();
        let model_keys = self.model().all_keys();
        if !model_keys.contains(&key) {
            let model = self.model();
            return Err(Error::invalid_key(key, model));
        }
        let map = self.inner.previous_value_map.lock().unwrap();
        match map.get(key) {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Null),
        }
    }

    fn get_value_map_value(&self, key: &str) -> Value {
        match self.inner.value_map.lock().unwrap().get(key) {
            Some(value) => value.clone(),
            None => Value::Null,
        }
    }

    pub fn get_value(&self, key: impl AsRef<str>) -> Result<Value> {
        let model_keys = self.model().all_keys();
        if !model_keys.contains(&key.as_ref()) {
            return Err(Error::invalid_key(key, self.model()));
        }
        Ok(self.get_value_map_value(key.as_ref()))
    }

    pub(crate) fn get_atomic_updator(&self, key: &str) -> Option<Value> {
        self.inner.atomic_updater_map.lock().unwrap().get(key).cloned()
    }

    pub fn set_select(&self, select: Option<&Value>) -> Result<()> {
        if select.is_none() {
            return Ok(());
        }
        let mut true_list: Vec<&str> = vec![];
        let mut false_list: Vec<&str> = vec![];
        let map = select.unwrap().as_dictionary().unwrap();
        for (key, value) in map {
            let bool_value = value.as_bool().unwrap();
            if bool_value {
                true_list.push(key.as_str());
            } else {
                false_list.push(key.as_str());
            }
        }
        let true_empty = true_list.is_empty();
        let false_empty = false_list.is_empty();
        if true_empty && false_empty {
            // just do nothing
            return Ok(());
        } else if !false_empty {
            // all - false
            let mut result: Vec<String> = vec![];
            self.model().all_keys().iter().for_each(|k| {
                if let Some(field) = self.model().field(k) {
                    if !false_list.contains(&&***&k) {
                        result.push(field.name.to_string());
                    }
                } else if let Some(property) = self.model().property(k) {
                    if !false_list.contains(&&***&k) {
                        result.push(property.name.to_string());
                    }
                }
            });
            *self.inner.selected_fields.lock().unwrap() = result;
            return Ok(());
        } else {
            // true
            let mut result: Vec<String> = vec![];
            self.model().all_keys().iter().for_each(|k| {
                if let Some(field) = self.model().field(k) {
                    if true_list.contains(&&***&k) {
                        result.push(field.name.to_string());
                    }
                } else if let Some(property) = self.model().property(k) {
                    if true_list.contains(&&***&k) {
                        result.push(property.name.to_string());
                    }
                }
            });
            *self.inner.selected_fields.lock().unwrap() = result;
            return Ok(());
        }
    }

    #[async_recursion]
    pub(crate) async fn apply_on_save_pipeline_and_validate_required_fields(&self, path: &KeyPath, ignore_required_relation: bool) -> Result<()> {
        // apply on save pipeline first
        let model_keys = self.model().save_keys();
        for key in model_keys {
            let field = self.model().field(key);
            if field.is_none() {
                continue;
            }
            let field = field.unwrap();
            if field.needs_on_save_callback() {
                let initial_value = match self.inner.value_map.lock().unwrap().deref().get(&key.to_string()) {
                    Some(value) => {
                        value.clone()
                    }
                    None => {
                        Value::Null
                    }
                };
                let ctx = self.pipeline_ctx_for_path_and_value(path + field.name(), initial_value);
                let value: Value = ctx.run_pipeline(&field.on_save).await?.try_into()?;
                self.inner.value_map.lock().unwrap().insert(key.to_string(), value);
                self.set_modified_field(key);
            }
        }
        // validate required fields
        for key in model_keys {
            if let Some(field) = self.model().field(key) {
                if field.auto || field.auto_increment || field.foreign_key {
                    continue
                }
                match &field.optionality {
                    Optionality::Optional => (),
                    Optionality::Required => {
                        let value = self.get_value(key).unwrap();
                        if value.is_null() {
                            return Err(Error::missing_required_input(path + *key));
                        }
                    }
                    Optionality::PresentWith(field_names) => {
                        let value = self.get_value(key).unwrap();
                        if value.is_null() {
                            for field_name in field_names {
                                match field_name {
                                    Value::Array(names) => {
                                        for name in names {
                                            let name = name.as_str().unwrap();
                                            let value_at_name = self.get_value(name).unwrap();
                                            if !value_at_name.is_null() {
                                                return Err(Error::missing_required_input_with_type(key, path))
                                            }
                                        }
                                    }
                                    Value::String(name) => {
                                        let value_at_name = self.get_value(name).unwrap();
                                        if !value_at_name.is_null() {
                                            return Err(Error::missing_required_input_with_type(key, path))
                                        }
                                    }
                                    _ => unreachable!()
                                }
                            }
                        }
                    }
                    Optionality::PresentWithout(field_names) => {
                        let value = self.get_value(key).unwrap();
                        if value.is_null() {
                            for field_name in field_names {
                                match field_name {
                                    Value::Array(names) => {
                                        for name in names {
                                            let name = name.as_str().unwrap();
                                            let value_at_name = self.get_value(name).unwrap();
                                            if !value_at_name.is_null() {
                                                break;
                                            }
                                            return Err(Error::missing_required_input_with_type(key, path));
                                        }
                                    }
                                    Value::String(name) => {
                                        let value_at_name = self.get_value(name).unwrap();
                                        if value_at_name.is_null() {
                                            return Err(Error::missing_required_input_with_type(key, path))
                                        }
                                    }
                                    _ => unreachable!()
                                }
                            }
                        }
                    }
                    Optionality::PresentIf(pipeline) => {
                        let value = self.get_value(key).unwrap();
                        if value.is_null() {
                            let ctx = self.pipeline_ctx_for_path_and_value(path + field.name(), Value::Null);
                            let invalid = ctx.run_pipeline(pipeline).await.is_err();
                            if invalid {
                                return Err(Error::new("missing required input"));
                            }
                        }
                    }
                }
            }
        }
        // validate required relations
        for key in self.model().relation_output_keys() {
            if let Some(relation) = self.model().relation(key) {
                if let Some(ignore) = &self.inner.ignore_relation {
                    if ignore.as_str() == relation.name() {
                        continue
                    }
                }
                if self.is_new() && relation.is_required() && !relation.is_vec {
                    if ignore_required_relation {
                        continue
                    }
                    // check whether foreign key is received or a value is provided
                    let map = self.inner.relation_mutation_map.lock().await;
                    if map.get(&key.to_string()).is_some() {
                        continue
                    }
                    for field_name in relation.fields() {
                        if self.get_value(field_name).unwrap().is_null() {
                            return Err(Error::missing_required_input(&(path + *key)));
                        }
                    }
                    continue
                }
            }
        }
        Ok(())
    }

    pub(crate) fn clear_new_state(&self) {
        let is_new = self.is_new();
        self.inner.is_new.store(false, Ordering::SeqCst);
        self.inner.is_modified.store(false, Ordering::SeqCst);
        // todo: set self as identity when identity
    }

    pub(crate) fn clear_state(&self) {
        self.inner.is_new.store(false, Ordering::SeqCst);
        self.inner.is_modified.store(false, Ordering::SeqCst);
        *self.inner.modified_fields.lock().unwrap() = BTreeSet::new();
    }

    #[async_recursion]
    pub(crate) async fn delete_from_database(&self) -> Result<()> {
        let model = self.model();
        let namespace = self.namespace();
        // check deny first
        for relation in model.relations() {
            if relation.through().is_some() {
                continue
            }
            let (opposite_model, opposite_relation) = namespace.opposite_relation(relation);
            if let Some(opposite_relation) = opposite_relation {
                if opposite_relation.delete == Delete::Deny {
                    let finder = self.intrinsic_where_unique_for_relation(relation);
                    let count = self.transaction_ctx().count(opposite_model, &finder, self.connection()).await.unwrap();
                    if count > 0 {
                        return Err(Error::deletion_denied(relation.name()));
                    }
                }
            }
        }
        // real delete
        self.connection().delete_object(self, ).await?;
        // nullify and cascade
        for relation in model.relations() {
            if relation.through().is_some() {
                continue
            }
            let (opposite_model, opposite_relation) = graph.opposite_relation(relation);
            if let Some(opposite_relation) = opposite_relation {
                match opposite_relation.delete_rule() {
                    DeleteRule::Default => {}, // do nothing
                    DeleteRule::Deny => {}, // done before
                    DeleteRule::Nullify => {
                        if !opposite_relation.has_foreign_key() {
                            continue
                        }
                        let finder = self.intrinsic_where_unique_for_relation(relation);
                        graph.batch(opposite_model, &finder, Action::from_u32(PROGRAM_CODE | DISCONNECT | (if relation.is_vec { MANY } else { SINGLE })), Initiator::ProgramCode(self.initiator().as_req()), |object| async move {
                            for key in opposite_relation.fields() {
                                object.set_value(key, Value::Null)?;
                            }
                            object.save_with_session_and_path( &path![]).await?;
                            Ok(())
                        }, self.connection()).await?;
                    },
                    DeleteRule::Cascade => {
                        let finder = self.intrinsic_where_unique_for_relation(relation);
                        graph.batch(opposite_model, &finder, Action::from_u32(PROGRAM_CODE | DELETE | (if relation.is_vec { MANY } else { SINGLE })), Initiator::ProgramCode(self.initiator().as_req()), |object| async move {
                            object.delete_from_database().await?;
                            Ok(())
                        }, self.connection()).await?;
                    }
                }
            }
        }
        Ok(())
    }

    #[async_recursion]
    async fn save_to_database(&self) -> Result<()> {
        self.connection().save_object(self).await?;
        self.clear_new_state();
        Ok(())
    }

    fn before_save_callback_check(&self) -> Result<()> {
        let inside_before_callback = self.inner.inside_before_save_callback.load(Ordering::SeqCst);
        if inside_before_callback {
            return Err(Error::invalid_operation("Save called inside before callback."));
        }
        Ok(())
    }

    pub(crate) async fn save_with_session_and_path<'a>(&self, path: &'a KeyPath) -> Result<()> {
        self.save_with_session_and_path_and_ignore(path, false).await
    }

    #[async_recursion]
    pub(crate) async fn save_with_session_and_path_and_ignore(&self, path: &KeyPath, ignore_required_relation: bool) -> Result<()> {
        // check if it's inside before callback
        self.before_save_callback_check()?;
        let is_new = self.is_new();
        // validate and save
        let is_modified = self.is_modified();
        if is_modified || is_new {
            // apply pipeline
            self.apply_on_save_pipeline_and_validate_required_fields(path, ignore_required_relation).await?;
            self.trigger_before_save_callbacks(path).await?;
            // perform relation manipulations (has foreign key)
            self.perform_relation_manipulations(|r| r.has_foreign_key(), path).await?;
            if !self.model().is_virtual() {
                self.save_to_database().await?;
            }
        } else {
            // perform relation manipulations (has foreign key)
            self.perform_relation_manipulations(|r| r.has_foreign_key(), path).await?;
        }
        // perform relation manipulations (doesn't have foreign key)
        self.perform_relation_manipulations(|r| !r.has_foreign_key(), path).await?;
        // clear properties
        self.clear_state();
        if is_modified || is_new {
            self.trigger_after_save_callbacks(path).await?;
        }
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        self.save_with_session_and_path(&path![]).await
    }

    pub(crate) async fn save_for_seed_without_required_relation(&self) -> Result<()> {
        self.save_with_session_and_path_and_ignore(&path![], true).await
    }

    async fn trigger_before_delete_callbacks<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let model = self.model();
        let pipeline = model.before_delete_pipeline();
        let ctx = PipelineCtx::initial_state_with_object_as_value(self.clone(), self.connection(), self.initiator().as_req()).with_path(path.as_ref());
        pipeline.process_into_permission_result(ctx).await
    }

    async fn trigger_after_delete_callbacks<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let model = self.model();
        let pipeline = model.after_delete_pipeline();
        let ctx = PipelineCtx::initial_state_with_object_as_value(self.clone(), self.connection(), self.initiator().as_req()).with_path(path.as_ref());
        pipeline.process_into_permission_result(ctx).await
    }

    async fn trigger_before_save_callbacks<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let model = self.model();
        let pipeline = model.before_save_pipeline();
        let ctx = PipelineCtx::initial_state_with_object_as_value(self.clone(), self.connection(), self.initiator().as_req()).with_path(path.as_ref());
        pipeline.process_into_permission_result(ctx).await
    }

    async fn trigger_after_save_callbacks<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        let inside_after_save_callback = self.inner.inside_after_save_callback.load(Ordering::SeqCst);
        if inside_after_save_callback {
            return Ok(());
        }
        self.inner.inside_after_save_callback.store(true, Ordering::SeqCst);
        let model = self.model();
        let pipeline = model.after_save_pipeline();
        let ctx = PipelineCtx::initial_state_with_object_as_value(self.clone(), self.connection(), self.initiator().as_req()).with_path(path.as_ref());
        pipeline.process_into_permission_result(ctx).await?;
        self.inner.inside_after_save_callback.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub async fn delete(&self) -> Result<()> {
        self.trigger_before_delete_callbacks(path![]).await?;
        self.delete_from_database().await
    }

    pub(crate) async fn delete_internal<'a>(&self, path: impl AsRef<KeyPath>) -> Result<()> {
        self.check_model_write_permission(path.as_ref()).await?;
        self.trigger_before_delete_callbacks(path.as_ref()).await?;
        self.delete_from_database().await?;
        self.trigger_after_delete_callbacks(path.as_ref()).await
    }

    #[async_recursion]
    pub(crate) async fn to_json_internal<'a>(&self, path: &KeyPath) -> Result<Value> {
        // check read permission
        self.check_model_read_permission(path.as_ref()).await?;
        // output
        let select_list = self.inner.selected_fields.lock().unwrap().clone();
        let select_filter = if select_list.is_empty() { false } else { true };
        let mut map: IndexMap<String, Value> = IndexMap::new();
        let keys = self.model().output_keys();
        for key in keys {
            if let Some(relation) = self.model().relation(key) {
                if self.has_query_relation_fetched(relation.name()) {
                    if !relation.is_vec {
                        let o = self.get_query_relation_object(key).unwrap();
                        match o {
                            Some(o) => {
                                map.insert(key.to_string(), o.to_json_internal(&(path.as_ref() + relation.name())).await.unwrap());
                            },
                            None => ()
                        };
                    } else {
                        let mut result_vec = vec![];
                        let vec = self.get_relation_vec(key).unwrap();
                        for (index, o) in vec.iter().enumerate() {
                            result_vec.push(o.to_json_internal(&(path.as_ref() + relation.name() + index)).await?);
                        }
                        map.insert(key.to_string(), Value::Array(result_vec));
                    }
                }
            } else if (!select_filter) || (select_filter && select_list.contains(&key.to_string())) {
                if let Some(field) = self.model().field(key) {
                    let value = self.get_value(key).unwrap();
                    if self.check_field_read_permission(field, path.as_ref()).await.is_err() {
                        continue
                    }
                    let context = PipelineCtx::initial_state_with_object(self.clone(), self.connection(), self.initiator().as_req())
                        .with_value(value)
                        .with_path(path![*key]);
                    let value = field.perform_on_output_callback(context).await?;
                    if !value.is_null() {
                        map.insert(key.to_string(), value);
                    }
                } else if let Some(property) = self.model().property(key) {
                    if property.cached && self.inner.cached_property_map.lock().unwrap().contains_key(&key.to_string()) {
                        let value = self.inner.cached_property_map.lock().unwrap().get(&key.to_string()).unwrap().clone();
                        if !value.is_null() {
                            map.insert(key.to_string(), value);
                        }
                    } else {
                        if let Some(getter) = &property.getter {
                            let ctx = PipelineCtx::initial_state_with_object(self.clone(), self.connection(), self.initiator().as_req());
                            let value = getter.process(ctx).await?;
                            if !value.is_null() {
                                map.insert(key.to_string(), value);
                            }
                        }
                    }
                }
            }
        }
        return Ok(Value::IndexMap(map))
    }

    pub fn is_new(&self) -> bool {
        self.inner.is_new.load(Ordering::SeqCst)
    }

    pub fn is_modified(&self) -> bool {
        self.inner.is_modified.load(Ordering::SeqCst)
    }

    pub(crate) fn identifier(&self) -> Value {
        let model = self.model();
        let mut identifier: HashMap<String, Value> = HashMap::new();
        for item in model.primary_index().items() {
            let val = self.get_value(item.field_name()).unwrap();
            identifier.insert(item.field_name().to_owned(), val);
        }
        Value::HashMap(identifier)
    }

    pub(crate) fn previous_identifier(&self) -> Value {
        let model = self.model();
        let mut identifier: HashMap<String, Value> = HashMap::new();
        for item in model.primary_index().items() {
            let modify_map = self.inner.modified_fields.lock().unwrap();
            let val = if modify_map.contains(item.field_name()) {
                if let Ok(val) = self.get_previous_value(item.field_name()) {
                    if val.is_null() {
                        self.get_value(item.field_name()).unwrap()
                    } else {
                        val
                    }
                } else {
                    self.get_value(item.field_name()).unwrap()
                }
            } else {
                self.get_value(item.field_name()).unwrap()
            };
            identifier.insert(item.field_name().to_owned(), val);
        }
        Value::HashMap(identifier)
    }

    pub(crate) fn db_identifier(&self) -> Value {
        let model = self.model();
        let mut identifier: HashMap<String, Value> = HashMap::new();
        let modified_fields = self.inner.modified_fields.lock().unwrap();
        for item in model.primary_index().items() {
            let val = if modified_fields.contains(item.field_name()) {
                self.get_previous_value(item.field_name()).unwrap()
            } else {
                self.get_value(item.field_name()).unwrap()
            };
            identifier.insert(self.model().field(item.field_name()).unwrap().column_name().to_owned(), val.clone());
        }
        Value::HashMap(identifier)
    }

    async fn perform_relation_manipulations<F: Fn(&'static Relation) -> bool>(&self, f: F, path: &KeyPath) -> Result<()> {
        for relation in self.model().relations() {
            if f(relation) {
                let many = relation.is_vec;
                // programming code set
                if many {
                    let object_set_many_map = self.inner.object_set_many_map.lock().await;
                    if let Some(objects_to_set) = object_set_many_map.get(relation.name()) {
                        self.nested_set_many_relation_object_object(relation, objects_to_set, path).await?;
                    }
                } else {
                    let object_set_map = self.inner.object_set_map.lock().await;
                    if let Some(option) = object_set_map.get(relation.name()) {
                        // disconnect current
                        let value = self.intrinsic_where_unique_for_relation(relation);
                        self.nested_disconnect_relation_object(relation, &value, path).await?;
                        if let Some(new_object) = option {
                            // connect new
                            self.link_and_save_relation_object(relation, new_object, path).await?;
                        }
                    }
                }
                // programming code connections
                let object_connect_map = self.inner.object_connect_map.lock().await;
                if let Some(objects_to_connect) = object_connect_map.get(relation.name()) {
                    for object in objects_to_connect {
                        self.link_and_save_relation_object(relation.as_ref(), object, path).await?;
                    }
                }
                // programming code disconnections
                let object_disconnect_map = self.inner.object_disconnect_map.lock().await;
                if let Some(objects_to_disconnect) = object_disconnect_map.get(relation.name()) {
                    for object in objects_to_disconnect {
                        if relation.has_join_table() {
                            self.delete_join_object(object, relation.as_ref(), self.graph().opposite_relation(relation).1.unwrap(), path).await?;
                        } else if relation.has_foreign_key() {
                            self.remove_linked_values_from_related_relation(relation);
                        } else {
                            object.remove_linked_values_from_related_relation_on_related_object(relation, &object);
                            object.save_with_session_and_path(path).await?;
                        }
                    }
                }
                // value mutation
                let relation_mutation_map = self.inner.relation_mutation_map.lock().await;
                if let Some(manipulation) = relation_mutation_map.get(relation.name()) {
                    if many {
                        self.perform_relation_manipulation_many(relation, manipulation, &(path + relation.name())).await?;
                    } else {
                        self.perform_relation_manipulation_one(relation, manipulation, &(path + relation.name())).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn create_join_object<'a>(&'a self, object: &'a Object, relation: &'static Relation, opposite_relation: &'static Relation, path: &'a KeyPath) -> Result<()> {
        let join_model = AppCtx::get().unwrap().model(relation.through_path().unwrap()).unwrap().unwrap();
        let action = Action::from_u32(JOIN_CREATE | CREATE | SINGLE);
        let join_object = self.graph().new_object(join_model, action, self.action_source().clone(), self.connection())?;
        join_object.set_teon(&teon!({})).await?; // initialize
        let local = relation.local();
        let foreign = opposite_relation.local();
        let join_local_relation = join_model.relation(local).unwrap();
        self.assign_linked_values_to_related_object(&join_object, join_local_relation);
        let join_foreign_relation = join_model.relation(foreign).unwrap();
        object.assign_linked_values_to_related_object(&join_object, join_foreign_relation);
        match join_object.save_with_session_and_path(path).await {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::unexpected_input_value_with_reason("Can't create join record.", path)),
        }
    }

    async fn delete_join_object<'a>(&'a self, object: &'a Object, relation: &'static Relation, opposite_relation: &'static Relation, path: &'a KeyPath) -> Result<()> {
        let join_model = AppCtx::get().unwrap().model(relation.through_path().unwrap()).unwrap().unwrap();
        let action = Action::from_u32(JOIN_DELETE | DELETE | SINGLE);
        let local = relation.local();
        let foreign = opposite_relation.local();
        let join_local_relation = join_model.relation(local).unwrap();
        let join_foreign_relation = join_model.relation(foreign).unwrap();
        let mut finder = HashMap::new();
        for (l, f) in join_local_relation.iter() {
            finder.insert(l.to_owned(), self.get_value(f).unwrap());
        }
        for (l, f) in join_foreign_relation.iter() {
            finder.insert(l.to_owned(), object.get_value(f).unwrap());
        }
        let r#where = Value::HashMap(finder);
        let object = match self.graph().find_unique_internal(join_model, &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
            Ok(object) => object,
            Err(_) => return Err(Error::unexpected_input_value_with_reason("Join object is not found.", path)),
        }.into_not_found_error()?;
        match object.delete_from_database().await {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::unexpected_input_value_with_reason("Can't delete join record.", path)),
        }
    }

    fn assign_linked_values_to_related_object(&self, object: &Object, opposite_relation: &'static Relation) {
        for (field, reference) in opposite_relation.iter() {
            object.set_value_to_value_map(field, self.get_value_map_value(reference));
        }
    }

    fn remove_linked_values_from_related_relation(&self, relation: &'static Relation) {
        for (field, _) in relation.iter() {
            self.set_value_to_value_map(field, Value::Null)
        }
    }

    fn remove_linked_values_from_related_relation_on_related_object(&self, relation: &'static Relation, object: &Object) {
        for (_, reference) in relation.iter() {
            object.set_value_to_value_map(reference, Value::Null)
        }
    }

    async fn link_and_save_relation_object(&self, relation: &'static Relation, object: &Object, path: &KeyPath) -> Result<()> {
        let mut linked = false;
        let (_, opposite_relation) = self.graph().opposite_relation(relation);
        if let Some(opposite_relation) = opposite_relation {
            if opposite_relation.has_foreign_key() {
                self.assign_linked_values_to_related_object(object, opposite_relation);
                linked = true;
            }
        }
        object.save_with_session_and_path(path).await?;
        if !linked {
            if relation.has_foreign_key() {
                object.assign_linked_values_to_related_object(self, relation);
            } else if relation.has_join_table() {
                self.create_join_object(object, relation, opposite_relation.unwrap(), path).await?;
            }
        }
        Ok(())
    }

    async fn nested_create_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let action = Action::from_u32(NESTED | CREATE | SINGLE);
        let object = self.graph().new_object(relation.model(), action, self.action_source().clone(), self.connection())?;
        object.set_teon_with_path(value.get("create").unwrap(), path).await?;
        if let Some(opposite) = self.graph().opposite_relation(relation).1 {
            object.ignore_relation(opposite.name());
        }
        self.link_and_save_relation_object(relation, &object, path).await
    }

    async fn nested_set_many_relation_object_object(&self, relation: &'static Relation, objects: &Vec<Object>, path: &KeyPath) -> Result<()> {
        // disconnect previous
        let records = self.fetch_relation_objects(relation.name(), None).await?;
        for record in records.iter() {
            self.nested_disconnect_relation_object_object(relation, record, path).await?;
        }
        // connect new
        for object in objects {
            self.link_and_save_relation_object(relation, &object, path).await?;
        }
        Ok(())
    }

    async fn nested_set_many_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        // disconnect previous
        let records = self.fetch_relation_objects(relation.name(), None).await?;
        for record in records.iter() {
            self.nested_disconnect_relation_object_object(relation, record, path).await?;
        }
        // connect new
        let value_vec = value.as_array().unwrap();
        for value in value_vec {
            self.nested_connect_relation_object(relation, value, path).await?;
        }
        Ok(())
    }

    async fn nested_set_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        if !(relation.has_foreign_key() && relation.is_required()) {
            // disconnect old
            let disconnect_value = self.intrinsic_where_unique_for_relation(relation);
            let _ = self.nested_disconnect_relation_object_no_check(relation, &disconnect_value, path).await;
        }
        if !value.is_null() {
            // connect new
            let action = Action::from_u32(NESTED | SET | SINGLE);
            let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": value }), true, action, self.action_source().clone(), self.connection()).await {
                Ok(object) => object.into_not_found_error()?,
                Err(_) => return Err(Error::unexpected_input_value_with_reason("Object is not found.", path)),
            };
            self.link_and_save_relation_object(relation, &object, path).await?;
        }
        Ok(())
    }

    async fn nested_connect_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let action = Action::from_u32(NESTED | CONNECT | SINGLE);
        let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": value }), true, action, self.action_source().clone(), self.connection()).await {
            Ok(object) => object,
            Err(_) => return Err(Error::unexpected_input_value_with_reason("Object is not found.", path)),
        }.into_not_found_error()?;
        self.link_and_save_relation_object(relation, &object, path).await
    }

    async fn nested_connect_or_create_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let r#where = value.get("where").unwrap();
        let create = value.get("create").unwrap();
        let action = Action::from_u32(CONNECT_OR_CREATE | CONNECT | NESTED | SINGLE);
        let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
            Ok(object) => object.into_not_found_error()?,
            Err(_) => {
                self.graph().new_object_with_teon_and_path(relation.model(), create, &(path + "create"), action, self.action_source().clone(), self.connection()).await?
            },
        };
        self.link_and_save_relation_object(relation, &object, path).await
    }

    fn intrinsic_where_unique_for_relation(&self, relation: &'static Relation) -> Value {
        Value::HashMap(relation.iter().map(|(f, r)| (r.to_owned(), self.get_value(f).unwrap())).collect())
    }

    async fn nested_disconnect_relation_object_object(&self, relation: &'static Relation, object: &Object, path: &KeyPath) -> Result<()> {
        if !relation.is_vec && relation.is_required() {
            return Err(Error::unexpected_input_value_with_reason("Cannot disconnect required relation.", path));
        }
        if relation.has_foreign_key() {
            self.remove_linked_values_from_related_relation(relation);
        } else if relation.has_join_table() {
            self.delete_join_object(object, relation, self.graph().opposite_relation(relation).1.unwrap(), path).await?;
        } else {
            object.remove_linked_values_from_related_relation_on_related_object(relation, &object);
            object.save_with_session_and_path(path).await?;
        }
        Ok(())
    }

    async fn nested_disconnect_relation_object_no_check(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        if relation.has_foreign_key() {
            self.remove_linked_values_from_related_relation(relation);
        } else {
            let r#where = value;
            let action = Action::from_u32(NESTED | DISCONNECT | SINGLE);
            let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
                Ok(object) => object,
                Err(_) => return Err(Error::unexpected_input_value_with_reason("object is not found", path)),
            }.into_not_found_error()?;
            object.remove_linked_values_from_related_relation_on_related_object(relation, &object);
            object.save_with_session_and_path(path).await?;
        }
        Ok(())
    }

    async fn nested_disconnect_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        if !relation.is_vec && relation.is_required() {
            return Err(Error::unexpected_input_value_with_reason("Cannot disconnect required relation.", path));
        }
        self.nested_disconnect_relation_object_no_check(relation, value, path).await?;
        Ok(())
    }

    async fn nested_upsert_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let mut r#where = self.intrinsic_where_unique_for_relation(relation);
        r#where.as_dictionary_mut().unwrap().extend(value.get("where").unwrap().as_dictionary().cloned().unwrap());
        let create = value.get("create").unwrap();
        let update = value.get("update").unwrap();
        let action = Action::from_u32(NESTED | UPSERT | UPDATE | SINGLE);
        match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await.into_not_found_error() {
            Ok(object) => {
                let path = path + "update";
                object.set_teon_with_path(update, &path).await?;
                object.save_with_session_and_path(&path).await?;
            },
            Err(_) => {
                let action = Action::from_u32(NESTED | UPSERT | CREATE | SINGLE);
                let object = self.graph().new_object_with_teon_and_path(relation.model(), create, &(path + "create"), action, self.action_source().clone(), self.connection()).await?;
                self.link_and_save_relation_object(relation, &object, path).await?;
            },
        };
        Ok(())
    }

    async fn nested_many_disconnect_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        if relation.has_join_table() {
            let action = Action::from_u32(JOIN_DELETE | DELETE | SINGLE);
            let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": value }), true, action, self.action_source().clone(), self.connection()).await {
                Ok(object) => object.into_not_found_error()?,
                Err(_) => return Err(Error::unexpected_input_value_with_reason("Object is not found.", path)),
            };
            self.delete_join_object(&object, relation, self.graph().opposite_relation(relation).1.unwrap(), path).await?;
        } else {
            let mut r#where = self.intrinsic_where_unique_for_relation(relation);
            r#where.as_dictionary_mut().unwrap().extend(value.as_dictionary().cloned().unwrap().into_iter());
            let action = Action::from_u32(DISCONNECT | NESTED | SINGLE);
            let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
                Ok(object) => object.into_not_found_error()?,
                Err(_) => return Err(Error::unexpected_input_value_with_reason("Object is not found.", path)),
            };
            object.remove_linked_values_from_related_relation_on_related_object(relation, &object);
            object.save_with_session_and_path(path).await?;
        }
        Ok(())
    }

    async fn find_relation_objects_by_value(&self, relation: &'static Relation, value: &Value, path: &KeyPath, action: Action) -> Result<Vec<Object>> {
        if relation.has_join_table() {
            let mut finder = HashMap::new();
            let join_relation = self.graph().through_relation(relation).1;
            for (l, f) in join_relation.iter() {
                finder.insert(l.to_owned(), self.get_value(f).unwrap());
            }
            finder.insert(self.graph().through_opposite_relation(relation).1.name().to_owned(), teon!({
                "is": value
            }));
            if let Ok(join_objects) = self.graph().find_many_internal(relation.through().unwrap(), &teon!({
                "where": Value::HashMap(finder),
                "include": {
                    self.graph().through_opposite_relation(relation).1.name(): true
                }
            }), true, action, self.action_source().clone(), self.connection()).await {
                let mut results = vec![];
                for join_object in join_objects {
                    let object = join_object.get_query_relation_object(self.graph().through_opposite_relation(relation).1.name())?.unwrap();
                    results.push(object);
                }
                Ok(results)
            } else {
                return Err(Error::unexpected_input_value_with_reason("Object is not found.", &(path + "where")));
            }
        } else {
            let mut r#where = self.intrinsic_where_unique_for_relation(relation);
            r#where.as_dictionary_mut().unwrap().extend(value.as_dictionary().cloned().unwrap());
            let action = Action::from_u32(NESTED | UPDATE | MANY);
            let objects = self.graph().find_many_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await.unwrap();
            Ok(objects)
        }
    }

    async fn find_relation_object_by_value(&self, relation: &'static Relation, value: &Value, path: &KeyPath, action: Action) -> Result<Object> {
        if relation.has_join_table() {
            let mut finder = HashMap::new();
            let join_relation = self.graph().through_relation(relation).1;
            for (l, f) in join_relation.iter() {
                finder.insert(l.to_owned(), self.get_value(f).unwrap());
            }
            finder.insert(self.graph().through_opposite_relation(relation).1.name().to_owned(), teon!({
                "is": value
            }));
            if let Ok(join_object) = self.graph().find_first_internal(relation.through().unwrap(), &teon!({
                "where": Value::HashMap(finder),
                "include": {
                    self.graph().through_opposite_relation(relation).1.name(): true
                }
            }), true, action, self.action_source().clone(), self.connection()).await.into_not_found_error() {
                let object = join_object.get_query_relation_object(self.graph().through_opposite_relation(relation).1.name())?.unwrap();
                Ok(object)
            } else {
                return Err(Error::unexpected_input_value_with_reason("Object is not found.", &(path + "where")));
            }
        } else {
            let mut r#where = self.intrinsic_where_unique_for_relation(relation);
            r#where.as_dictionary_mut().unwrap().extend(value.as_dictionary().cloned().unwrap());
            let action = Action::from_u32(NESTED | UPDATE | SINGLE);
            let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
                Ok(object) => object,
                Err(_) => return Err(Error::unexpected_input_value_with_reason("Object is not found.", &(path + "where"))),
            }.into_not_found_error()?;
            Ok(object)
        }
    }

    async fn nested_many_update_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let object = self.find_relation_object_by_value(relation, value.get("where").unwrap(), path, Action::from_u32(NESTED | UPDATE | SINGLE)).await?;
        object.set_teon_with_path(value.get("update").unwrap(), &(path + "update")).await?;
        object.save_with_session_and_path(path).await?;
        Ok(())
    }

    async fn nested_many_update_many_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let objects = self.find_relation_objects_by_value(relation, value.get("where").unwrap(), path, Action::from_u32(NESTED | UPDATE | MANY)).await?;
        let update = value.get("update").unwrap();
        for object in objects {
            object.set_teon_with_path(update, path).await?;
            object.save_with_session_and_path(path).await?;
        }
        Ok(())
    }

    async fn nested_update_relation_object<'a>(&'a self, relation: &'static Relation, value: &'a Value, path: &'a KeyPath) -> Result<()> {
        let r#where = value.get("where").unwrap();
        let action = NESTED | UPDATE | SINGLE;
        let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
            Ok(object) => object.into_not_found_error()?,
            Err(_) => return Err(Error::unexpected_input_value_with_reason("update: object not found", path)),
        };
        object.set_teon_with_path(value.get("update").unwrap(), path).await?;
        object.save_with_session_and_path(path).await?;
        Ok(())
    }

    async fn nested_delete_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        if !relation.is_vec && relation.is_required() {
            return Err(Error::unexpected_input_value_with_reason("Cannot delete required relation.", path));
        }
        let r#where = value.get("where").unwrap();
        let action = Action::from_u32(NESTED | DELETE | SINGLE);
        let object = match self.graph().find_unique_internal(relation.model(), &teon!({ "where": r#where }), true, action, self.action_source().clone(), self.connection()).await {
            Ok(object) => object.into_not_found_error()?,
            Err(_) => return Err(Error::unexpected_input_value_with_reason("delete: object not found", path)),
        };
        object.delete_from_database().await?;
        if relation.has_join_table() {
            let opposite_relation = self.graph().opposite_relation(relation).1.unwrap();
            self.delete_join_object(&object, relation, opposite_relation, path).await?;
        }
        if relation.has_foreign_key() {
            self.remove_linked_values_from_related_relation(relation);
        }
        Ok(())
    }

    async fn nested_many_delete_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let object = self.find_relation_object_by_value(relation, value, path, Action::from_u32(NESTED | DELETE | SINGLE)).await?;
        object.delete_from_database().await?;
        if relation.has_join_table() {
            let opposite_relation = self.graph().opposite_relation(relation).1.unwrap();
            self.delete_join_object(&object, relation, opposite_relation, path).await?;
        }
        Ok(())
    }

    async fn nested_many_delete_many_relation_object(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        let objects = self.find_relation_objects_by_value(relation, value, path, Action::from_u32(NESTED | DELETE | MANY)).await?;
        for object in objects {
            object.delete_from_database().await?;
            if relation.has_join_table() {
                let opposite_relation = self.graph().opposite_relation(relation).1.unwrap();
                self.delete_join_object(&object, relation, opposite_relation, path).await?;
            }
        }
        Ok(())
    }

    async fn disconnect_object_which_connects_to<'a>(&'a self, relation: &'static Relation, value: &'a Value) -> Result<()> {
        if let Ok(that) = self.graph().find_unique::<Object>(self.model(), &teon!({
            "where": {
                relation.name(): {
                    "is": value
                }
            }
        }), self.connection(), self.initiator().as_req()).await.into_not_found_error() {
            if relation.is_required() {
                return Err(Error::cannot_disconnect_previous_relation());
            } else {
                for (l, _f) in relation.iter() {
                    that.set_value(l, Value::Null).unwrap();
                }
                that.save().await.unwrap();
            }
        }
        Ok(())
    }

    async fn perform_relation_manipulation_one_inner(&self, relation: &'static Relation, action: Action, value: &Value, path: &KeyPath) -> Result<()> {
        let action_u32 = action.to_u32();
        if !relation.is_vec && !relation.has_foreign_key() && !self.is_new() {
            match action {
                NESTED_CREATE_ACTION | NESTED_CONNECT_ACTION | NESTED_CONNECT_OR_CREATE_ACTION => {
                    let disconnect_value = self.intrinsic_where_unique_for_relation(relation);
                    let _ = self.nested_disconnect_relation_object_no_check(relation, &disconnect_value, path).await;
                },
                _ => ()
            }
        }
        if !relation.is_vec && relation.has_foreign_key() {
            if let Some(opposite_relation) = self.graph().opposite_relation(relation).1 {
                if !opposite_relation.is_vec {
                    match action {
                        NESTED_CONNECT_ACTION | NESTED_SET_ACTION => {
                            if !value.is_null() {
                                self.disconnect_object_which_connects_to(relation, value).await?;
                            }
                        }
                        NESTED_CONNECT_OR_CREATE_ACTION => {
                            self.disconnect_object_which_connects_to(relation, value.get("where").unwrap()).await?;
                        }
                        _ => ()
                    }
                }
            }
        }
        match action_u32 {
            NESTED_CREATE_ACTION => self.nested_create_relation_object(relation, value, &path).await,
            NESTED_CONNECT_ACTION => self.nested_connect_relation_object(relation, value, &path).await,
            NESTED_SET_ACTION => self.nested_set_relation_object(relation, value, &path).await,
            NESTED_CONNECT_OR_CREATE_ACTION => self.nested_connect_or_create_relation_object(relation, value, &path).await,
            NESTED_DISCONNECT_ACTION => self.nested_disconnect_relation_object(relation, value, &path).await,
            NESTED_UPDATE_ACTION => self.nested_update_relation_object(relation, value, &path).await,
            NESTED_DELETE_ACTION => self.nested_delete_relation_object(relation, value, &path).await,
            NESTED_UPSERT_ACTION => self.nested_upsert_relation_object(relation, value, &path).await,
            _ => unreachable!(),
        }
    }

    fn normalize_relation_one_value<'a>(&'a self, relation: &'static Relation, action: Action, value: &'a Value) -> Cow<Value> {
        match action.to_u32() {
            NESTED_CREATE_ACTION => Owned(Value::HashMap(hashmap! {"create".to_owned() => value.clone()})),
            NESTED_UPDATE_ACTION => Owned(Value::HashMap(hashmap! {"update".to_owned() => value.clone(), "where".to_owned() => self.intrinsic_where_unique_for_relation(relation)})),
            NESTED_DELETE_ACTION => Owned(Value::HashMap(hashmap! {"where".to_owned() => self.intrinsic_where_unique_for_relation(relation)})),
            NESTED_DISCONNECT_ACTION => Owned(self.intrinsic_where_unique_for_relation(relation)),
            NESTED_UPSERT_ACTION => {
                let mut value = value.clone();
                value.as_dictionary_mut().unwrap().insert("where".to_owned(), self.intrinsic_where_unique_for_relation(relation));
                Owned(value)
            }
            _ => Borrowed(value)
        }
    }

    async fn perform_relation_manipulation_one(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        for (key, value) in value.as_dictionary().unwrap() {
            let key = key.as_str();
            let path = path + key;
            let action = Action::nested_action_from_name(key).unwrap();
            let other_model = self.graph().opposite_relation(relation).0;
            let normalized_value = self.normalize_relation_one_value(relation, action, value);
            let ctx = PipelineCtx::initial_state_with_value(normalized_value.as_ref().clone(), self.connection(), self.initiator().as_req()).with_path(path.clone()).with_action(action);
            let (transformed_value, new_action) = other_model.transformed_action(ctx).await?;
            self.perform_relation_manipulation_one_inner(relation, new_action, &transformed_value, &path).await?;
        }
        Ok(())
    }

    fn normalize_relation_many_value<'a>(&'a self, action: Action, value: &'a Value) -> Cow<Value> {
        match action.to_u32() {
            NESTED_CREATE_ACTION => Owned(Value::HashMap(hashmap! {"create".to_owned() => value.clone()})),
            _ => Borrowed(value)
        }
    }

    async fn perform_relation_manipulation_many_inner(&self, relation: &'static Relation, action: Action, value: &Value, path: &KeyPath) -> Result<()> {
        match action.to_u32() {
            NESTED_CREATE_ACTION => self.nested_create_relation_object(relation, value, &path).await,
            NESTED_CONNECT_ACTION => self.nested_connect_relation_object(relation, value, &path).await,
            NESTED_SET_ACTION => self.nested_set_many_relation_object(relation, value, &path).await,
            NESTED_CONNECT_OR_CREATE_ACTION => self.nested_connect_or_create_relation_object(relation, value, &path).await,
            NESTED_DISCONNECT_ACTION => self.nested_many_disconnect_relation_object(relation, value, &path).await,
            NESTED_UPSERT_ACTION => self.nested_upsert_relation_object(relation, value, &path).await,
            NESTED_UPDATE_ACTION => self.nested_many_update_relation_object(relation, value, &path).await,
            NESTED_UPDATE_MANY_ACTION => self.nested_many_update_many_relation_object(relation, value, &path).await,
            NESTED_DELETE_ACTION => self.nested_many_delete_relation_object(relation, value, &path).await,
            NESTED_DELETE_MANY_ACTION => self.nested_many_delete_many_relation_object(relation, value, &path).await,
            _ => unreachable!(),
        }
    }

    async fn perform_relation_manipulation_many(&self, relation: &'static Relation, value: &Value, path: &KeyPath) -> Result<()> {
        for (key, value) in value.as_dictionary().unwrap() {
            let key = key.as_str();
            let path = path + key;
            let action = Action::nested_action_from_name(key).unwrap();
            let other_model = self.graph().opposite_relation(relation).0;
            if value.is_vec() && action.to_u32() != NESTED_SET_ACTION {
                for (index, value) in value.as_array().unwrap().iter().enumerate() {
                    let normalized_value = self.normalize_relation_many_value(action, value);
                    let ctx = PipelineCtx::initial_state_with_value(normalized_value.as_ref().clone(), self.connection(), self.initiator().as_req()).with_path(&(path.clone() + index)).with_action(action);
                    let (transformed_value, new_action) = other_model.transformed_action(ctx).await?;
                    self.perform_relation_manipulation_many_inner(relation, new_action, &transformed_value, &path).await?;
                }
            }  else {
                let normalized_value = self.normalize_relation_many_value(action, value);
                let ctx = PipelineCtx::initial_state_with_value(normalized_value.as_ref().clone(), self.connection(), self.initiator().as_req()).with_path(path.clone()).with_action(action);
                let (transformed_value, new_action) = other_model.transformed_action(ctx).await?;
                self.perform_relation_manipulation_many_inner(relation, new_action, &transformed_value, &path).await?;
            }
        }
        Ok(())
    }

    pub async fn refreshed(&self, include: Option<&Value>, select: Option<&Value>) -> Result<Object> {
        if self.model().is_virtual() {
            self.set_select(select).unwrap();
            return Ok(self.clone())
        }
        let graph = self.graph();
        let mut finder = teon!({
            "where": self.identifier(),
        });
        if let Some(include) = include {
            finder.as_dictionary_mut().unwrap().insert("include".to_string(), include.clone());
        }
        if let Some(select) = select {
            finder.as_dictionary_mut().unwrap().insert("select".to_string(), select.clone());
        }
        let target = graph.find_unique_internal(self.model(), &finder, false, self.action(), self.action_source().clone(), self.connection()).await.into_not_found_error();
        match target {
            Ok(obj) => {
                if self.model().has_virtual_fields() {
                    self.copy_virtual_fields(&obj);
                }
                Ok(obj)
            }
            Err(err) => Err(err)
        }
    }

    fn copy_virtual_fields(&self, other: &Object) {
        for field in self.model().fields() {
            if field.r#virtual {
                let result = self.get_value(field.name()).unwrap();
                other.set(field.name(), result).unwrap();
            }
        }
    }

    pub async fn force_set_relation_objects(&self, key: &str, objects: Vec<Object>) -> () {
        self.inner.object_set_many_map.lock().await.insert(key.to_owned(), objects);
    }

    pub async fn force_add_relation_objects(&self, key: &str, objects: Vec<Object>) -> () {
        self.inner.object_connect_map.lock().await.insert(key.to_owned(), objects);
    }

    pub async fn force_remove_relation_objects(&self, key: &str, objects: Vec<Object>) -> () {
        self.inner.object_disconnect_map.lock().await.insert(key.to_owned(), objects);
    }

    pub async fn force_get_relation_objects(&self, key: &str, find_many_args: impl AsRef<Value>) -> Result<Vec<Object>> {
        self.fetch_relation_objects(key, Some(find_many_args.as_ref())).await
    }

    pub async fn force_set_relation_object(&self, key: &str, object: Option<Object>) -> () {
        self.inner.object_set_map.lock().await.insert(key.to_owned(), object);
    }

    pub async fn force_get_relation_object(&self, key: &str) -> Result<Option<Object>> {
        if self.has_mutation_relation_fetched(key) {
            self.get_mutation_relation_object(key)
        } else {
            self.fetch_relation_object(key, None).await
        }
    }

    pub async fn fetch_relation_object(&self, key: impl AsRef<str>, find_unique_arg: Option<&Value>) -> Result<Option<Object>> {
        // get relation
        let model = self.model();
        let relation = model.relation(key.as_ref());
        if relation.is_none() {
            // todo() err here
        }
        let relation = relation.unwrap();
        let mut finder = self.intrinsic_where_unique_for_relation(relation);
        if let Some(find_unique_arg) = find_unique_arg {
            if let Some(include) = find_unique_arg.get("include") {
                finder.as_dictionary_mut().unwrap().insert("include".to_owned(), include.clone());
            }
            if let Some(select) = find_unique_arg.get("select") {
                finder.as_dictionary_mut().unwrap().insert("select".to_owned(), select.clone());
            }
        }
        let relation_model_name = relation.model();
        let graph = self.graph();
        let action = Action::from_u32(NESTED | FIND | PROGRAM_CODE | SINGLE);
        match graph.find_unique_internal(relation_model_name, &finder, false, action, Initiator::ProgramCode(self.initiator().as_req()), self.connection()).await {
            Ok(result) => {
                self.inner.relation_query_map.lock().unwrap().insert(key.as_ref().to_string(), vec![result.into_not_found_error()?]);
                let obj = self.inner.relation_query_map.lock().unwrap().get(key.as_ref()).unwrap().get(0).unwrap().clone();
                Ok(Some(obj.clone()))
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    pub async fn fetch_relation_objects(&self, key: impl AsRef<str>, find_many_arg: Option<&Value>) -> Result<Vec<Object>> {
        // get relation
        let model = self.model();
        let relation = model.relation(key.as_ref());
        if relation.is_none() {
            // todo() err here
        }
        let relation = relation.unwrap();
        let empty = teon!({});
        let include_inside = if find_many_arg.is_some() {
            find_many_arg.unwrap()
        } else {
            &empty
        };
        let action = Action::from_u32(INTERNAL_POSITION | FIND | PROGRAM_CODE | MANY);
        if relation.has_join_table() {
            let identifier = self.identifier();
            let new_self = self.graph().find_unique_internal(model, &teon!({
                "where": identifier,
                "include": {
                    key.as_ref(): include_inside
                }
            }), false, action, Initiator::ProgramCode(self.initiator().as_req()), self.connection()).await.into_not_found_error()?;
            let vec = new_self.inner.relation_query_map.lock().unwrap().get(key.as_ref()).unwrap().clone();
            Ok(vec)
        } else {
            let mut finder = teon!({});
            if let Some(find_many_arg) = find_many_arg {
                for (k, v) in find_many_arg.as_dictionary().unwrap().iter() {
                    finder.as_dictionary_mut().unwrap().insert(k.clone(), v.clone());
                }
            }
            if finder.as_dictionary().unwrap().get("where").is_none() {
                finder.as_dictionary_mut().unwrap().insert("where".to_string(), teon!({}));
            }
            for (index, local_field_name) in relation.fields().iter().enumerate() {
                let foreign_field_name = relation.references().get(index).unwrap();
                let value = self.get_value(local_field_name).unwrap();
                if value == Value::Null {
                    return Ok(vec![]);
                }
                let json_value = value;
                finder.as_dictionary_mut().unwrap().get_mut("where").unwrap().as_dictionary_mut().unwrap().insert(foreign_field_name.to_owned(), json_value);
            }
            let relation_model = AppCtx::get().unwrap().model(relation.model_path()).unwrap().unwrap();
            let results = self.graph().find_many_internal(relation_model, &finder, false, action, Initiator::ProgramCode(self.initiator().as_req()), self.connection()).await?;
            Ok(results)
        }
    }

    pub(crate) fn keys_for_save(&self) -> Vec<&str> {
        if self.is_new() {
            self.model().save_keys().iter().map(|k| *k).collect()
        } else {
            self.model().save_keys().iter().filter(|k| {
                self.inner.modified_fields.lock().unwrap().contains(&k.to_string()) ||
                    self.inner.atomic_updater_map.lock().unwrap().contains_key(&k.to_string())
            }).map(|k| *k).collect()
        }
    }

    pub(crate) fn action(&self) -> Action {
        self.inner.action
    }

    pub(crate) fn ignore_relation(&self, name: &str) {
        self.inner.as_ref().to_mut().ignore_relation = Some(name.to_owned());
    }
}

#[derive(Debug)]
struct ObjectInner {
    request_ctx: Option<request::Ctx>,
    transaction_ctx: transaction::Ctx,
    model: &'static Model,
    action: Action,
    is_initialized: AtomicBool,
    is_new: AtomicBool,
    is_modified: AtomicBool,
    is_partial: AtomicBool,
    is_deleted: AtomicBool,
    inside_before_save_callback: AtomicBool,
    inside_after_save_callback: AtomicBool,
    selected_fields: Arc<Mutex<Vec<String>>>,
    modified_fields: Arc<Mutex<BTreeSet<String>>>,
    value_map: Arc<Mutex<BTreeMap<String, Value>>>,
    previous_value_map: Arc<Mutex<BTreeMap<String, Value>>>,
    atomic_updater_map: Arc<Mutex<BTreeMap<String, Value>>>,
    relation_mutation_map: Arc<TokioMutex<BTreeMap<String, Value>>>,
    relation_query_map: Arc<Mutex<BTreeMap<String, Vec<Object>>>>,
    cached_property_map: Arc<Mutex<BTreeMap<String, Value>>>,
    object_set_map: Arc<TokioMutex<BTreeMap<String, Option<Object>>>>,
    object_set_many_map: Arc<TokioMutex<BTreeMap<String, Vec<Object>>>>,
    object_connect_map: Arc<TokioMutex<BTreeMap<String, Vec<Object>>>>,
    object_disconnect_map: Arc<TokioMutex<BTreeMap<String, Vec<Object>>>>,
    ignore_relation: Option<String>,
}

impl Serialize for Object {

    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_none()
    }
}

fn check_user_json_keys<'a>(map: &IndexMap<String, Value>, allowed: &HashSet<&str>, model: &'static Model) -> Result<()> {
    if let Some(unallowed) = map.keys().find(|k| !allowed.contains(k.as_str())) {
        return Err(Error::new(format!("key '{}' is not allowed for {}", unallowed, model.name())));
    }
    Ok(())
}