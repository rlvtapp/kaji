use super::*;
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{fmt::Write, path::Path};
fn scalar(client: &GraphqlClient, name: &str, input: bool) -> Result<&'static str> {
    let builtin = match name {
        "String" | "ID" => Some("string"),
        "Int" | "Float" => Some("number"),
        "Boolean" => Some("boolean"),
        _ => None,
    };
    if let Some(value) = builtin {
        return Ok(value);
    }
    let Some(mapping) = client.scalars.get(name) else {
        return Ok("unknown");
    };
    let value = if input {
        &mapping.input
    } else {
        &mapping.output
    };
    match value.trim() {
        "string" => Ok("string"),
        "number" => Ok("number"),
        "boolean" => Ok("boolean"),
        "unknown" => Ok("unknown"),
        _ => anyhow::bail!(
            "GraphQL helper scalar {name} has unsupported {} mapping {value:?}; runtime validators/fixtures require primitive wire types",
            if input { "input" } else { "output" }
        ),
    }
}
fn schema(client: &GraphqlClient, ty: &ModelType, input: bool) -> Result<String> {
    let mut value = match &ty.kind {
        ModelKind::Scalar(name) => {
            let kind = scalar(client, name, input)?;
            if kind == "unknown" {
                "z.unknown().refine(value => value !== undefined && value !== null, 'required non-null scalar')".into()
            } else if name == "Int" {
                "z.number().int().min(-2147483648).max(2147483647)".into()
            } else {
                format!("z.{kind}()")
            }
        }
        ModelKind::Enum(values) => format!("z.enum({})", serde_json::to_string(values)?),
        ModelKind::Literal(value) => format!("z.literal({})", serde_json::to_string(value)?),
        ModelKind::Named(name) => {
            ensure!(
                client.definition.input_objects.contains_key(name),
                "unknown GraphQL input object {name}"
            );
            format!("input{name}Schema")
        }
        ModelKind::List(item) => format!("z.array({})", schema(client, item, input)?),
        ModelKind::Object(fields) => object(client, fields, input)?,
        ModelKind::Union(variants) => {
            if variants.len() == 1 {
                schema(client, &variants[0], input)?
            } else {
                ensure!(
                    variants.len() >= 2,
                    "GraphQL helper union needs at least two alternatives"
                );
                format!(
                    "z.union([{}])",
                    variants
                        .iter()
                        .map(|ty| schema(client, ty, input))
                        .collect::<Result<Vec<_>>>()?
                        .join(",")
                )
            }
        }
    };
    if ty.nullable {
        value.push_str(".nullable()");
    }
    Ok(value)
}
fn object(client: &GraphqlClient, fields: &[ModelField], input: bool) -> Result<String> {
    let fields = fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}: {}{}",
                serde_json::to_string(&field.name)?,
                schema(client, &field.ty, input)?,
                if field.optional { ".optional()" } else { "" }
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(format!("z.object({{{}}}).strict()", fields.join(",")))
}
pub(super) fn zod(client: &GraphqlClient, target: &Path) -> Result<String> {
    let mut out = format!("import {{ z }} from 'zod';\n{}", imports(client, target)?);
    for (name, fields) in &client.definition.input_objects {
        writeln!(
            out,
            "const input{name}Schema: z.ZodTypeAny = z.lazy(() => {});",
            object(client, fields, true)?
        )?;
    }
    for op in &client.definition.operations {
        let symbols = &client.operations[&op.name];
        writeln!(
            out,
            "export const {}VariablesSchema: z.ZodType<{}> = {} as z.ZodType<{}>;",
            op.name,
            symbols.variables.name,
            object(client, &op.variables, true)?,
            symbols.variables.name
        )?;
        writeln!(
            out,
            "export const {}ResultSchema: z.ZodType<{}> = {} as z.ZodType<{}>;",
            op.name,
            symbols.result.name,
            schema(client, &op.result, false)?,
            symbols.result.name
        )?;
    }
    Ok(out)
}
fn value(
    client: &GraphqlClient,
    ty: &ModelType,
    input: bool,
    options: &FixtureOptions,
    depth: usize,
) -> Result<String> {
    if ty.nullable {
        return Ok("null".into());
    }
    ensure!(
        depth <= options.max_depth,
        "GraphQL fixture exceeds max_depth {}; required recursive inputs cannot terminate",
        options.max_depth
    );
    Ok(match &ty.kind {
        ModelKind::Scalar(name) => {
            let directional = format!("{name}.{}", if input { "input" } else { "output" });
            if let Some(value) = options
                .overrides
                .get(&directional)
                .or_else(|| options.overrides.get(name))
            {
                let kind = scalar(client, name, input)?;
                ensure!(
                    match kind {
                        "string" => value.is_string(),
                        "number" => value.is_number(),
                        "boolean" => value.is_boolean(),
                        _ => !value.is_null(),
                    },
                    "GraphQL fixture override {directional} does not match {kind} wire type"
                );
                if name == "Int" {
                    ensure!(
                        value
                            .as_i64()
                            .is_some_and(|value| i32::try_from(value).is_ok()),
                        "GraphQL Int fixture override must be a signed 32-bit integer"
                    );
                }
                serde_json::to_string(value)?
            } else {
                match scalar(client, name, input)? {
                    "string" => "faker.string.alphanumeric(12)".into(),
                    "number" => "faker.number.int({min:0,max:10000})".into(),
                    "boolean" => "faker.datatype.boolean()".into(),
                    _ => "{}".into(),
                }
            }
        }
        ModelKind::Enum(values) => serde_json::to_string(
            values
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty GraphQL enum"))?,
        )?,
        ModelKind::Literal(value) => serde_json::to_string(value)?,
        ModelKind::Named(name) => {
            let fields = client
                .definition
                .input_objects
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("unknown input object {name}"))?;
            fixture_object(client, fields, input, options, depth + 1)?
        }
        ModelKind::List(_) => "[]".into(),
        ModelKind::Object(fields) => fixture_object(client, fields, input, options, depth + 1)?,
        ModelKind::Union(variants) => value(
            client,
            variants
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty GraphQL union"))?,
            input,
            options,
            depth + 1,
        )?,
    })
}
fn fixture_object(
    client: &GraphqlClient,
    fields: &[ModelField],
    input: bool,
    options: &FixtureOptions,
    depth: usize,
) -> Result<String> {
    Ok(format!(
        "{{{}}}",
        fields
            .iter()
            .filter(|field| !field.optional)
            .map(|field| Ok(format!(
                "{}:{}",
                serde_json::to_string(&field.name)?,
                value(client, &field.ty, input, options, depth)?
            )))
            .collect::<Result<Vec<_>>>()?
            .join(",")
    ))
}
pub(super) fn faker(
    client: &GraphqlClient,
    target: &Path,
    options: &FixtureOptions,
) -> Result<String> {
    fn collect(
        client: &GraphqlClient,
        ty: &ModelType,
        input: bool,
        used: &mut std::collections::BTreeSet<(String, bool)>,
        seen: &mut std::collections::BTreeSet<String>,
    ) {
        if ty.nullable {
            return;
        }
        match &ty.kind {
            ModelKind::Scalar(name) => {
                used.insert((name.clone(), input));
            }
            ModelKind::Object(fields) => {
                for field in fields.iter().filter(|field| !field.optional) {
                    collect(client, &field.ty, input, used, seen);
                }
            }
            ModelKind::Named(name) => {
                if seen.insert(name.clone()) {
                    if let Some(fields) = client.definition.input_objects.get(name) {
                        for field in fields.iter().filter(|field| !field.optional) {
                            collect(client, &field.ty, input, used, seen);
                        }
                    }
                    seen.remove(name);
                }
            }
            ModelKind::Union(items) => {
                if let Some(first) = items.first() {
                    collect(client, first, input, used, seen);
                }
            }
            _ => {}
        }
    }
    let mut used = std::collections::BTreeSet::new();
    for op in &client.definition.operations {
        collect(
            client,
            &op.result,
            false,
            &mut used,
            &mut Default::default(),
        );
        for field in op.variables.iter().filter(|field| !field.optional) {
            collect(client, &field.ty, true, &mut used, &mut Default::default());
        }
    }
    for key in options.overrides.keys() {
        let (name, direction) = if let Some(name) = key.strip_suffix(".input") {
            (name, Some(true))
        } else if let Some(name) = key.strip_suffix(".output") {
            (name, Some(false))
        } else {
            (key.as_str(), None)
        };
        ensure!(
            used.iter().any(|(used_name, input)| used_name == name
                && direction.is_none_or(|direction| direction == *input)),
            "GraphQL Faker override {key:?} does not apply to a generated required non-null scalar; optional fields are omitted, nullable fields are null, lists are empty, and object overrides are unsupported"
        );
    }
    let mut out = format!(
        "import {{ Faker, en }} from '@faker-js/faker';\nconst faker=new Faker({{locale:[en]}});\nexport function seed(value:number):void {{ faker.seed(value); }}\n{}",
        imports(client, target)?
    );
    if let Some(seed) = options.seed {
        writeln!(out, "faker.seed({seed});")?;
    }
    for op in &client.definition.operations {
        let symbols = &client.operations[&op.name];
        writeln!(
            out,
            "export function fake{}Variables(): {} {{ return {}; }}",
            op.name,
            symbols.variables.name,
            fixture_object(client, &op.variables, true, options, 0)?
        )?;
        writeln!(
            out,
            "export function fake{}Result(): {} {{ return {}; }}",
            op.name,
            symbols.result.name,
            value(client, &op.result, false, options, 0)?
        )?;
    }
    Ok(out)
}
