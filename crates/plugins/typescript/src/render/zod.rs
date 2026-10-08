//! Zod auxiliary rendering.
use super::*;

/// Emits Zod schemas and their inferred TypeScript types.
#[derive(Default)]
pub struct TypeScriptZod;

impl TypeScriptZod {
    pub(crate) fn generate_with_models(
        &self,
        api: &Api,
        config: &ArtifactOptions,
        options: &crate::ModelOptions,
    ) -> Result<Vec<GeneratedFile>> {
        self.generate(&crate::json::artifact_api(api, options), config)
    }

    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        let prepared = crate::auxiliary_layout::prepare(api);
        let api = &prepared;
        let mut output = format!("{NOTICE}\nimport {{ z }} from 'zod';\n\n");
        let inline_cycles = crate::auxiliary_layout::has_cycles(api)
            && matches!(config.layout, Some(crate::SourceLayout::SingleFile));
        if inline_cycles {
            output.push_str(&super::render_models(api));
        }
        for schema in &api.schemas {
            let name = type_identifier(&schema.name);
            let annotation = if inline_cycles {
                format!(": z.ZodType<{name}>")
            } else {
                String::new()
            };
            let _ = writeln!(
                output,
                "export const {name}Schema{annotation} = {};",
                render_zod(&schema.value)
            );
            if !inline_cycles {
                let _ = writeln!(
                    output,
                    "export type {name} = z.infer<typeof {name}Schema>;\n"
                );
            }
        }
        render_zod_operation_schemas(&mut output, api);
        render_zod_registry(&mut output, api);
        anyhow::ensure!(config.max_file_bytes > 0, "max_file_bytes must be positive");
        if crate::auxiliary_layout::uses_modules(api, config, output.len(), false)?
            || (crate::auxiliary_layout::has_cycles(api) && !inline_cycles)
        {
            return crate::auxiliary_layout::zod(api, config);
        }
        Ok(vec![GeneratedFile::new(
            extra_output_path(config, "typescript", "zod.ts"),
            output,
        )?])
    }
}

/// Exposes media-specific schemas rather than guessing which representation a
/// caller will send or receive. This makes the generated module usable from a
/// hook or wrapper without coupling the generated SDK runtime to Zod.
pub(crate) fn render_zod_operation_schemas(output: &mut String, api: &Api) {
    for operation in &api.operations {
        let name = type_identifier(&operation.id);
        if let Some(body) = &operation.request_body {
            let media = body
                .media_types
                .iter()
                .filter_map(|m| m.schema.as_ref().map(|s| (m, s)))
                .collect::<Vec<_>>();
            if !media.is_empty() {
                let _ = writeln!(output, "export const {name}RequestBodySchemas: {{");
                for (m, s) in &media {
                    let _ = writeln!(
                        output,
                        " readonly {}: z.ZodType<{}>;",
                        js_string(&m.content_type),
                        render_value(s)
                    );
                }
                output.push_str("} = {\n");
                for (m, s) in &media {
                    let _ = writeln!(
                        output,
                        " {}: {},",
                        js_string(&m.content_type),
                        render_zod(s)
                    );
                }
                output.push_str("} as const;\n\n");
            }
        }
        let responses = operation
            .responses
            .iter()
            .filter(|r| r.media_types.iter().any(|m| m.schema.is_some()))
            .collect::<Vec<_>>();
        if !responses.is_empty() {
            let _ = writeln!(output, "export const {name}ResponseSchemas: {{");
            for response in &responses {
                let _ = writeln!(output, " readonly {}: {{", js_string(&response.status));
                for m in &response.media_types {
                    if let Some(schema) = &m.schema {
                        let _ = writeln!(
                            output,
                            " readonly {}: z.ZodType<{}>;",
                            js_string(&m.content_type),
                            render_value(schema)
                        );
                    }
                }
                output.push_str(" };\n");
            }
            output.push_str("} = {\n");
            for response in &responses {
                let _ = writeln!(output, " {}: {{", js_string(&response.status));
                for m in &response.media_types {
                    if let Some(schema) = &m.schema {
                        let _ = writeln!(
                            output,
                            " {}: {},",
                            js_string(&m.content_type),
                            render_zod(schema)
                        );
                    }
                }
                output.push_str(" },\n");
            }
            output.push_str("} as const;\n\n");
        }
    }
}

