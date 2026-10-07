//! Validated pagination declarations shared by native generator plugins.
use crate::{Api, Operation, SchemaKind, SchemaValue};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaginationKind {
    Cursor,
    OffsetLimit,
    Page,
    Url,
}

/// Same portable shape as x-kaji-pagination. An explicit rule overrides extensions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaginationRule {
    #[serde(rename = "type")]
    pub kind: PaginationKind,
    #[serde(default)]
    pub inputs: Vec<PaginationInputRule>,
    pub outputs: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaginationInputRule {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "in", default = "parameters")]
    pub location: String,
}
fn parameters() -> String {
    "parameters".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PaginationPlan {
    pub kind: PaginationKind,
    pub inputs: Vec<PaginationInput>,
    pub results: Option<Selector>,
    pub continuation: Option<Selector>,
    /// Renderers must constrain URL continuations to the configured API origin.
    pub same_origin: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum PaginationValueKind {
    String,
    Integer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PaginationInput {
    pub name: String,
    pub role: String,
    pub location: String,
    pub required: bool,
    pub value_kind: PaginationValueKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Selector {
    pub expression: String,
    pub segments: Vec<SelectorSegment>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum SelectorSegment {
    Field(String),
    Index(i64),
}

impl Selector {
    /// Supported portable paths: $.items, $.pages[-1].next and JSON pointers.
    /// Wildcards, filters and script expressions are rejected rather than guessed.
    pub fn parse(expression: &str) -> Result<Self> {
        let mut segments = Vec::new();
        if let Some(pointer) = expression.strip_prefix('/') {
            for part in pointer.split('/') {
                let mut decoded = String::new();
                let mut chars = part.chars();
                while let Some(ch) = chars.next() {
                    if ch == '~' {
                        decoded.push(match chars.next() {
                            Some('0') => '~',
                            Some('1') => '/',
                            _ => bail!("invalid JSON pointer escape"),
                        });
                    } else {
                        decoded.push(ch);
                    }
                }
                segments.push(SelectorSegment::Field(decoded));
            }
        } else {
            let mut rest = expression
                .strip_prefix('$')
                .context("pagination selector must start with $ or /")?;
            while !rest.is_empty() {
                if let Some(after) = rest.strip_prefix('.') {
                    let end = after.find(['.', '[']).unwrap_or(after.len());
                    let name = &after[..end];
                    if name.is_empty()
                        || name.chars().any(|ch| {
                            ch.is_whitespace() || matches!(ch, '*' | '?' | ']' | '(' | ')' | '$')
                        })
                    {
                        bail!("unsupported pagination selector field");
                    }
                    segments.push(SelectorSegment::Field(name.into()));
                    rest = &after[end..];
                } else if let Some(after) = rest.strip_prefix('[') {
                    let end = after
                        .find(']')
                        .context("unterminated pagination selector index")?;
                    segments.push(SelectorSegment::Index(
                        after[..end]
                            .parse()
                            .context("pagination selector index must be an integer")?,
                    ));
                    rest = &after[end + 1..];
                } else {
                    bail!("invalid pagination selector syntax");
                }
            }
        }
        Ok(Self {
            expression: expression.into(),
            segments,
        })
    }
    pub fn select<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut value = value;
        for segment in &self.segments {
            value = match segment {
                SelectorSegment::Field(name) => match value {
                    Value::Array(items) => items.get(name.parse::<usize>().ok()?)?,
                    _ => value.get(name)?,
                },
                SelectorSegment::Index(index) => {
                    let items = value.as_array()?;
                    let index = if *index < 0 {
                        i64::try_from(items.len()).ok()?.checked_add(*index)?
                    } else {
                        *index
                    };
                    items.get(usize::try_from(index).ok()?)?
                }
            };
        }
        Some(value)
    }
}

/// Validate an explicit recipe declaration or a retained Kaji/Speakeasy extension.
/// Invalid declarations return a diagnostic instead of silently disabling pagination.
pub fn normalize_pagination(
    api: &Api,
    operation: &Operation,
    explicit: Option<&PaginationRule>,
) -> Result<Option<PaginationPlan>> {
    let parsed;
    let rule = if let Some(rule) = explicit {
        rule
    } else {
        let Some(extension) = operation
            .annotations
            .get("x-kaji-pagination")
            .or_else(|| operation.annotations.get("x-speakeasy-pagination"))
        else {
            return Ok(None);
        };
        parsed = serde_json::from_value::<PaginationRule>(extension.clone())
            .with_context(|| format!("invalid pagination declaration for {}", operation.id))?;
        &parsed
    };
    if operation
        .responses
        .iter()
        .flat_map(|r| &r.media_types)
        .any(|media| {
            media.content_type == "text/event-stream"
                || media.content_type == "application/octet-stream"
        })
    {
        bail!("pagination requires a decoded JSON response");
    }
    let mut roles = match rule.kind {
        PaginationKind::Cursor => vec!["cursor"],
        PaginationKind::OffsetLimit => vec!["offset", "limit"],
        PaginationKind::Page => vec!["page"],
        PaginationKind::Url => vec![],
    };
    if matches!(rule.kind, PaginationKind::Cursor | PaginationKind::Page)
        && rule.inputs.iter().any(|input| input.kind == "limit")
    {
        roles.push("limit");
    }
    let mut inputs = Vec::new();
    for role in &roles {
        let matches = rule
            .inputs
            .iter()
            .filter(|input| input.kind == *role)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            bail!("pagination needs exactly one {role} input");
        }
        let input = matches[0];
        let (schema, location, required) = match input.location.as_str() {
            "parameters" => {
                let parameters = operation
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.name == input.name)
                    .collect::<Vec<_>>();
                if parameters.len() != 1 {
                    bail!(
                        "pagination input {} must identify one parameter",
                        input.name
                    );
                }
                let parameter = parameters[0];
                if !matches!(parameter.location.as_str(), "query" | "header" | "path")
                    || (parameter.location == "path" && !parameter.required)
                {
                    bail!("unsupported pagination parameter location");
                }
                (
                    parameter
                        .schema
                        .as_ref()
                        .context("pagination parameter has no schema")?,
                    parameter.location.clone(),
                    parameter.required,
                )
            }
            "requestBody" => {
                let body = operation
                    .request_body
                    .as_ref()
                    .context("pagination body is required")?;
                let media = body
                    .media_types
                    .iter()
                    .find(|media| {
                        media.content_type == "application/json"
                            || media.content_type.ends_with("+json")
                    })
                    .context("pagination body must be JSON")?;
                let schema = resolve(
                    api,
                    media
                        .schema
                        .as_ref()
                        .context("pagination body has no schema")?,
                )?;
                let SchemaKind::Object { fields, .. } = &schema.kind else {
                    bail!("pagination body must be an object");
                };
                let field = fields
                    .iter()
                    .find(|field| field.name == input.name)
                    .context("pagination body field not found")?;
                (&field.value, "requestBody".into(), field.required)
            }
            _ => bail!("unsupported pagination input location"),
        };
        let kind = &resolve(api, schema)?.kind;
        if (*role == "cursor" && !matches!(kind, SchemaKind::String | SchemaKind::Integer))
            || (*role != "cursor" && !matches!(kind, SchemaKind::Integer))
        {
            bail!("pagination {role} input has incompatible type");
        }
        inputs.push(PaginationInput {
            name: input.name.clone(),
            role: role.to_string(),
            location,
            required,
            value_kind: if matches!(kind, SchemaKind::String) {
                PaginationValueKind::String
            } else {
                PaginationValueKind::Integer
            },
        });
    }
    if rule
        .inputs
        .iter()
        .any(|input| !roles.contains(&input.kind.as_str()))
    {
        bail!("unknown pagination input role");
    }
    let continuation_key = match rule.kind {
        PaginationKind::Cursor => Some("nextCursor"),
        PaginationKind::Url => Some("nextUrl"),
        _ => None,
    };
    let continuation = continuation_key
        .map(|key| {
            Selector::parse(
                rule.outputs
                    .get(key)
                    .with_context(|| format!("pagination outputs.{key} is required"))?,
            )
        })
        .transpose()?;
    let results = rule
        .outputs
        .get("results")
        .map(|path| Selector::parse(path))
        .transpose()?;
    if matches!(
        rule.kind,
        PaginationKind::OffsetLimit | PaginationKind::Page
    ) && results.is_none()
    {
        bail!("pagination outputs.results is required");
    }
    if let Some(schema) = operation
        .responses
        .iter()
        .filter(|response| {
            response
                .status
                .parse::<u16>()
                .is_ok_and(|status| (200..300).contains(&status))
        })
        .flat_map(|response| &response.media_types)
        .filter(|media| {
            media.content_type == "application/json" || media.content_type.ends_with("+json")
        })
        .find_map(|media| media.schema.as_ref())
    {
        if let Some(selector) = &results {
            if !matches!(
                resolve(api, selected_schema(api, schema, selector)?)?.kind,
                SchemaKind::Array { .. }
            ) {
                bail!("pagination results selector must resolve to an array");
            }
        }
        if let Some(selector) = &continuation {
            let kind = &resolve(api, selected_schema(api, schema, selector)?)?.kind;
            if !matches!(
                kind,
                SchemaKind::String | SchemaKind::Integer | SchemaKind::Null
            ) || (rule.kind == PaginationKind::Url
                && !matches!(kind, SchemaKind::String | SchemaKind::Null))
            {
                bail!("pagination continuation selector has incompatible type");
            }
        }
    }
    Ok(Some(PaginationPlan {
        kind: rule.kind.clone(),
        inputs,
        results,
        continuation,
        same_origin: rule.kind == PaginationKind::Url,
    }))
}

fn resolve<'a>(api: &'a Api, mut schema: &'a SchemaValue) -> Result<&'a SchemaValue> {
    let mut seen = std::collections::BTreeSet::new();
    while let SchemaKind::Reference { reference } = &schema.kind {
        if !seen.insert(reference) {
            bail!("recursive pagination schema reference");
        }
        let name = reference
            .rsplit('/')
            .next()
            .unwrap()
            .replace("~1", "/")
            .replace("~0", "~");
        schema = &api
            .schemas
            .iter()
            .find(|schema| schema.name == name)
            .context("pagination schema reference not found")?
            .value;
    }
    Ok(schema)
}
fn selected_schema<'a>(
    api: &'a Api,
    mut schema: &'a SchemaValue,
    selector: &Selector,
) -> Result<&'a SchemaValue> {
    for segment in &selector.segments {
        schema = resolve(api, schema)?;
        schema = match (&schema.kind, segment) {
            (SchemaKind::Object { fields, .. }, SelectorSegment::Field(name)) => {
                &fields
                    .iter()
                    .find(|field| &field.name == name)
                    .context("pagination selector field not found")?
                    .value
            }
            (SchemaKind::Array { items }, SelectorSegment::Index(_)) => items,
            (SchemaKind::Array { items }, SelectorSegment::Field(name))
                if name.parse::<usize>().is_ok() =>
            {
                items
            }
            _ => bail!("pagination selector does not match response schema"),
        };
    }
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdditionalProperties, Field, OperationMediaType, OperationParameter, OperationResponse,
    };
    use serde_json::json;
    fn operation() -> Operation {
        Operation {
            id: "list".into(),
            parameters: vec![OperationParameter { name: "cursor".into(), location: "query".into(), required: false, schema: Some(SchemaValue::new(SchemaKind::String)), description: None, annotations: Default::default() }],
            responses: vec![OperationResponse { status: "200".into(), description: None, media_types: vec![OperationMediaType { content_type: "application/json".into(), schema: Some(SchemaValue::new(SchemaKind::Object { fields: vec![Field { name: "next".into(), value: SchemaValue::new(SchemaKind::String), required: false, annotations: Default::default() }], additional_properties: AdditionalProperties::Forbidden })) }] }],
            annotations: [("x-speakeasy-pagination".into(), json!({"type":"cursor", "inputs":[{"name":"cursor", "type":"cursor", "in":"parameters"}], "outputs":{"nextCursor":"$.next"}}))].into(),
            ..Default::default()
        }
    }
    #[test]
    fn normalizes_extensions_and_explicit_rules_with_schema_validation() {
        let mut operation = operation();
        let api = Api::default();
        let plan = normalize_pagination(&api, &operation, None)
            .unwrap()
            .unwrap();
        assert_eq!(plan.inputs[0].location, "query");
        assert_eq!(
            plan.continuation.unwrap().select(&json!({"next":"abc"})),
            Some(&json!("abc"))
        );
        let explicit: PaginationRule =
            serde_json::from_value(json!({"type":"url", "outputs":{"nextUrl":"$.next"}})).unwrap();
        assert!(
            normalize_pagination(&api, &operation, Some(&explicit))
                .unwrap()
                .unwrap()
                .same_origin
        );
        operation.parameters[0].schema = Some(SchemaValue::new(SchemaKind::Boolean));
        assert!(normalize_pagination(&api, &operation, None).is_err());
    }
    #[test]
    fn rejects_invalid_selectors_and_declarations_without_guessing() {
        for path in [
            "next",
            "$..next",
            "$.items[*]",
            "$.items[foo]",
            "/bad~2escape",
        ] {
            assert!(Selector::parse(path).is_err(), "{path}");
        }
        assert_eq!(
            Selector::parse("$.pages[-1].next")
                .unwrap()
                .select(&json!({"pages":[{"next":"last"}]})),
            Some(&json!("last"))
        );
        assert_eq!(
            Selector::parse("/a~1b/0")
                .unwrap()
                .select(&json!({"a/b":[42]})),
            Some(&json!(42))
        );
        let mut operation = operation();
        operation.annotations.insert(
            "x-kaji-pagination".into(),
            json!({"type":"cursor", "inputs":[], "outputs":{"nextCursor":"$.next"}}),
        );
        assert!(normalize_pagination(&Api::default(), &operation, None).is_err());
        operation.annotations.clear();
        assert!(
            normalize_pagination(&Api::default(), &operation, None)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn validates_page_and_offset_parameters_and_result_arrays() {
        let mut operation = operation();
        operation.parameters = ["page", "limit"]
            .into_iter()
            .map(|name| OperationParameter {
                name: name.into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            })
            .collect();
        operation.responses.clear();
        let rule: PaginationRule = serde_json::from_value(json!({"type":"page", "inputs":[{"name":"page", "type":"page"},{"name":"limit", "type":"limit"}], "outputs":{"results":"$.items"}})).unwrap();
        let plan = normalize_pagination(&Api::default(), &operation, Some(&rule))
            .unwrap()
            .unwrap();
        assert_eq!(plan.inputs.len(), 2);
        operation.parameters[1].schema = Some(SchemaValue::new(SchemaKind::String));
        assert!(normalize_pagination(&Api::default(), &operation, Some(&rule)).is_err());
    }
}
