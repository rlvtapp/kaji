//! Scoped native GraphQL generation; HTTP contracts and JavaScript hooks remain separate.
use super::*;
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
    packages: Vec<SdkPackage>,
    #[serde(default)]
    scalars: BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    subscriptions: bool,
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
    let request: Request = serde_json::from_str(request)?;
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    for package in request.packages {
        if !["typescript", "rust"].contains(&package.language.as_str()) {
            bail!("GraphQL requires TypeScript or Rust output");
        }
        if package.style.is_some()
            || package.client_name.is_some()
            || package.raw
            || package.jobs.is_some()
            || !package.plugins.is_empty()
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
        let input =
            InputProvider::<GraphqlOperations>::new(registry.clone(), "graphql", &request.source)
                .using(&request.provider)
                .with_options(InputOptions {
                    operation_files: request.operation_files.iter().map(Into::into).collect(),
                    ..Default::default()
                });
        if package.language == "rust" {
            if !request.scalars.is_empty() || request.subscriptions || package.transport.is_some() {
                bail!(
                    "Rust GraphQL does not support scalar mappings, subscriptions, or transport options"
                );
            }
            let common = package_common(&package)?;
            let mut target = rust::package(package.path)
                .common(common)
                .with(rust::graphql(Some(input.handle())))
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        let mut generator = ts::graphql(Some(input.handle())).scalars(request.scalars.clone());
        if request.subscriptions {
            generator = generator.subscriptions();
        }
        let common = package_common(&package)?;
        let mut target = ts::package(package.path)
            .common(common)
            .with(input)
            .with(generator);
        if let Some(name) = package.name {
            target = target.name(name);
        }
        profiles = profiles.package(target);
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