/// Renders a deliberately small, stable public contract for consumers which
/// want to opt into validation. Zod 4 schema values implement Standard Schema
/// V1, so callers can use either Zod's `parse`/`safeParse` API or a Standard
/// Schema-aware integration without a second validation dependency.
pub(crate) fn render_zod_registry(output: &mut String, api: &Api) {
    output.push_str(
        "/**\n * Component schemas keyed by their OpenAPI component name.\n *\n * Every value is a Zod 4 schema and therefore implements Standard Schema V1.\n */\nexport const kajiSchemas = {\n",
    );
    for schema in &api.schemas {
        let _ = writeln!(
            output,
            "  {}: {}Schema,",
            js_string(
                schema
                    .value
                    .extensions
                    .get("poolster.aux.schema_name")
                    .and_then(Value::as_str)
                    .unwrap_or(&schema.name)
            ),
            type_identifier(&schema.name)
        );
    }
    output.push_str("} as const;\n\n");
    output.push_str("export type PoolsterSchemaName = keyof typeof kajiSchemas;\n");
    output.push_str("export type PoolsterSchema = (typeof kajiSchemas)[PoolsterSchemaName];\n");
    output.push_str(
        "export const getPoolsterSchema = <Name extends PoolsterSchemaName>(name: Name): (typeof kajiSchemas)[Name] => kajiSchemas[name];\n\n",
    );

    render_zod_operation_registry(output, api, "kajiOperationSchemas");
    output.push_str("export type PoolsterOperationId = keyof typeof kajiOperationSchemas;\n");
    output.push_str("export type PoolsterOperationSchemas = typeof kajiOperationSchemas;\n");
    output.push_str(
        "export const getPoolsterOperationSchemas = <Operation extends PoolsterOperationId>(operation: Operation): PoolsterOperationSchemas[Operation] => kajiOperationSchemas[operation];\n",
    );
}

