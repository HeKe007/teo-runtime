use key_path::path;
use teo_teon::Value;
use crate::request;
use crate::response::Response;

pub async fn count(req_ctx: &request::Ctx) -> crate::path::Result<Response> {
    let model = req_ctx.namespace().model_at_path(&req_ctx.handler_match().path()).unwrap();
    let result = req_ctx.transaction_ctx().count(model, req_ctx.body(), path![]).await?;
    Ok(Response::data(Value::Int64(result as i64))?)
}