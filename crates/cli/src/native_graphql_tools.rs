//! GraphQL collections and executable command packages consume native operations.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn append_graphql_tools(
    profiles: ProfileSet,
    input: &NativeInputConfig,
    registry: Arc<InputRegistry>,
    language: &str,
    path: &str,
    name: Option<&str>,
    common: Common,
    plugin: Option<&PluginConfig>,
    config: Option<&PackageConfig>,
) -> Result<ProfileSet> {
    ensure!(
        plugin.is_none_or(|p| p.style.is_none()
            && p.raw.is_none()
            && p.transport.is_none()
            && p.subscriptions.is_none()
            && p.scalars.is_empty()
            && p.groups.is_empty()),
        "GraphQL collection/CLI plugins do not support SDK style, transport, scalar mappings, grouping or subscription options"
    );
    ensure!(
        config.is_none_or(|p| p.client_style.is_none() && !p.sdk_raw),
        "GraphQL collection/CLI packages do not support SDK style options"
    );
    let input = provider::<GraphqlOperations>(input, registry);
    ensure!(
        plugin.is_none_or(|p| p.oauth.is_none()),
        "GraphQL CLI OAuth discovery is unsupported; supply runtime headers or bearer credentials"
    );
    match language {
        "rust-cli" => {
            let mut generator = rust_cli::graphql().input(input.handle());
            if let Some(value) = plugin.and_then(|p| p.command_name.as_deref()) {
                generator = generator.command_name(value);
            }
            if let Some(value) = plugin.and_then(|p| p.base_url.as_deref()) {
                generator = generator.endpoint(value);
            }
            let mut package = rust_cli::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                package = package.name(name);
            }
            Ok(profiles.package(package))
        }
        "typescript-cli" => {
            let mut generator = ts_cli::graphql().input(input.handle());
            if let Some(value) = plugin.and_then(|p| p.command_name.as_deref()) {
                generator = generator.command_name(value);
            }
            if let Some(value) = plugin.and_then(|p| p.base_url.as_deref()) {
                generator = generator.endpoint(value);
            }
            let mut package = ts_cli::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                package = package.name(name);
            }
            Ok(profiles.package(package))
        }
        "postman" => {
            let mut generator = postman::graphql()
                .input(input.handle())
                .strict(plugin.and_then(|p| p.strict).unwrap_or(true))
                .group_by_kind(plugin.and_then(|p| p.group_by_tag).unwrap_or(true))
                .split_by_group(plugin.and_then(|p| p.split_by_group).unwrap_or(false));
            if let Some(value) = plugin.and_then(|p| p.output.as_deref()) {
                generator = generator.output(value);
            }
            let collection = generator.handle();
            let mut package = postman::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                package = package.name(name);
            }
            if let Some(value) = plugin.and_then(|p| p.base_url.as_deref()) {
                package = package.base_url(value);
            }
            if let Some(env) =
                config.and_then(|p| p.plugins.iter().find(|p| p.name == "environment"))
            {
                let mut environment = postman::graphql_environment().using_collection(collection);
                if let Some(value) = &env.output {
                    environment = environment.output(value);
                }
                package = package.with(environment);
            }
            Ok(profiles.package(package))
        }
        _ => bail!("unsupported GraphQL tool target"),
    }
}
