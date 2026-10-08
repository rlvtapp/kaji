//! Node-API boundary for Kaji's normalized contract and safe output tree.

use std::path::Path;

use anyhow::{Context, Result as AnyResult, bail};
use kaji::prelude::*;
use kaji::{csharp, elixir, go, java, php, python, ruby, rust, swift, ts};
use kaji_core::adapter::OpenApiSidecar;
use kaji_core::{Api, GeneratedFile, GeneratedTree, SecuritySchemeCatalog};
use kaji_inputs::default_registry;
use napi::bindgen_prelude::{AsyncTask, Task};
use napi::{Env, Error, Result, Status};
use napi_derive::napi;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contract {
    api: Api,
    security_schemes: SecuritySchemeCatalog,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SdkPackage {
    language: String,
    path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    raw: bool,
    #[serde(default)]
    jobs: Option<usize>,
    #[serde(default)]
    plugins: Vec<NativePlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativePlugin {
    name: String,
    #[serde(default)]
    output: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OutputFile {
    path: String,
    contents: String,
    #[serde(default)]
    preserve_existing: bool,
    #[serde(default)]
    owner: Option<String>,
}

fn napi_error(error: impl std::fmt::Display) -> Error {
    Error::new(Status::GenericFailure, error.to_string())
}

fn napi_anyhow(error: anyhow::Error) -> Error {
    napi_error(format!("{error:#}"))
}

fn package_common(package: &SdkPackage) -> AnyResult<Common> {
    let mut common = Common::default();
    if let Some(style) = package.style.as_deref() {
        common = common.client_style(match style {
            "flat" => SdkClientStyle::Flat,
            "namespaced" => SdkClientStyle::Namespaced,
            _ => bail!("SDK style must be flat or namespaced"),
        });
    }
    if let Some(version) = &package.version {
        common = common.package_version(version);
    }
    Ok(common)
}

fn validate_options(package: &SdkPackage) -> AnyResult<()> {
    if package.path.is_empty() {
        bail!("SDK package path must not be empty");
    }
    if package.language != "typescript"
        && (package.transport.is_some() || package.client_name.is_some() || package.raw)
    {
        bail!("transport, clientName and raw are TypeScript-only SDK options");
    }
    if package.language != "go" && package.jobs.is_some() {
        bail!("jobs is a Go-only SDK option");
    }
    if package.jobs == Some(0) {
        bail!("jobs must be at least 1");
    }
    if package.language != "typescript" && !package.plugins.is_empty() {
        bail!("registered native auxiliaries currently require a TypeScript package");
    }
    Ok(())
}

fn profiles(packages: Vec<SdkPackage>) -> AnyResult<ProfileSet> {
    let mut profiles = ProfileSet::new(".");
    for package in packages {
        validate_options(&package)?;
        let common = package_common(&package)?;
        let path = package.path.clone();
        match package.language.as_str() {
            "typescript" => {
                let mut sdk = ts::sdk();
                sdk = match package.transport.as_deref().unwrap_or("fetch") {
                    "fetch" => sdk.fetch(),
                    "axios" => sdk.axios(),
                    _ => bail!("TypeScript transport must be fetch or axios"),
                };
                if package.raw {
                    sdk = sdk.raw();
                }
                if let Some(client_name) = &package.client_name {
                    sdk = sdk.client_name(client_name);
                }
                let mut target = ts::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                let mut target = target.with(sdk);
                for plugin in package.plugins {
                    target = match plugin.name.as_str() {
                        "zod" | "faker" | "msw" | "cypress" => {
                            let mut auxiliary = match plugin.name.as_str() {
                                "zod" => ts::composition::zod(),
                                "faker" => ts::composition::faker(),
                                "msw" => ts::composition::msw(),
                                _ => ts::composition::cypress(),
                            };
                            if let Some(output) = plugin.output {
                                auxiliary = auxiliary.output(output);
                            }
                            target.with(auxiliary)
                        }
                        "react-query" | "vue-query" | "swr" => {
                            let mut query = match plugin.name.as_str() {
                                "react-query" => ts::composition::react_query(),
                                "vue-query" => ts::composition::vue_query(),
                                _ => ts::composition::swr(),
                            };
                            if let Some(output) = plugin.output {
                                query = query.output(output);
                            }
                            target.with(query)
                        }
                        _ => bail!(
                            "native plugin {:?} is not registered in this addon",
                            plugin.name
                        ),
                    };
                }
                profiles = profiles.package(target);
            }
            "rust" => {
                let mut target = rust::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(rust::sdk()));
            }
            "go" => {
                let mut target = go::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                let mut sdk = go::sdk();
                if let Some(jobs) = package.jobs {
                    sdk = sdk.jobs(jobs);
                }
                profiles = profiles.package(target.with(sdk));
            }
            "python" => {
                let mut target = python::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(python::sdk()));
            }
            "php" => {
                let mut target = php::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(php::sdk()));
            }
            "java" => {
                let mut target = java::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(java::sdk()));
            }
            "csharp" => {
                let mut target = csharp::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(csharp::sdk()));
            }
            "elixir" => {
                let mut target = elixir::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(elixir::sdk()));
            }
            "ruby" => {
                let mut target = ruby::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(ruby::sdk()));
            }
            "swift" => {
                let mut target = swift::package(path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(swift::sdk()));
            }
            language => bail!("unsupported SDK language {language:?}"),
        }
    }
    Ok(profiles)
}

