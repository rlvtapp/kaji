//! Select native contract consumers without adapting protocols into HTTP APIs.
use super::*;
mod incremental;
use poolster_core::{
    engine::Contract,
    input::{InputProvider, InputRegistry},
    native::{
        GraphqlOperations, events::EventOperations, rpc::RpcContract, workflows::WorkflowOperations,
    },
};
use std::sync::Arc;

pub(super) fn pipeline(format: &str) -> Option<(&'static str, &'static str)> {
    match format {
        "graphql" => Some(("typescript", "graphql")),
        "arazzo" => Some(("typescript", "workflow")),
        "asyncapi" => Some(("typescript", "asyncapi")),
        "protobuf" => Some(("go", "grpc")),
        _ => None,
    }
}
pub(super) fn language_compatible(format: &str, language: &str) -> bool {
    (format == "graphql"
        && [
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
            "postman",
            "rust-cli",
            "typescript-cli",
        ]
        .contains(&language))
        || pipeline(format).is_some_and(|(target, _)| language == target)
}
pub(super) fn compatible(format: &str, package: &PackageConfig) -> bool {
    if format == "graphql" && ["rust-cli", "typescript-cli"].contains(&package.language.as_str()) {
        return package.plugins.len() == 1
            && matches!(package.plugins[0].name.as_str(), "cli" | "graphql");
    }
    if format == "graphql" && package.language == "postman" {
        return package
            .plugins
            .iter()
            .filter(|p| matches!(p.name.as_str(), "collection" | "graphql" | "sdk"))
            .count()
            == 1
            && package
                .plugins
                .iter()
                .filter(|p| p.name == "environment")
                .count()
                <= 1
            && package.plugins.iter().all(|p| {
                matches!(
                    p.name.as_str(),
                    "collection" | "graphql" | "sdk" | "environment"
                )
            });
    }
    if format == "graphql"
        && [
            "rust", "go", "python", "php", "java", "csharp", "dotnet", "ruby", "swift", "elixir",
        ]
        .contains(&package.language.as_str())
    {
        return package.plugins.len() == 1
            && matches!(package.plugins[0].name.as_str(), "graphql" | "sdk");
    }
    if format == "graphql" && package.language == "typescript" {
        let mains = package
            .plugins
            .iter()
            .filter(|p| matches!(p.name.as_str(), "graphql" | "sdk"))
            .count();
        return mains == 1
            && package.plugins.iter().all(|p| {
                matches!(
                    p.name.as_str(),
                    "graphql"
                        | "sdk"
                        | "zod"
                        | "faker"
                        | "msw"
                        | "cypress"
                        | "react-query"
                        | "vue-query"
                        | "swr"
                )
            });
    }
    pipeline(format).is_some_and(|(language, plugin)| {
        package.language == language
            && package.plugins.len() == 1
            && package.plugins[0].name == plugin
    })
}
pub(super) fn input_language_compatible(input: &NativeInputConfig, language: &str) -> bool {
    language_compatible(&input.format, language)
        && !(input.options.graphql_incremental
            && ["postman", "rust-cli", "typescript-cli"].contains(&language))
}
pub(super) fn input_compatible(input: &NativeInputConfig, package: &PackageConfig) -> bool {
    compatible(&input.format, package)
        && input_language_compatible(input, &package.language)
        && (!input.options.graphql_incremental || package.plugins.len() == 1)
}
fn provider<C: Contract>(
    input: &NativeInputConfig,
    registry: Arc<InputRegistry>,
) -> InputProvider<C> {
    let provider = InputProvider::<C>::new(registry, &input.format, &input.path)
        .with_options(input.options.clone());
    if let Some(id) = &input.provider {
        provider.using(id)
    } else {
        provider
    }
}
pub(super) fn build(options: &Generate, input: &NativeInputConfig) -> Result<ProfileSet> {
    let registry = Arc::new(poolster_inputs::default_registry()?);
    let mut profiles = ProfileSet::new(".");
    let mut append = |language: &str,
                      path: &str,
                      name: Option<&str>,
                      version: Option<&str>,
                      plugin: Option<&PluginConfig>,
                      package_config: Option<&PackageConfig>|
     -> Result<()> {
        let explicit_style = plugin
            .and_then(|p| p.style.as_deref())
            .or(package_config.and_then(|p| p.client_style.as_deref()));
        let raw = plugin.and_then(|p| p.raw).unwrap_or(false)
            || package_config.is_some_and(|p| p.sdk_raw)
            || options.raw;
        ensure!(
            !(raw && explicit_style.is_some()),
            "GraphQL raw and style are mutually exclusive"
        );
        let style = explicit_style.unwrap_or(if options.style == SdkClientStyle::Flat {
            "flat"
        } else {
            "idiomatic"
        });
        ensure!(
            ["raw", "flat", "idiomatic", "namespaced", "grouped"].contains(&style),
            "GraphQL style must be raw, flat, idiomatic, or namespaced"
        );
        let common = Common {
            package_version: Some(version.unwrap_or(&options.version).to_owned()),
            ..Default::default()
        };
        let old = std::mem::replace(&mut profiles, ProfileSet::new("."));
        profiles = if input.format == "graphql" && input.options.graphql_incremental {
            incremental::append(
                old,
                input,
                registry.clone(),
                language,
                path,
                name,
                common,
                plugin,
                raw,
                style,
            )?
        } else if input.format == "graphql"
            && ["postman", "rust-cli", "typescript-cli"].contains(&language)
        {
            append_graphql_tools(
                old,
                input,
                registry.clone(),
                language,
                path,
                name,
                common,
                plugin,
                package_config,
            )?
        } else if input.format == "graphql" && language == "rust" {
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = rust::graphql(Some(input_provider.handle())).scalars(
                plugin
                    .map(|p| {
                        p.scalars
                            .iter()
                            .map(|(name, m)| {
                                (
                                    name.clone(),
                                    rust::GraphqlScalarMapping::new(&m.input, &m.output),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            );
            let generator = generator.groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = rust::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "go" {
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = go::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default())
                .scalars(
                    plugin
                        .map(|p| {
                            p.scalars
                                .iter()
                                .map(|(name, m)| {
                                    (
                                        name.clone(),
                                        go::GraphqlScalarMapping::new(&m.input, &m.output),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = go::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "python" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "python GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = python::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = python::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "php" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "php GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = php::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = php::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "java" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "java GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = java::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions(true)
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = java::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "csharp" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "csharp GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = csharp::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions(true)
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = csharp::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "dotnet" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "dotnet GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = dotnet::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions(true)
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = dotnet::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "ruby" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "ruby GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = ruby::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = ruby::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "swift" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "swift GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = swift::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions(true)
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = swift::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "graphql" && language == "elixir" {
            ensure!(
                plugin.is_none_or(|p| p.scalars.is_empty()),
                "elixir GraphQL custom scalar mappings are not supported"
            );
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let generator = elixir::graphql(Some(input_provider.handle()))
                .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
            let generator = if plugin.and_then(|p| p.subscriptions).unwrap_or(false) {
                generator.subscriptions()
            } else {
                generator
            };
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut package = elixir::package(path)
                .common(common)
                .with(generator)
                .with(input_provider);
            if let Some(name) = name {
                package = package.name(name);
            }
            old.package(package)
        } else if input.format == "protobuf" {
            let output = plugin
                .map(|plugin| NativeOutputConfig {
                    module: plugin.module.clone(),
                    toolchain: plugin.toolchain.clone().unwrap_or_default(),
                    go_packages: plugin.go_packages.clone(),
                })
                .unwrap_or_else(|| options.native_output.clone());
            let module = output.module.context("Protobuf Go generation requires a module import path: grpc plugin.module or --module")?;
            let input_provider = provider::<RpcContract>(input, registry.clone());
            let mut output_plugin = go::grpc(module)
                .input(input_provider.handle())
                .toolchain(output.toolchain.into_tools());
            for (file, mapping) in output.go_packages {
                output_plugin = output_plugin.go_package(file, mapping);
            }
            old.package(
                go::package(path)
                    .common(common)
                    .with(input_provider)
                    .with(output_plugin),
            )
        } else {
            let package = match input.format.as_str() {
                "graphql" => {
                    let input_provider = provider::<GraphqlOperations>(input, registry.clone());
                    let output_plugin = ts::graphql(Some(input_provider.handle()))
                        .scalars(plugin.map(|p| p.scalars.clone()).unwrap_or_default());
                    let output_plugin =
                        output_plugin.groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
                    let output_plugin = if raw || style == "raw" {
                        output_plugin.raw()
                    } else if style == "flat" {
                        output_plugin.flat()
                    } else {
                        output_plugin.idiomatic()
                    };
                    let output_plugin = if plugin
                        .and_then(|plugin| plugin.subscriptions)
                        .unwrap_or(false)
                    {
                        output_plugin.subscriptions()
                    } else {
                        output_plugin
                    };
                    let client = output_plugin.handle();
                    native_graphql_addons::attach(
                        ts::package(path).with(input_provider).with(output_plugin),
                        package_config
                            .map(|p| p.plugins.as_slice())
                            .unwrap_or_default(),
                        client,
                    )?
                }
                "arazzo" => {
                    let input_provider = provider::<WorkflowOperations>(input, registry.clone());
                    ts::package(path)
                        .with(ts::workflow(Some(input_provider.handle())))
                        .with(input_provider)
                }
                "asyncapi" => {
                    let input_provider = provider::<EventOperations>(input, registry.clone());
                    ts::package(path)
                        .with(ts::asyncapi(Some(input_provider.handle())))
                        .with(input_provider)
                }
                _ => bail!("unsupported native pipeline"),
            };
            let package = if let Some(name) = name {
                package.name(name)
            } else {
                package
            };
            old.package(package.common(common))
        };
        Ok(())
    };
    if let Some(packages) = &options.config_packages {
        for package in packages
            .iter()
            .filter(|package| input_compatible(input, package))
        {
            append(
                &package.language,
                &package.path,
                package.name.as_deref(),
                package.version.as_deref(),
                package.plugins.iter().find(|p| {
                    matches!(
                        p.name.as_str(),
                        "graphql" | "sdk" | "workflow" | "asyncapi" | "grpc" | "cli" | "collection"
                    )
                }),
                Some(package),
            )?;
        }
    } else {
        for language in options
            .languages
            .iter()
            .filter(|language| input_language_compatible(input, language))
        {
            append(language, language, None, None, None, None)?;
        }
    }
    Ok(profiles)
}
pub(super) fn validate_recipe(document: &serde_json::Value, config: &ProjectConfig) -> Result<()> {
    let input = config
        .input
        .as_ref()
        .context("native recipe input missing")?;
    ensure!(
        document.get("openapi").is_none(),
        "set exactly one of input or openapi"
    );
    ensure!(
        document.get("defaults").is_none_or(|value| value
            .as_object()
            .is_some_and(|object| object.is_empty()
                || (input.format == "graphql" && object.keys().all(|key| key == "client_style")))),
        "native recipes do not support HTTP defaults"
    );
    for (raw, package) in document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .zip(&config.packages)
    {
        if !input_compatible(input, package) {
            continue;
        }
        for key in ["layout", "idempotency", "middleware", "api_reference"] {
            ensure!(
                raw.get(key).is_none(),
                "native {} packages do not support {key}",
                input.format
            );
        }
        let allowed: &[&str] = match (input.format.as_str(), package.language.as_str()) {
            ("graphql", "postman") => &[
                "name",
                "output",
                "base_url",
                "strict",
                "group_by_tag",
                "split_by_group",
            ],
            ("graphql", "rust-cli" | "typescript-cli") => &["name", "command_name", "base_url"],
            ("graphql", _) => &[
                "name",
                "transport",
                "subscriptions",
                "scalars",
                "style",
                "raw",
                "groups",
                "contracts",
            ],
            ("protobuf", _) => &["name", "module", "toolchain", "go_packages"],
            _ => &["name"],
        };
        for (raw_plugin, plugin) in raw["plugins"]
            .as_array()
            .context("plugins must be an array")?
            .iter()
            .zip(&package.plugins)
        {
            let supported = if matches!(
                plugin.name.as_str(),
                "graphql" | "sdk" | "workflow" | "asyncapi" | "grpc" | "cli" | "collection"
            ) {
                allowed
            } else {
                match plugin.name.as_str() {
                    "cypress" => &["name", "output", "cypress_options"][..],
                    "faker" => &["name", "output", "fixture_options"][..],
                    _ => &["name", "output"][..],
                }
            };
            for key in raw_plugin
                .as_object()
                .context("plugin must be an object")?
                .keys()
            {
                ensure!(
                    supported.contains(&key.as_str()),
                    "native {} plugin option {key:?} is unsupported",
                    input.format
                );
            }
        }
        let plugin = package
            .plugins
            .iter()
            .find(|p| {
                matches!(
                    p.name.as_str(),
                    "graphql" | "sdk" | "workflow" | "asyncapi" | "grpc" | "cli" | "collection"
                )
            })
            .context("missing native generator")?;
        ensure!(
            plugin.client_name.is_none(),
            "native GraphQL client_name is unsupported"
        );
        ensure!(
            !(input.format == "graphql"
                && (package.sdk_raw || plugin.raw == Some(true))
                && config.defaults.client_style.is_some()),
            "GraphQL raw and explicit default client_style are mutually exclusive"
        );
        ensure!(
            input.format == "graphql" || (package.client_style.is_none() && !package.sdk_raw),
            "native client style/raw applies only to GraphQL"
        );
        if input.format == "graphql"
            && [
                "rust", "go", "python", "php", "java", "csharp", "dotnet", "ruby", "swift",
                "elixir",
            ]
            .contains(&package.language.as_str())
        {
            ensure!(
                plugin.transport.is_none(),
                "{} GraphQL transport options are not supported",
                package.language
            );
        }
        if input.format == "graphql" && package.language == "typescript" {
            ensure!(
                plugin.transport.as_deref().is_none_or(|v| v == "fetch"),
                "GraphQL HTTP transport must be fetch"
            );
        }
        if input.format == "protobuf" {
            ensure!(
                plugin.module.as_ref().is_some_and(|v| !v.is_empty()),
                "grpc plugin requires module import path"
            );
        }
    }
    Ok(())
}

#[path = "native_graphql_tools.rs"]
mod tools;
use tools::append_graphql_tools;
