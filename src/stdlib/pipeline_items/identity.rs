use async_trait::async_trait;
use crate::core::initiator::Initiator;
use crate::core::item::Item;
use crate::value::value::Value;
use crate::core::pipeline::ctx::PipelineCtx;
use crate::core::pipeline::Pipeline;
use crate::core::result::Result;

#[derive(Debug, Clone)]
pub struct IdentityItem {
    pipeline: Pipeline
}

impl IdentityItem {
    pub fn new(pipeline: Pipeline) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl Item for IdentityItem {
    async fn call<'a>(&self, ctx: PipelineCtx<'a>) -> Result<PipelineCtx<'a>> {
        match ctx.get_object()?.action_source() {
            Initiator::Identity(user, _) => {
                let user = match user {
                    Some(u) => Value::Object(u.clone()),
                    None => Value::Null,
                };
                Ok(ctx.with_value(self.pipeline.process(ctx.with_value(user)).await?))
            }
            _ => Ok(ctx)
        }
    }
}
use async_trait::async_trait;
use crate::core::pipeline::item::Item;
use crate::core::result::Result;
use crate::core::pipeline::ctx::Ctx;

#[derive(Debug, Copy, Clone)]
pub struct ConnectIdentityItem {}

impl ConnectIdentityItem {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Item for ConnectIdentityItem {
    async fn call<'a>(&self, ctx: Ctx<'a>) -> Result<Ctx<'a>> {

        if let Some(identity) = ctx.object.as_ref().unwrap().env().trigger().as_identity() {
            let model = ctx.object.as_ref().unwrap().model();
            let relation_name = ctx.key_path[0].as_key().unwrap();
            let relation = model.relation(relation_name).unwrap();
            let relation_model_name = relation.model();
            let identity_model_name = identity.model().name();
            if relation_model_name != identity_model_name {
                return ctx;
            }
            // here set
            // ctx.object.link_connect(&identity, relation, )
            // let mut map = ctx.object.inner.relation_connection_map.lock().unwrap();
            // let connections = map.get(relation_name);
            // if connections.is_none() {
            //     map.insert(relation_name.to_string(), Vec::new());
            //     map.get_mut(relation_name).unwrap().push(RelationConnection::Link(identity.clone()));
            // }
            ctx.clone()
        } else {
            ctx
        }
    }
}

