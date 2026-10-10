use super::*;

/// Emits a self-contained ReDoc entry point and a small Redocly config.
#[derive(Default)]
pub struct ReDoc;

impl ReDoc {
    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        let spec = config.openapi_spec.as_deref().unwrap_or("openapi.yaml");
        let title = config.title.as_deref().unwrap_or(&api.name);
        let html = format!(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body><redoc spec-url=\"{}\"></redoc><script src=\"https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js\"></script></body></html>\n",
            html_escape(title),
            html_escape(spec)
        );
        let yaml = format!(
            "theme:\n  openapi:\n    htmlTemplate: ./redoc.html\n    theme:\n      typography:\n        fontSize: 15px\n# {} {}\n",
            api.name, api.version
        );
        Ok(vec![
            GeneratedFile::new(extra_output_path(config, "redoc", "redoc.html"), html)?,
            GeneratedFile::new(extra_output_path(config, "redoc", "redocly.yaml"), yaml)?,
        ])
    }
}

/// Emits a portable MCP tool manifest. A future server runtime can consume the
/// same manifest without making the codegen crate depend on an MCP SDK.
#[derive(Default)]
pub struct McpToolManifest;

impl McpToolManifest {
    pub fn generate(&self, api: &Api, config: &ArtifactOptions) -> Result<Vec<GeneratedFile>> {
        let tools = api.operations.iter().map(|operation| {
            let mut properties = serde_json::Map::from_iter([(
                "path".to_owned(),
                json!({ "type": "string", "default": operation.path }),
            )]);
            if operation.request_body.is_some() {
                properties.insert("body".to_owned(), json!({}));
            }
            json!({
                "name": operation_identifier(&operation.id, config),
                "description": format!("{} {}", operation.method.as_str(), operation.path),
                "inputSchema": {
                    "type": "object",
                    "properties": properties,
                    "required": if operation.request_body.as_ref().is_some_and(|body| body.required) { json!(["body"]) } else { json!([]) },
                },
            })
        }).collect::<Vec<_>>();
        let manifest = json!({ "name": api.name, "version": api.version, "tools": tools });
        Ok(vec![GeneratedFile::new(
            extra_output_path(config, "mcp", "tools.json"),
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?])
    }
}

pub(crate) fn extra_output_path(
    config: &ArtifactOptions,
    default_directory: &str,
    file: &str,
) -> String {
    if config.output_dir.is_some() {
        output_path(config, file)
    } else {
        format!("{default_directory}/{file}")
    }
}

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
