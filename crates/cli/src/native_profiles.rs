//! Select native contract consumers without adapting protocols into HTTP APIs.
use super::*;
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
    (format == "graphql" && language == "rust")
        || pipeline(format).is_some_and(|(target, _)| language == target)
}
pub(super) fn compatible(format: &str, package: &PackageConfig) -> bool {
    if format == "graphql" && package.language == "rust" {
        return package.plugins.len() == 1 && package.plugins[0].name == "graphql";
    }
    pipeline(format).is_some_and(|(language, plugin)| {
        package.language == language
            && package.plugins.len() == 1
            && package.plugins[0].name == plugin
    })
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
                      plugin: Option<&PluginConfig>|
     -> Result<()> {
        let common = Common {
            package_version: Some(version.unwrap_or(&options.version).to_owned()),
            ..Default::default()
        };
        let old = std::mem::replace(&mut profiles, ProfileSet::new("."));
        profiles = if input.format == "graphql" && language == "rust" {
            let input_provider = provider::<GraphqlOperations>(input, registry.clone());
            let mut package = rust::package(path)
                .common(common)
                .with(rust::graphql(Some(input_provider.handle())))
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
                    let output_plugin = if plugin
                        .and_then(|plugin| plugin.subscriptions)
                        .unwrap_or(false)
                    {
                        output_plugin.subscriptions()
                    } else {
                        output_plugin
                    };
                    ts::package(path).with(input_provider).with(output_plugin)
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
            .filter(|package| compatible(&input.format, package))
        {
            append(
                &package.language,
                &package.path,
                package.name.as_deref(),
                package.version.as_deref(),
                Some(&package.plugins[0]),
            )?;
        }
    } else {
        for language in options
            .languages
            .iter()
            .filter(|language| language_compatible(&input.format, language))
        {
            append(language, language, None, None, None)?;
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
        document
            .get("defaults")
            .is_none_or(|value| value.as_object().is_some_and(|object| object.is_empty())),
        "native recipes do not support HTTP defaults"
    );
    for (raw, package) in document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .zip(&config.packages)
    {
        if !compatible(&input.format, package) {
            continue;
        }
        for key in [
            "client_style",
            "layout",
            "idempotency",
            "middleware",
            "api_reference",
        ] {
            ensure!(
                raw.get(key).is_none(),
                "native {} packages do not support {key}",
                input.format
            );
        }
        let allowed: &[&str] = match input.format.as_str() {
            "graphql" => &["name", "transport", "subscriptions", "scalars"],
            "protobuf" => &["name", "module", "toolchain", "go_packages"],
            _ => &["name"],
        };
        for key in raw["plugins"][0]
            .as_object()
            .context("plugin must be an object")?
            .keys()
        {
            ensure!(
                allowed.contains(&key.as_str()),
                "native {} plugin option {key:?} is unsupported",
                input.format
            );
        }
        let plugin = &package.plugins[0];
        if input.format == "graphql" && package.language == "rust" {
            ensure!(
                plugin.scalars.is_empty(),
                "Rust GraphQL scalar mappings are not supported"
            );
            ensure!(
                plugin.subscriptions != Some(true),
                "Rust GraphQL subscriptions are not supported"
            );
            ensure!(
                plugin.transport.is_none(),
                "Rust GraphQL transport options are not supported"
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
