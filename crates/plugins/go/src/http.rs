//! HTTP source plan, package documentation and pagination lowering.
use super::*;

pub(super) fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    client_style: SdkClientStyle,
    jobs: usize,
) -> Result<GeneratedTree> {
    let prepared = symbols::prepare(api);
    let api = prepared.as_ref();
    let output_dir = normalized_output_dir(output_dir)?;
    let composed = composition::models(api);
    let schemas = composed.as_deref().unwrap_or(&api.schemas);
    let package = package_name
        .map(go_package_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| go_package_name(&api.name));
    if package.is_empty() {
        bail!("a Go package name could not be derived from the API name")
    }

    let module = go_module_name(package_name.unwrap_or(&api.name));
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(
        output_path(&output_dir, "go.mod"),
        render_go_mod(&module),
    )?)?;
    layout::generate(
        &mut tree,
        api,
        schemas,
        &output_dir,
        &package,
        client_style,
        jobs,
    )?;
    tree.insert(GeneratedFile::new(
        output_path(&output_dir, "README.md"),
        render_readme(api, client_style),
    )?)?;
    if api.operations.iter().any(has_multipart) {
        tree.insert(GeneratedFile::new(
            output_path(&output_dir, "MULTIPART.md"),
            include_str!("../templates/multipart.md.tmpl"),
        )?)?;
    }
    if client_style == SdkClientStyle::Namespaced {
        tree.insert(GeneratedFile::new(
            output_path(&output_dir, "STYLE_GUIDE.md"),
            render_style_guide(api),
        )?)?;
    }
    Ok(tree)
}

pub(super) fn normalized_output_dir(output_dir: &str) -> Result<String> {
    let output_dir = output_dir.trim_matches('/');
    if output_dir.is_empty() || output_dir == "." {
        return Ok(String::new());
    }
    if output_dir.split('/').any(|component| component == "..") {
        bail!("Go SDK output directory cannot contain parent-directory components")
    }
    Ok(output_dir.to_owned())
}

pub(super) fn output_path(output_dir: &str, file: &str) -> String {
    if output_dir.is_empty() {
        file.to_owned()
    } else {
        format!("{output_dir}/{file}")
    }
}

pub(super) fn render_go_mod(module: &str) -> String {
    format!("module {module}\n\ngo 1.22\n")
}
#[derive(Clone, Debug)]
pub(super) enum GoResponseKind {
    None,
    Json(String),
    Text,
    Binary,
    EventStream,
}

pub(super) const PAGINATION_RUNTIME: &str = include_str!("../templates/pagination.go.tmpl");

#[derive(Clone, Debug)]
pub(super) enum GoCursorLocation {
    Parameter {
        name: String,
        optional: bool,
    },
    /// Request-body cursor pagers are deliberately limited to a named object
    /// schema with an optional string field. That lets us clone and update a
    /// typed request without mutating the caller's value or guessing JSON.
    RequestBody {
        body_type: String,
        field_name: String,
        body_required: bool,
    },
}