#[napi]
pub fn available_native_plugins() -> Vec<String> {
    [
        "typescript/zod",
        "typescript/faker",
        "typescript/msw",
        "typescript/cypress",
        "typescript/react-query",
        "typescript/vue-query",
        "typescript/swr",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[napi]
pub fn available_input_plugins() -> Result<String> {
    let registry = default_registry().map_err(napi_anyhow)?;
    serde_json::to_string(&registry.plugins()).map_err(napi_error)
}

pub struct InspectInput {
    format: String,
    provider: Option<String>,
    source: String,
}

impl Task for InspectInput {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let registry = default_registry().map_err(napi_anyhow)?;
        let loaded = registry
            .load(
                &self.format,
                self.provider.as_deref(),
                Path::new(&self.source),
            )
            .map_err(napi_anyhow)?;
        serde_json::to_string(&serde_json::json!({
            "provider": loaded.provider,
            "source": loaded.source,
            "summary": loaded.contract.summary,
            "diagnostics": loaded.contract.diagnostics,
        }))
        .map_err(napi_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn inspect_input(
    format: String,
    provider: Option<String>,
    source: String,
) -> AsyncTask<InspectInput> {
    AsyncTask::new(InspectInput {
        format,
        provider,
        source,
    })
}

pub struct LoadContract {
    artifacts: String,
    name: String,
    version: String,
}

impl Task for LoadContract {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let adapted = OpenApiSidecar::new(&self.artifacts, &self.name, &self.version)
            .load()
            .map_err(napi_error)?;
        serde_json::to_string(&Contract {
            api: adapted.api,
            security_schemes: adapted.security_schemes,
        })
        .map_err(napi_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn load_contract(artifacts: String, name: String, version: String) -> AsyncTask<LoadContract> {
    AsyncTask::new(LoadContract {
        artifacts,
        name,
        version,
    })
}

pub struct GenerateSdk {
    contract: String,
    packages: String,
}

impl Task for GenerateSdk {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let contract: Contract = serde_json::from_str(&self.contract).map_err(napi_error)?;
        let packages: Vec<SdkPackage> = serde_json::from_str(&self.packages).map_err(napi_error)?;
        if packages.is_empty() {
            return Ok("[]".to_owned());
        }
        let tree = kaji::generate_with_security_catalog(
            &contract.api,
            profiles(packages).map_err(napi_error)?,
            Some(&contract.security_schemes),
        )
        .map_err(napi_anyhow)?;
        let files = tree
            .into_owned_files()
            .map(|(file, preserve_existing, owner)| OutputFile {
                path: file.path.to_string_lossy().into_owned(),
                contents: file.contents,
                preserve_existing,
                owner,
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&files).map_err(napi_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn generate_sdk(contract: String, packages: String) -> AsyncTask<GenerateSdk> {
    AsyncTask::new(GenerateSdk { contract, packages })
}

pub struct Materialize {
    files: String,
    output: String,
    write: bool,
}

impl Task for Materialize {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let files: Vec<OutputFile> = serde_json::from_str(&self.files).map_err(napi_error)?;
        let mut tree = GeneratedTree::default();
        for file in files {
            let path = file.path.clone();
            let generated = GeneratedFile::new(&path, file.contents).map_err(napi_error)?;
            if file.preserve_existing {
                tree.insert_custom(generated).map_err(napi_error)?;
            } else {
                tree.insert(generated).map_err(napi_error)?;
            }
            if let Some(owner) = file.owner {
                tree.set_owner(path, owner).map_err(napi_error)?;
            }
        }
        let output = Path::new(&self.output);
        let changes = tree.check(output).map_err(napi_error)?;
        if self.write {
            tree.write_to(output)
                .with_context(|| format!("write generated output to {}", output.display()))
                .map_err(napi_anyhow)?;
        }
        serde_json::to_string(&changes).map_err(napi_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn materialize(files: String, output: String, write: bool) -> AsyncTask<Materialize> {
    AsyncTask::new(Materialize {
        files,
        output,
        write,
    })
}
