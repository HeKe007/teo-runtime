use num_integer::Roots;
use std::ops::Add;
use bigdecimal::num_traits::{Pow};
use bigdecimal::BigDecimal;
use teo_teon::Value;
use crate::arguments::Arguments;
use teo_result::Error;
use crate::namespace::Namespace;
use crate::object::Object;
use crate::pipeline::Ctx;
use teo_result::{Result, ResultExt};

pub(in crate::stdlib) fn load_pipeline_math_items(namespace: &mut Namespace) {

    namespace.define_pipeline_item("add", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("add")?;
        let arg_object = &ctx.resolve_pipeline(
            args.get_object("value").err_prefix("add(value)")?,
            "add(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("add(value)")?;
        Ok(Object::from((input + arg).err_prefix("add")?))
    });

    namespace.define_pipeline_item("sub", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("sub")?;
        let arg_object = &ctx.resolve_pipeline(
            args.get_object("value").err_prefix("sub(value)")?,
            "sub(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("sub(value)")?;
        Ok(Object::from((input - arg).err_prefix("sub")?))
    });

    namespace.define_pipeline_item("mul", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("mul")?;
        let arg_object = &ctx.resolve_pipeline(
            args.get_object("value").err_prefix("mul(value)")?,
            "mul(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("mul(value)")?;
        Ok(Object::from((input * arg).err_prefix("mul")?))
    });

    namespace.define_pipeline_item("div", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("div")?;
        let arg_object = &ctx.resolve_pipeline(
            args.get_object("value").err_prefix("div(value)")?,
            "div(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("div(value)")?;
        Ok(Object::from((input / arg).err_prefix("div")?))
    });

    namespace.define_pipeline_item("mod", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("mod")?;
        let arg_object = &ctx.resolve_pipeline(
            args.get_object("value").err_prefix("mod(value)")?,
            "mod(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("mod(value)")?;
        Ok(Object::from((input % arg).err_prefix("mod")?))
    });

    namespace.define_pipeline_item("max", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("max")?;
        let arg_object = ctx.resolve_pipeline(
            args.get_object("value").err_prefix("max(value)")?,
            "max(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("max(value)")?;
        Ok(if input > arg {
            arg_object
        } else {
            ctx.value().clone()
        })
    });

    namespace.define_pipeline_item("min", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("min")?;
        let arg_object = ctx.resolve_pipeline(
            args.get_object("value").err_prefix("min(value)")?,
            "min(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("min(value)")?;
        Ok(if input < arg {
            arg_object
        } else {
            ctx.value().clone()
        })
    });

    namespace.define_pipeline_item("floor", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("floor")?;
        Ok(match input {
            Value::Float32(f) => Object::from(f.floor()),
            Value::Float(f) => Object::from(f.floor()),
            Value::Decimal(d) => Object::from(d.with_scale(0)),
            _ => Err(Error::new("floor: invalid input"))?
        })
    });

    namespace.define_pipeline_item("ceil", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("ceil")?;
        Ok(match input {
            Value::Float32(f) => Object::from(f.ceil()),
            Value::Float(f) => Object::from(f.ceil()),
            Value::Decimal(d) => Object::from(if d.digits() == 0 {
                d.clone()
            } else {
                d.with_scale(0).add(BigDecimal::from(1))
            }),
            _ => Err(Error::new("ceil: invalid input"))?
        })
    });

    namespace.define_pipeline_item("round", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("round")?;
        Ok(match input {
            Value::Float32(f) => Object::from(f.round()),
            Value::Float(f) => Object::from(f.round()),
            Value::Decimal(d) => Object::from(d.round(0)),
            _ => Err(Error::new("round: invalid input"))?
        })
    });

    namespace.define_pipeline_item("abs", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("abs")?;
        Ok(match input {
            Value::Int(i) => Object::from(i.abs()) ,
            Value::Int64(i) => Object::from(i.abs()) ,
            Value::Float32(f) => Object::from(f.abs()),
            Value::Float(f) => Object::from(f.abs()),
            Value::Decimal(d) => Object::from(d.abs()),
            _ => Err(Error::new("abs: invalid input"))?
        })
    });

    namespace.define_pipeline_item("sqrt", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("sqrt")?;
        Ok(match input {
            Value::Int(i)   => Object::from(i.sqrt()),
            Value::Int64(i) => Object::from(i.sqrt()),
            Value::Float32(f) => Object::from(f.sqrt()),
            Value::Float(f) => Object::from(f.sqrt()),
            Value::Decimal(d) => Object::from(if let Some(d) = d.sqrt() {
                d
            } else {
                Err(Error::new(format!("sqrt: decimal value '{d}' is invalid")))?
            }),
            _ => Err(Error::new("sqrt: invalid input"))?
        })
    });

    namespace.define_pipeline_item("cbrt", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("cbrt")?;
        Ok(match input {
            Value::Int(i)   => Object::from((*i as f64).cbrt() as i32),
            Value::Int64(i) => Object::from((*i as f64).cbrt() as i64),
            Value::Float32(f) => Object::from(f.cbrt()),
            Value::Float(f) => Object::from(f.cbrt()),
            Value::Decimal(d) => Object::from(d.cbrt()),
            _ => Err(Error::new("cbrt: invalid input"))?
        })
    });

    namespace.define_pipeline_item("pow", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("pow")?;
        let arg_object = ctx.resolve_pipeline(
            args.get_object("value").err_prefix("pow(value)")?,
            "pow(value)",
        ).await?;
        let arg: &Value = arg_object.try_into_err_prefix("pow(value)")?;
        if input.is_any_int() {
            if !arg.is_any_int() {
                return Err(Error::new("pow(value): value is not integer"));
            }
        } else if input.is_any_float() {
            if !arg.is_any_int_or_float() {
                return Err(Error::new("pow(value): value is not int or float"));
            }
        }
        Ok(match input {
            Value::Int(i) => Object::from(i.pow(arg.to_int().unwrap() as u32)),
            Value::Int64(i)   => Object::from(i.pow(arg.to_int().unwrap() as u32)),
            Value::Float32(f) => Object::from(f.powf(arg.to_float().unwrap() as f32)),
            Value::Float(f)   => Object::from(f.powf(arg.to_float().unwrap())),
            _ => Err(Error::new("pow: invalid input"))?
        })
    });

    namespace.define_pipeline_item("root", |args: Arguments, ctx: Ctx| async move {
        let input: &Value = ctx.value().try_into_err_prefix("root")?;
        let arg_object = ctx.resolve_pipeline(
            args.get_object("value").err_prefix("root(value)")?,
            "root(value)",
        ).await?;
        let arg: i32 = arg_object.try_into_err_prefix("root(value)")?;
        Ok( match input {
            Value::Int(i)     => Object::from(i.nth_root(arg as u32)),
            Value::Int64(i)   => Object::from(i.nth_root(arg as u32)),
            _ => Err(Error::new("root: invalid input"))?
        })
    });


}
