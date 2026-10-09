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
    let request: Request = serde_json::from_str(request)?;
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    for mut package in request.packages {
        output_options::apply(&mut package, "graphql")?;
        if ![
            "typescript",
            "rust",
            "go",
            "python",
            "php",
            "java",
            "csharp",
            "dotnet",
            "ruby",
            "swift",
            "elixir",
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
        let input =
            InputProvider::<GraphqlOperations>::new(registry.clone(), "graphql", &request.source)
                .using(&request.provider)
                .with_options(InputOptions {
                    operation_files: request.operation_files.iter().map(Into::into).collect(),
                    ..Default::default()
                });
        if package.raw.unwrap_or(false) && package.style.is_some() {
            bail!("GraphQL raw and style are mutually exclusive");
        }
        let style = package.style.as_deref().unwrap_or("idiomatic");
        if !["raw", "flat", "idiomatic", "namespaced", "grouped"].contains(&style) {
            bail!("GraphQL style must be raw, flat, idiomatic, or namespaced");
        }
        let common = Common {
            package_version: package.version.clone(),
            ..Default::default()
        };
        if package.language == "rust" {
            if request.subscriptions || package.transport.is_some() {
                bail!("Rust GraphQL does not support subscriptions or transport options");
            }
            let mut mappings = request.rust_scalars.clone();
            for (name, mapping) in &package.scalars {
                let mapping = rust::GraphqlScalarMapping::new(&mapping.input, &mapping.output);
                if mappings.get(name).is_some_and(|old| old != &mapping) {
                    bail!("conflicting Rust scalar mapping {name}");
                }
                mappings.insert(name.clone(), mapping);
            }
            let mut generator = rust::graphql(Some(input.handle()))
                .scalars(mappings)
                .groups(package.groups.clone());
            generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = rust::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "go" {
            if request.subscriptions || package.transport.is_some() {
                bail!("go GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("go GraphQL custom scalar mappings are not supported");
            }
            let generator = go::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = go::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "python" {
            if request.subscriptions || package.transport.is_some() {
                bail!("python GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("python GraphQL custom scalar mappings are not supported");
            }
            let generator = python::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = python::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "php" {
            if request.subscriptions || package.transport.is_some() {
                bail!("php GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("php GraphQL custom scalar mappings are not supported");
            }
            let generator = php::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = php::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "java" {
            if request.subscriptions || package.transport.is_some() {
                bail!("java GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("java GraphQL custom scalar mappings are not supported");
            }
            let generator = java::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = java::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "csharp" {
            if request.subscriptions || package.transport.is_some() {
                bail!("csharp GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("csharp GraphQL custom scalar mappings are not supported");
            }
            let generator = csharp::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = csharp::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "dotnet" {
            if request.subscriptions || package.transport.is_some() {
                bail!("dotnet GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("dotnet GraphQL custom scalar mappings are not supported");
            }
            let generator = dotnet::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = dotnet::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "ruby" {
            if request.subscriptions || package.transport.is_some() {
                bail!("ruby GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("ruby GraphQL custom scalar mappings are not supported");
            }
            let generator = ruby::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = ruby::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "swift" {
            if request.subscriptions || package.transport.is_some() {
                bail!("swift GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("swift GraphQL custom scalar mappings are not supported");
            }
            let generator = swift::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = swift::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        if package.language == "elixir" {
            if request.subscriptions || package.transport.is_some() {
                bail!("elixir GraphQL does not support subscriptions or transport options");
            }
            if !package.scalars.is_empty() {
                bail!("elixir GraphQL custom scalar mappings are not supported");
            }
            let generator = elixir::graphql(Some(input.handle())).groups(package.groups.clone());
            let generator = if package.raw.unwrap_or(false) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = elixir::package(package.path)
                .common(common)
                .with(generator)
                .with(input);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
            continue;
        }
        let mut mappings = request.scalars.clone();
        for (name, mapping) in &package.scalars {
            if mappings.get(name).is_some_and(|old| old != mapping) {
                bail!("conflicting TypeScript scalar mapping {name}");
            }
            mappings.insert(name.clone(), mapping.clone());
        }
        let mut generator = ts::graphql(Some(input.handle()))
            .scalars(mappings)
            .groups(package.groups.clone());
        generator = if package.raw.unwrap_or(false) || style == "raw" {
            generator.raw()
        } else if style == "flat" {
            generator.flat()
        } else {
            generator.idiomatic()
        };
        if request.subscriptions {
            generator = generator.subscriptions();
        }
        let client = generator.handle();
        let mut target = ts::package(package.path)
            .common(common)
            .with(input)
            .with(generator);
        if let Some(name) = package.name {
            target = target.name(name);
        }
        let target = graphql_addons::attach(target, package.plugins, client)?;
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
