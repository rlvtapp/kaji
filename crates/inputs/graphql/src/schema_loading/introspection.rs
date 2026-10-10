use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::fmt::Write;

pub(super) fn to_sdl(document: &Value) -> Result<String> {
    if let Some(errors) = document.get("errors") {
        ensure!(
            errors.as_array().is_some_and(|e| e.is_empty()),
            "introspection response contains errors"
        );
    }
    let schema = document
        .get("__schema")
        .or_else(|| document.get("data").and_then(|d| d.get("__schema")))
        .context("introspection JSON requires __schema or data.__schema")?;
    let mut out = String::new();
    description(schema, &mut out)?;
    out.push_str("schema {\n");
    for (field, kind) in [
        ("queryType", "query"),
        ("mutationType", "mutation"),
        ("subscriptionType", "subscription"),
    ] {
        if let Some(root) = schema.get(field).filter(|v| !v.is_null()) {
            writeln!(out, "  {kind}: {}", name(root)?)?;
        } else {
            ensure!(kind != "query", "introspection schema requires queryType");
        }
    }
    out.push_str("}\n");
    for ty in array(schema, "types")? {
        let n = name(ty)?;
        if n.starts_with("__") || ["String", "ID", "Int", "Float", "Boolean"].contains(&n) {
            continue;
        }
        description(ty, &mut out)?;
        match text(ty, "kind")? {
            "SCALAR" => {
                write!(out, "scalar {n}")?;
                if let Some(url) = ty.get("specifiedByURL").and_then(Value::as_str) {
                    write!(out, " @specifiedBy(url: {})", serde_json::to_string(url)?)?;
                }
                out.push('\n');
            }
            kind @ ("OBJECT" | "INTERFACE") => {
                write!(
                    out,
                    "{} {n}",
                    if kind == "OBJECT" {
                        "type"
                    } else {
                        "interface"
                    }
                )?;
                if let Some(interfaces) = ty
                    .get("interfaces")
                    .and_then(Value::as_array)
                    .filter(|a| !a.is_empty())
                {
                    out.push_str(" implements ");
                    for (i, interface) in interfaces.iter().enumerate() {
                        if i > 0 {
                            out.push_str(" & ");
                        }
                        out.push_str(name(interface)?);
                    }
                }
                out.push_str(" {\n");
                for field in array(ty, "fields")? {
                    description(field, &mut out)?;
                    write!(out, "  {}", name(field)?)?;
                    arguments(field, &mut out)?;
                    write!(out, ": {}", type_ref(required(field, "type")?, 0)?)?;
                    deprecation(field, &mut out)?;
                    out.push('\n');
                }
                out.push_str("}\n");
            }
            "INPUT_OBJECT" => {
                write!(out, "input {n}")?;
                if ty.get("isOneOf").and_then(Value::as_bool) == Some(true) {
                    out.push_str(" @oneOf");
                }
                out.push_str(" {\n");
                for field in array(ty, "inputFields")? {
                    description(field, &mut out)?;
                    write!(out, "  ")?;
                    input_field(field, &mut out)?;
                    out.push('\n');
                }
                out.push_str("}\n");
            }
            "ENUM" => {
                writeln!(out, "enum {n} {{")?;
                for value in array(ty, "enumValues")? {
                    description(value, &mut out)?;
                    write!(out, "  {}", name(value)?)?;
                    deprecation(value, &mut out)?;
                    out.push('\n');
                }
                out.push_str("}\n");
            }
            "UNION" => {
                write!(out, "union {n} = ")?;
                for (i, possible) in array(ty, "possibleTypes")?.iter().enumerate() {
                    if i > 0 {
                        out.push_str(" | ");
                    }
                    out.push_str(name(possible)?);
                }
                out.push('\n');
            }
            kind => bail!("unknown introspection type kind {kind:?}"),
        }
    }
    for directive in array(schema, "directives")? {
        let n = name(directive)?;
        if ["skip", "include", "deprecated", "specifiedBy", "oneOf"].contains(&n) {
            continue;
        }
        description(directive, &mut out)?;
        write!(out, "directive @{n}")?;
        arguments(directive, &mut out)?;
        if directive.get("isRepeatable").and_then(Value::as_bool) == Some(true) {
            out.push_str(" repeatable");
        }
        out.push_str(" on ");
        for (i, location) in array(directive, "locations")?.iter().enumerate() {
            if i > 0 {
                out.push_str(" | ");
            }
            let location = location.as_str().context("invalid directive location")?;
            identifier(location)?;
            out.push_str(location);
        }
        out.push('\n');
    }
    Ok(out)
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    v.get(key)
        .filter(|v| !v.is_null())
        .with_context(|| format!("introspection field {key} is missing"))
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    required(v, key)?
        .as_str()
        .with_context(|| format!("introspection field {key} must be a string"))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    required(v, key)?
        .as_array()
        .with_context(|| format!("introspection field {key} must be an array"))
}
fn identifier(n: &str) -> Result<()> {
    ensure!(
        !n.is_empty()
            && n.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid introspection name {n:?}"
    );
    Ok(())
}
fn name(v: &Value) -> Result<&str> {
    let n = text(v, "name")?;
    identifier(n)?;
    Ok(n)
}
fn description(v: &Value, out: &mut String) -> Result<()> {
    if let Some(d) = v.get("description").filter(|v| !v.is_null()) {
        writeln!(
            out,
            "{}",
            serde_json::to_string(d.as_str().context("description must be a string")?)?
        )?;
    }
    Ok(())
}
fn deprecation(v: &Value, out: &mut String) -> Result<()> {
    if v.get("isDeprecated").and_then(Value::as_bool) == Some(true) {
        out.push_str(" @deprecated");
        if let Some(reason) = v.get("deprecationReason").and_then(Value::as_str) {
            write!(out, "(reason: {})", serde_json::to_string(reason)?)?;
        }
    }
    Ok(())
}
fn input_field(v: &Value, out: &mut String) -> Result<()> {
    write!(out, "{}: {}", name(v)?, type_ref(required(v, "type")?, 0)?)?;
    if let Some(default) = v.get("defaultValue").filter(|v| !v.is_null()) {
        write!(
            out,
            " = {}",
            constant_literal(
                default
                    .as_str()
                    .context("introspection defaultValue must be a GraphQL literal string")?
            )?
        )?;
    }
    deprecation(v, out)
}
fn arguments(v: &Value, out: &mut String) -> Result<()> {
    let args = array(v, "args")?;
    if !args.is_empty() {
        out.push('(');
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            description(arg, out)?;
            input_field(arg, out)?;
        }
        out.push(')');
    }
    Ok(())
}
fn type_ref(v: &Value, depth: usize) -> Result<String> {
    ensure!(depth < 128, "introspection type nesting exceeds128");
    match text(v, "kind")? {
        "NON_NULL" => {
            let inner = required(v, "ofType")?;
            ensure!(
                text(inner, "kind")? != "NON_NULL",
                "nested NON_NULL introspection type"
            );
            Ok(format!("{}!", type_ref(inner, depth + 1)?))
        }
        "LIST" => Ok(format!(
            "[{}]",
            type_ref(required(v, "ofType")?, depth + 1)?
        )),
        "SCALAR" | "OBJECT" | "INTERFACE" | "UNION" | "ENUM" | "INPUT_OBJECT" => {
            Ok(name(v)?.into())
        }
        kind => bail!("invalid introspection type reference kind {kind}"),
    }
}

fn constant_literal(value: &str) -> Result<&str> {
    let parsed = apollo_compiler::ast::Document::parse(
        format!("input PoolsterDefault {{ value: String = {value} }}"),
        "introspection-default.graphql",
    )
    .map_err(|e| anyhow::anyhow!("invalid introspection defaultValue: {e}"))?;
    ensure!(
        parsed.definitions.len() == 1,
        "introspection defaultValue contains extra definitions"
    );
    let apollo_compiler::ast::Definition::InputObjectTypeDefinition(input) = &parsed.definitions[0]
    else {
        bail!("invalid introspection defaultValue")
    };
    ensure!(
        input.fields.len() == 1
            && input.directives.is_empty()
            && input.fields[0].directives.is_empty(),
        "introspection defaultValue contains extra syntax"
    );
    Ok(value)
}
