//! Select native contract consumers without adapting protocols into HTTP APIs.
use super::*;
mod build;
mod graphql_sdk;
mod incremental;
pub(super) use build::build;
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
            "rust", "go", "python", "php", "symfony", "java", "csharp", "ruby", "swift", "elixir",
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
            && ["postman", "rust-cli", "typescript-cli", "symfony"].contains(&language))
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
                "rust", "go", "python", "php", "java", "csharp", "ruby", "swift", "elixir",
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
