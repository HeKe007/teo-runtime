use regex::Regex;
use crate::arguments::Arguments;
use crate::namespace::Namespace;
use crate::pipeline::Ctx;
use once_cell::sync::Lazy;
use teo_result::Error;
use teo_result::ResultExt;
use crate::value::Value;

pub(super) static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b$").unwrap()
});

pub(super) static HEX_COLOR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Fa-f0-9]{6}$").unwrap()
});

pub(super) static SECURE_PASSWORD_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r#"[A-Z]"#).unwrap(),
        Regex::new(r#"[a-z]"#).unwrap(),
        Regex::new(r#"\d"#).unwrap(),
        Regex::new(r#"[!@#$&*`~()\-_+=\[\]{}:;'",<>.?\\|/]"#).unwrap(),
    ]
});

pub(in crate::stdlib) fn load_pipeline_string_validation_items(namespace: &mut Namespace) {

    namespace.define_pipeline_item("isEmail", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isEmail")?;
        if !EMAIL_REGEX.is_match(input) {
            Err(Error::new("input is not email"))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isHexColor", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isHexColor")?;
        if !HEX_COLOR_REGEX.is_match(input) {
            Err(Error::new("input is not hex color"))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isSecurePassword", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isSecurePassword")?;
        for regex in SECURE_PASSWORD_REGEXES.iter() {
            if !regex.is_match(input) {
                Err(Error::new("input is not secure password"))?
            }
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isNumeric", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isNumeric")?;
        for c in input.chars(){
            if !c.is_numeric(){
                Err(Error::new("input is not numeric"))?
            }
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isAlphanumeric", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isAlphanumeric")?;
        for c in input.chars(){
            if !c.is_alphanumeric(){
                Err(Error::new("input is not alphanumeric"))?
            }
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isAlphabetic", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isAlphabetic")?;
        for c in input.chars(){
            if !c.is_alphabetic(){
                Err(Error::new("input is not alphabetic"))?
            }
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isSuffixOf", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isSuffixOf")?;
        let arg_object: Value = ctx.resolve_pipeline_with_err_prefix(
            args.get_object("value").error_message_prefixed("isSuffixOf")?,
            "isSuffixOf",
        ).await?;
        let arg: &str = arg_object.try_ref_into_err_prefix("isSuffixOf")?;
        if !arg.ends_with(input) {
            Err(Error::new(format!("input is not suffix of \"{arg}\"")))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("hasSuffix", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("hasSuffix")?;
        let arg_object: Value = ctx.resolve_pipeline_with_err_prefix(
            args.get_object("value").error_message_prefixed("hasSuffix")?,
            "hasSuffix",
        ).await?;
        let arg: &str = arg_object.try_ref_into_err_prefix("hasSuffix")?;
        if !input.ends_with(arg) {
            Err(Error::new(format!("input doesn't have suffix \"{arg}\"")))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("isPrefixOf", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("isPrefixOf")?;
        let arg_object: Value = ctx.resolve_pipeline_with_err_prefix(
            args.get_object("value").error_message_prefixed("isPrefixOf")?,
            "isPrefixOf",
        ).await?;
        let arg: &str = arg_object.try_ref_into_err_prefix("isPrefixOf")?;
        if !arg.starts_with(input) {
            Err(Error::new(format!("input is not prefix of \"{arg}\"")))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("hasPrefix", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("hasPrefix")?;
        let arg_object: Value = ctx.resolve_pipeline_with_err_prefix(
            args.get_object("value").error_message_prefixed("hasPrefix")?,
            "hasPrefix",
        ).await?;
        let arg: &str = arg_object.try_ref_into_err_prefix("hasPrefix")?;
        if !input.starts_with(arg) {
            Err(Error::new(format!("input doesn't have suffix \"{arg}\"")))?
        }
        Ok(ctx.value().clone())
    });

    namespace.define_pipeline_item("regexMatch", |args: Arguments, ctx: Ctx| async move {
        let input: &str = ctx.value().try_ref_into_err_prefix("regexMatch")?;
        let regex: &Regex = args.get("regex").error_message_prefixed("regexMatch")?;
        if !regex.is_match(input){
            Err(Error::new(format!("input doesn't match regex")))?
        }
        Ok(ctx.value().clone())
    });
}