//! Optional target-neutral API reference consumer. Never includes example/default values.
use crate::{
    Api, GeneratedFile, SchemaKind, SchemaValue,
    engine::{Contract, Handle, Language, Meta, Plugin, PluginContext, Provision},
};
use anyhow::Result;
use std::{fmt::Write, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct ApiReferenceDocument {
    pub path: String,
    pub contents: String,
}
impl Contract for ApiReferenceDocument {
    const NAME: &'static str = "kaji.api-reference.v1";
}
pub struct ApiReference<L: Language> {
    meta: Meta,
    output: String,
    language: PhantomData<L>,
}
pub fn api_reference<L: Language>() -> ApiReference<L> {
    ApiReference {
        meta: Meta::new(),
        output: "API_REFERENCE.md".into(),
        language: PhantomData,
    }
}
impl<L: Language> ApiReference<L> {
    pub fn handle(&self) -> Handle<ApiReferenceDocument> {
        self.meta.handle()
    }
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.output = path.into();
        self
    }
}
impl<L: Language> Plugin<L> for ApiReference<L> {
    fn kind(&self) -> &'static str {
        "api-reference"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<ApiReferenceDocument>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let contents = render(cx.api);
        cx.files
            .emit(GeneratedFile::new(&self.output, &contents)?)?;
        cx.publish(ApiReferenceDocument {
            path: self.output.clone(),
            contents,
        })
    }
}
fn escaped(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '|' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '!' | '#' | ':' | '\\' => {
                format!("&#{};", c as u32)
            }
            c if c.is_control() => " ".into(),
            _ => c.to_string(),
        })
        .collect()
}
fn shape(schema: Option<&SchemaValue>, depth: usize) -> String {
    let Some(schema) = schema else {
        return "No schema declared".into();
    };
    if depth > 12 {
        return "Nested schema (depth limit)".into();
    }
    let mut result = match &schema.kind {
        SchemaKind::Any => "any".into(),
        SchemaKind::Null => "null".into(),
        SchemaKind::Boolean => "boolean".into(),
        SchemaKind::Integer => "integer".into(),
        SchemaKind::Number => "number".into(),
        SchemaKind::String => "string".into(),
        SchemaKind::Reference { reference } => {
            if reference.starts_with('#') {
                format!("Reference {}", escaped(reference))
            } else {
                "External reference (URI omitted)".into()
            }
        }
        SchemaKind::Array { items } => format!("array of {}", shape(Some(items), depth + 1)),
        SchemaKind::Object { fields, .. } => format!("object ({} fields)", fields.len()),
        SchemaKind::OneOf { variants } => format!("oneOf ({} variants)", variants.len()),
        SchemaKind::AnyOf { variants } => format!("anyOf ({} variants)", variants.len()),
        SchemaKind::AllOf { variants } => format!("allOf ({} variants)", variants.len()),
        SchemaKind::Not { .. } => "not schema".into(),
    };
    if schema.nullable || schema.nullish {
        result.push_str("; nullable")
    }
    if schema.read_only {
        result.push_str("; readOnly")
    }
    if schema.write_only {
        result.push_str("; writeOnly")
    }
    result
}
pub fn render(api: &Api) -> String {
    let mut out = format!(
        "# {} API reference\n\nVersion: {}\n\nThis reference describes the normalized API contract, not generated client method names. Examples, defaults, source descriptions and credential values are omitted. External reference URIs are omitted.\n\n",
        escaped(&api.name),
        escaped(&api.version)
    );
    let mut operations = api.operations.iter().collect::<Vec<_>>();
    operations.sort_by(|a, b| (&a.id, &a.path).cmp(&(&b.id, &b.path)));
    for op in operations {
        let _ = writeln!(
            out,
            "## {}\n\n{} {}\n",
            escaped(&op.id),
            op.method.as_str(),
            escaped(&op.path)
        );
        out.push_str("### Parameters\n\n| Name | Location | Required | Schema |\n| --- | --- | --- | --- |\n");
        for p in &op.parameters {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                escaped(&p.name),
                escaped(&p.location),
                p.required,
                shape(p.schema.as_ref(), 0)
            );
        }
        if op.parameters.is_empty() {
            out.push_str("| — | — | — | None declared |\n")
        }
        if let Some(policy) = crate::idempotency::resolved(op) {
            let _ = writeln!(
                out,
                "\n### Idempotency\n\nKey header: `{}`. {} Caller-supplied keys take precedence and each automatic retry reuses the same key. To retry a logical operation across SDK calls or process restarts, supply and persist your own key. The API server must implement deduplication.\n",
                escaped(&policy.header),
                if policy.auto_generate {
                    "An omitted key receives a fresh UUID per SDK call."
                } else {
                    "Keys are caller-supplied; automatic generation is disabled."
                }
            );
        }
        if let Some(body) = &op.request_body {
            let _ = writeln!(
                out,
                "\n### Request body\n\nRequired: {}\n\n| Media type | Schema |\n| --- | --- |",
                body.required
            );
            for media in &body.media_types {
                let _ = writeln!(
                    out,
                    "| {} | {} |",
                    escaped(&media.content_type),
                    shape(media.schema.as_ref(), 0)
                );
            }
        }
        out.push_str("\n### Responses\n\n| Status | Media type | Schema |\n| --- | --- | --- |\n");
        for response in &op.responses {
            if response.media_types.is_empty() {
                let _ = writeln!(
                    out,
                    "| {} | — | No representation declared |",
                    escaped(&response.status)
                );
            }
            for media in &response.media_types {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    escaped(&response.status),
                    escaped(&media.content_type),
                    shape(media.schema.as_ref(), 0)
                );
            }
        }
        if op.responses.is_empty() {
            out.push_str("| — | — | None declared |\n")
        }
        out.push('\n');
    }
    out.push_str("## Component schemas\n\n| Name | Shape |\n| --- | --- |\n");
    let mut schemas = api.schemas.iter().collect::<Vec<_>>();
    schemas.sort_by(|a, b| a.name.cmp(&b.name));
    for schema in &schemas {
        let _ = writeln!(
            out,
            "| {} | {} |",
            escaped(&schema.name),
            shape(Some(&schema.value), 0)
        );
    }
    for schema in schemas {
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let _ = writeln!(
                out,
                "\n### {} fields\n\n| Name | Required | Shape |\n| --- | --- | --- |",
                escaped(&schema.name)
            );
            for field in fields {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    escaped(&field.name),
                    field.required,
                    shape(Some(&field.value), 0)
                );
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HttpMethod, Operation, OperationMediaType, OperationResponse, Schema, engine::Packages,
    };
    struct Test;
    impl Language for Test {
        const NAME: &'static str = "test";
        type Settings = ();
        type Workspace = ();
    }
    #[test]
    fn reference_escapes_untrusted_text_and_omits_source_secrets() {
        let mut schema = SchemaValue::new(SchemaKind::String);
        schema.default = Some(serde_json::json!("secret-default"));
        schema.enum_values = vec![serde_json::json!("secret-enum")];
        schema.description = Some("secret-description".into());
        let mut api = Api {
            name: "<script>|API".into(),
            schemas: vec![Schema::new("name|`", schema)],
            ..Default::default()
        };
        api.operations.push(Operation {
            id: "read[evil](https://evil)".into(),
            method: HttpMethod::Get,
            path: "/things/{id}".into(),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: Some("secret-response".into()),
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference(
                        "https://example.test/schema?secret-token",
                    )),
                }],
            }],
            ..Default::default()
        });
        let text = render(&api);
        assert!(!text.contains("<script>"));
        assert!(!text.contains("secret"));
        assert!(text.contains("application/json"));
        assert!(text.contains("External reference (URI omitted)"));
        assert!(text.contains("&#124;"));
    }
    #[test]
    fn reference_documents_resolved_package_policy() {
        let api = Api {
            operations: vec![crate::Operation {
                id: "createOrder".into(),
                method: crate::HttpMethod::Post,
                path: "/orders".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let config = crate::idempotency::IdempotencyConfig {
            operations: std::collections::BTreeMap::from([(
                "createOrder".into(),
                crate::idempotency::IdempotencyRule {
                    header: "X-Once".into(),
                    auto_generate: true,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let tree = Packages::new()
            .package(
                crate::engine::Package::<Test>::new("docs")
                    .idempotency(config)
                    .with(api_reference()),
            )
            .generate(&api, None)
            .unwrap();
        let document = tree.get("docs/API_REFERENCE.md").unwrap();
        assert!(document.contains("Key header: `X-Once`"));
        assert!(document.contains("fresh UUID per SDK call"));
        assert!(document.contains("process restarts"));
        assert!(document.contains("server must implement deduplication"));
        assert!(api.operations[0].parameters.is_empty());
    }

    #[test]
    fn plugin_emits_owned_custom_output_and_rejects_escaping_paths() {
        let package = crate::engine::Package::<Test>::new("docs")
            .with(api_reference().output("reference/API.md"));
        let tree = Packages::new()
            .package(package)
            .generate(&Api::default(), None)
            .unwrap();
        assert!(tree.get("docs/reference/API.md").is_some());
        assert!(
            Packages::new()
                .package(
                    crate::engine::Package::<Test>::new("docs")
                        .with(api_reference().output("../escape"))
                )
                .generate(&Api::default(), None)
                .is_err()
        );
    }
}