#[derive(Clone, Debug)]
pub(super) struct GoCursorPagination {
    pub(super) location: GoCursorLocation,
    pub(super) next_cursor_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct GoUrlPagination {
    pub(super) next_url_path: String,
}

#[derive(Clone, Debug)]
pub(super) struct GoOffsetPagination {
    pub(super) offset_name: String,
    pub(super) limit_name: String,
    pub(super) results_path: String,
}

pub(super) fn offset_pagination(api: &Api, operation: &Operation) -> Option<GoOffsetPagination> {
    // Legacy offset rendering accepts opaque response envelopes.
    let mut declaration = operation.clone();
    declaration.responses.clear();
    let plan = poolster_core::pagination::normalize_pagination(api, &declaration, None)
        .ok()
        .flatten()?;
    if plan.kind != poolster_core::pagination::PaginationKind::OffsetLimit {
        return None;
    }
    let find = |role: &str| {
        let input = plan.inputs.iter().find(|input| {
            input.role == role && input.location != "requestBody" && !input.required
        })?;
        let parameter = operation
            .parameters
            .iter()
            .find(|parameter| parameter.name == input.name)?;
        Some(parameter_field_name(operation, parameter))
    };
    Some(GoOffsetPagination {
        offset_name: find("offset")?,
        limit_name: find("limit")?,
        results_path: native_selector(&plan.results?)?,
    })
}

pub(super) fn cursor_pagination(api: &Api, operation: &Operation) -> Option<GoCursorPagination> {
    let plan = poolster_core::pagination::normalize_pagination(api, operation, None)
        .ok()
        .flatten()?;
    if plan.kind != poolster_core::pagination::PaginationKind::Cursor {
        return None;
    }
    let input = plan.inputs.iter().find(|input| input.role == "cursor")?;
    if input.value_kind != poolster_core::pagination::PaginationValueKind::String {
        return None;
    }
    let cursor_name = input.name.as_str();
    let next_cursor_path = native_selector(&plan.continuation?)?;
    let location = if input.location == "requestBody" {
        body_cursor_location(api, operation, cursor_name)?
    } else {
        let parameter = operation.parameters.iter().find(|parameter| {
            parameter.name == cursor_name
                && matches!(parameter.location.as_str(), "query" | "header" | "path")
                && matches!(
                    parameter.schema.as_ref().map(|schema| &schema.kind),
                    Some(SchemaKind::String)
                )
        })?;
        // OpenAPI path parameters are always required. Do not turn an invalid
        // optional path parameter into a paginator API.
        if parameter.location == "path" && !parameter.required {
            return None;
        }
        GoCursorLocation::Parameter {
            name: parameter_field_name(operation, parameter),
            optional: !parameter.required,
        }
    };
    Some(GoCursorPagination {
        location,
        next_cursor_path,
    })
}

pub(super) fn native_selector(selector: &poolster_core::pagination::Selector) -> Option<String> {
    use poolster_core::pagination::SelectorSegment;
    if selector.expression.starts_with('/') {
        return Some(selector.expression.clone());
    }
    let mut output = String::from("$");
    for segment in &selector.segments {
        match segment {
            SelectorSegment::Field(name) if !name.contains(['.', '[', ']']) => {
                output.push('.');
                output.push_str(name);
            }
            SelectorSegment::Index(index) => output.push_str(&format!("[{index}]")),
            _ => return None,
        }
    }
    Some(output)
}

pub(super) fn body_cursor_location(
    api: &Api,
    operation: &Operation,
    cursor_name: &str,
) -> Option<GoCursorLocation> {
    let body = operation.request_body.as_ref()?;
    let schema = body
        .media_types
        .iter()
        .find(|media| media.content_type.contains("json"))
        .or_else(|| body.media_types.first())?
        .schema
        .as_ref()?;
    let SchemaKind::Reference { reference } = &schema.kind else {
        return None;
    };
    let body_name = reference.rsplit('/').next()?;
    let body_schema = api.schemas.iter().find(|schema| schema.name == body_name)?;
    let SchemaKind::Object { fields, .. } = &body_schema.value.kind else {
        return None;
    };
    let field = fields.iter().find(|field| {
        field.name == cursor_name
            && !field.required
            && matches!(field.value.kind, SchemaKind::String)
    })?;
    Some(GoCursorLocation::RequestBody {
        body_type: go_type_name(body_name),
        field_name: model_field_names(fields)[&field.name].clone(),
        body_required: body.required,
    })
}

pub(super) fn url_pagination(api: &Api, operation: &Operation) -> Option<GoUrlPagination> {
    let plan = poolster_core::pagination::normalize_pagination(api, operation, None)
        .ok()
        .flatten()?;
    if plan.kind != poolster_core::pagination::PaginationKind::Url {
        return None;
    }
    Some(GoUrlPagination {
        next_url_path: native_selector(&plan.continuation?)?,
    })
}

pub(super) fn sequential_media(content_type: &str) -> bool {
    let media = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        media.as_str(),
        "application/x-ndjson"
            | "application/ndjson"
            | "application/jsonl"
            | "application/json-seq"
    ) || media.ends_with("+json-seq")
}
pub(super) fn render_readme(api: &Api, style: SdkClientStyle) -> String {
    let usage = match style {
        SdkClientStyle::Flat => "client.GetContact(ctx, input)",
        SdkClientStyle::Namespaced => "client.Contacts.Get(ctx, input)",
    };
    format!(
        "# {} Go SDK\n\nGenerated by Poolster. This package exposes the `{}` client style. Direct operations remain available in either mode. Generate with `SdkClientStyle::Flat` for `client.GetContact(...)`, or `SdkClientStyle::Namespaced` for resource-first calls such as `client.Contacts.Get(...)`.\n\n```go\nclient, err := NewClient(ClientConfig{{BaseURL: \"https://api.example.com\"}})\nif err != nil {{ panic(err) }}\n_ = {}\n```\n",
        api.name, usage, usage
    ) + include_str!("../templates/middleware.md.tmpl")
        + &page_pagination::documentation(api)
}

pub(super) fn render_style_guide(api: &Api) -> String {
    format!(
        "# {} Go SDK styles\n\nPoolster supports two stable client surfaces:\n\n- `SdkClientStyle::Flat`: `client.GetContact(ctx, input)`\n- `SdkClientStyle::Namespaced`: `client.Contacts.Get(ctx, input)`\n\nThe namespaced surface is initialized by `NewClient` and delegates to the same typed operation methods, so both styles can coexist during migration.\n\n```go\nclient, err := NewClient(ClientConfig{{BaseURL: \"https://api.example.com\"}})\nif err != nil {{ panic(err) }}\ncontact, err := client.Contacts.Get(ctx, &GetContactRequest{{}})\n_ = contact\n_ = err\n```\n",
        api.name
    )
}