pub(crate) fn render_zod_operation_registry(output: &mut String, api: &Api, registry: &str) {
    let _ = writeln!(output, "export const {registry}: {{");
    for operation in &api.operations {
        let name = type_identifier(&operation.id);
        let key = operation
            .annotations
            .get("poolster.aux.operation_id")
            .and_then(Value::as_str)
            .unwrap_or(&operation.id);
        let _ = writeln!(output, " readonly {}: {{", js_string(key));
        if operation
            .request_body
            .as_ref()
            .is_some_and(|b| b.media_types.iter().any(|m| m.schema.is_some()))
        {
            let _ = writeln!(
                output,
                " readonly request: typeof {name}RequestBodySchemas;"
            );
        }
        if operation
            .responses
            .iter()
            .any(|r| r.media_types.iter().any(|m| m.schema.is_some()))
        {
            let _ = writeln!(output, " readonly responses: typeof {name}ResponseSchemas;");
        }
        output.push_str(" };\n");
    }
    output.push_str("} = {\n");
    for operation in &api.operations {
        let name = type_identifier(&operation.id);
        let request = operation
            .request_body
            .as_ref()
            .is_some_and(|body| body.media_types.iter().any(|media| media.schema.is_some()));
        let responses = operation.responses.iter().any(|response| {
            response
                .media_types
                .iter()
                .any(|media| media.schema.is_some())
        });
        let _ = writeln!(
            output,
            "  {}: {{",
            js_string(
                operation
                    .annotations
                    .get("poolster.aux.operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or(&operation.id)
            )
        );
        if request {
            let _ = writeln!(output, "    request: {name}RequestBodySchemas,");
        }
        if responses {
            let _ = writeln!(output, "    responses: {name}ResponseSchemas,");
        }
        output.push_str("  },\n");
    }
    output.push_str("} as const;\n\n");
}

fn artifact_zod_literal(literal: &Value, schema: &SchemaValue) -> Option<String> {
    if matches!(literal, Value::Array(_) | Value::Object(_)) {
        return None;
    }
    artifact_literal(literal, schema).map(|literal| format!("z.literal({literal})"))
}

pub(crate) fn artifact_literal(literal: &Value, schema: &SchemaValue) -> Option<String> {
    if literal.is_number() {
        match schema
            .extensions
            .get("x-kaji-integer")
            .and_then(Value::as_str)
        {
            Some("bigint") => return Some(format!("{literal}n")),
            Some("string") => return Some(js_string(&literal.to_string())),
            _ => {}
        }
    }
    ts_literal(literal)
}

pub(crate) fn render_zod(value: &SchemaValue) -> String {
    let primitive = match &value.kind {
        SchemaKind::Any => "z.unknown()".to_owned(),
        SchemaKind::Null => "z.null()".to_owned(),
        SchemaKind::Boolean => "z.boolean()".to_owned(),
        SchemaKind::Integer => match value
            .extensions
            .get("x-kaji-integer")
            .and_then(Value::as_str)
        {
            Some("bigint") => "z.bigint()".into(),
            Some("string") => "z.string().regex(/^-?(?:0|[1-9]\\d*)$/)".into(),
            _ => "z.number().int()".into(),
        },
        SchemaKind::Number => "z.number()".to_owned(),
        SchemaKind::String => "z.string()".to_owned(),
        SchemaKind::Array { items } => format!("z.array({})", render_zod(items)),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let fields = fields
                .iter()
                .map(|field| {
                    let optional = if field.required { "" } else { ".optional()" };
                    format!(
                        "{}: {}{}",
                        js_string(&field.name),
                        render_zod(&field.value),
                        optional
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let object = format!("z.object({{ {fields} }})");
            match additional_properties {
                AdditionalProperties::Forbidden => format!("{object}.strict()"),
                AdditionalProperties::Any | AdditionalProperties::Unspecified => {
                    format!("{object}.catchall(z.unknown())")
                }
                AdditionalProperties::Schema { value } => {
                    format!("{object}.catchall({})", render_zod(value))
                }
            }
        }
        SchemaKind::Reference { reference } => format!(
            "z.lazy(() => {}Schema)",
            type_identifier(reference.rsplit('/').next().unwrap_or(reference))
        ),
        SchemaKind::OneOf { variants } => {
            let schemas = variants.iter().map(render_zod).collect::<Vec<_>>();
            match schemas.len() {
                0 => "z.never()".into(),
                1 => schemas[0].clone(),
                _ => format!(
                    "z.union([{}]).refine((value) => [{}].filter(schema => schema.safeParse(value).success).length === 1)",
                    schemas.join(", "),
                    schemas.join(", ")
                ),
            }
        }
        SchemaKind::AnyOf { variants } => match variants.as_slice() {
            [] => "z.never()".to_owned(),
            [one] => render_zod(one),
            values => format!(
                "z.union([{}])",
                values.iter().map(render_zod).collect::<Vec<_>>().join(", ")
            ),
        },
        SchemaKind::AllOf { variants } => match variants.as_slice() {
            [] => "z.unknown()".to_owned(),
            [one] => render_zod(one),
            values => values
                .iter()
                .skip(1)
                .fold(render_zod(&values[0]), |current, next| {
                    format!("z.intersection({current}, {})", render_zod(next))
                }),
        },
        SchemaKind::Not { schema } => format!(
            "z.unknown().refine((value) => !({}).safeParse(value).success)",
            render_zod(schema)
        ),
    };
    let constrained = if let Some(constant) = value
        .const_value
        .as_ref()
        .and_then(|literal| artifact_zod_literal(literal, value))
    {
        constant
    } else if !value.enum_values.is_empty() {
        let values = value
            .enum_values
            .iter()
            .filter_map(|literal| artifact_zod_literal(literal, value))
            .collect::<Vec<_>>();
        if values.len() == 1 {
            values[0].clone()
        } else if values.is_empty() {
            primitive
        } else {
            format!("z.union([{}])", values.join(", "))
        }
    } else {
        primitive
    };
    let constrained = crate::auxiliary_validation::constrain(constrained, value);
    let constrained = if value.nullish {
        format!("{constrained}.nullish()")
    } else if value.nullable {
        format!("{constrained}.nullable()")
    } else {
        constrained
    };
    if value.optional && !value.nullish {
        format!("{constrained}.optional()")
    } else {
        constrained
    }
}
