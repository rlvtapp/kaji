//! Scoped native GraphQL generation; HTTP contracts and JavaScript hooks remain separate.
use super::*;
mod incremental;
use poolster_core::{
    input::{InputOptions, InputProvider},
    native::GraphqlOperations,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    source: String,
    provider: String,
    operation_files: Vec<String>,
    #[serde(default)]
    import_roots: Vec<String>,
    #[serde(default)]
    incremental: bool,
    packages: Vec<SdkPackage>,
    #[serde(default)]
    scalars: BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    subscriptions: bool,
    #[serde(default)]
    rust_scalars: BTreeMap<String, rust::GraphqlScalarMapping>,
}

pub struct GenerateGraphql {
    request: String,
}
impl Task for GenerateGraphql {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        generate(&self.request).map_err(napi_anyhow)
    }
    fn resolve(&mut self, _: Env, output: String) -> Result<String> {
        Ok(output)
    }
}
fn generate(request: &str) -> AnyResult<String> {
    let mut request: Request = serde_json::from_str(request)?;
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    for mut package in std::mem::take(&mut request.packages) {
        output_options::apply(&mut package, "graphql")?;
        let subscriptions = package.subscriptions.unwrap_or(request.subscriptions);
        if ![
            "typescript",
            "rust",
            "go",
            "python",
            "php",
            "symfony",
            "java",
            "csharp",
            "ruby",
            "swift",
            "elixir",
            "postman",
            "rust-cli",
            "typescript-cli",
        ]
        .contains(&package.language.as_str())
        {
            bail!("Unsupported GraphQL output language");
        }
        if package.client_name.is_some()
            || package.jobs.is_some()
            || (package.language != "typescript" && !package.plugins.is_empty())
        {
            bail!("GraphQL does not support HTTP SDK options or auxiliaries");
        }
        if package
            .transport
            .as_deref()
            .is_some_and(|value| value != "fetch")
        {
            bail!("GraphQL HTTP transport must be fetch");
        }
        if request.incremental {
            profiles = incremental::append(profiles, &request, package, registry.clone())?;
            continue;
        }
        let input =
            InputProvider::<GraphqlOperations>::new(registry.clone(), "graphql", &request.source)
                .using(&request.provider)
                .with_options(InputOptions {
                    operation_files: request.operation_files.iter().map(Into::into).collect(),
                    import_roots: request.import_roots.iter().map(Into::into).collect(),
                    ..Default::default()
                });
        if package.raw.unwrap_or(false) && package.style.is_some() {
            bail!("GraphQL raw and style are mutually exclusive");
        }
        profiles = outputs::append(profiles, &request, package, input, subscriptions)?;
    }
    let tree = poolster::generate_native(profiles)?;
    let files: Vec<_> = tree
        .into_owned_files()
        .map(|(file, preserve_existing, owner)| OutputFile {
            path: file.path.to_string_lossy().into_owned(),
            contents: file.contents,
            preserve_existing,
            owner,
        })
        .collect();
    Ok(serde_json::to_string(&files)?)
}
#[napi]
pub fn generate_graphql(request: String) -> AsyncTask<GenerateGraphql> {
    AsyncTask::new(GenerateGraphql { request })
}

#[path = "graphql_tools.rs"]
mod tools;

mod outputs;
mod typescript;
