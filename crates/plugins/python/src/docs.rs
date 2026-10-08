use super::*;

pub(super) fn render_init(api: &Api, client_style: SdkClientStyle) -> String {
    let client_imports = if partition_python_errors(api) {
        vec!["ApiError".to_owned()]
    } else {
        std::iter::once("ApiError".to_owned())
            .chain(declared_error_class_names(api))
            .collect::<Vec<_>>()
    };
    let resource_import = if client_style == SdkClientStyle::Namespaced {
        "from .resources import *\n".to_owned()
    } else {
        String::new()
    };
    format!(
        "{NOTICE}\nfrom .client import Client\nfrom .presence import with_present_fields\nfrom .oauth import OAuthClientCredentials, AsyncOAuthClientCredentials\nfrom .multipart import MultipartBody, FilePart, JsonPart, RawJsonPart\nfrom .response_validation import ResponseDecodeError\nfrom .runtime import {}\nfrom .models import *\n{resource_import}",
        client_imports.join(", "),
    ) + if partition_python_errors(api) {
        "from .errors import *\n"
    } else {
        ""
    }
}

pub(super) fn render_readme(
    api: &Api,
    distribution: &str,
    module: &str,
    client_style: SdkClientStyle,
) -> String {
    let usage = api
        .operations
        .first()
        .map(|operation| {
            let name = python_identifier(&snake_case(&operation.id));
            let mut arguments = operation
                .parameters
                .iter()
                .filter(|item| item.required)
                .map(|item| {
                    let sample = item
                        .schema
                        .as_ref()
                        .map(python_example)
                        .unwrap_or_else(|| "None".into());
                    format!("{}={sample}", python_identifier(&item.name))
                })
                .collect::<Vec<_>>();
            if let Some(body) = &operation.request_body {
                if body.required {
                    let sample = body
                        .media_types
                        .first()
                        .and_then(|media| media.schema.as_ref())
                        .map(python_example)
                        .unwrap_or_else(|| "{}".into());
                    arguments.push(format!("body={sample}"));
                }
            }
            let method = if client_style == SdkClientStyle::Namespaced {
                let resource = resource_name(operation);
                let resources = resource_operations(api);
                let attribute = resource_attribute(&resource, &resources[&resource]);
                let method = resource_method_name(&name, &attribute);
                format!("{attribute}.{method}")
            } else {
                name
            };
            format!("client.{method}({})", arguments.join(", "))
        })
        .unwrap_or_else(|| "# This contract has no callable operations.".into());
    format!(
        "# {distribution}\n\nGenerated Python SDK for {}. This package uses the `{}` client style. Poolster can emit direct flat operations or resource namespaces; the direct methods remain available in either mode.\n\n```python\nfrom {module} import Client\n\nclient = Client(\"https://api.example.com\", api_key=\"…\")\n{}\n```\n",
        api.name,
        match client_style {
            SdkClientStyle::Flat => "flat",
            SdkClientStyle::Namespaced => "namespaced",
        },
        usage,
    ) + include_str!("../templates/middleware_readme.md")
}

pub(super) fn python_example(value: &SchemaValue) -> String {
    if let Some(first) = value.enum_values.first() {
        return python_literal(first);
    }
    match &value.kind {
        SchemaKind::String => "\"example\"".into(),
        SchemaKind::Integer => "1".into(),
        SchemaKind::Number => "1.0".into(),
        SchemaKind::Boolean => "True".into(),
        SchemaKind::Array { .. } => "[]".into(),
        SchemaKind::Object { .. } => "{}".into(),
        _ => "...".into(),
    }
}

pub(super) fn render_api_reference(api: &Api) -> String {
    let mut output = format!(
        "# {} Python API reference\n\nDirect methods remain available on both flat and namespaced clients.\n\n",
        api.name
    );
    for operation in &api.operations {
        let name = python_identifier(&snake_case(&operation.id));
        let (signature, _) = operation_signature(operation);
        let _ = writeln!(
            output,
            "## `{name}`\n\n`{} {}`\n\n```python\nClient.{name}(self{signature}) -> {}\n```\n",
            operation.method.as_str(),
            operation.path,
            response_type(operation)
        );
        for response in &operation.responses {
            let _ = writeln!(
                output,
                "- Response `{}`: {}",
                response.status,
                response
                    .description
                    .as_deref()
                    .unwrap_or("Declared API response")
            );
        }
        output.push('\n');
    }
    output
}

pub(super) fn render_style_guide(api: &Api, module: &str) -> String {
    format!(
        "# {} Python SDK styles\n\n`SdkClientStyle::Flat` exposes direct methods; `SdkClientStyle::Namespaced` adds resource facades. The namespaced attributes are initialized by `Client` and delegate to the same typed direct operations, so both styles can coexist during migration.\n\n{}",
        api.name,
        render_readme(api, module, module, SdkClientStyle::Namespaced)
    )
}
