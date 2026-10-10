//! Construct native GraphQL SDK packages using canonical language factories.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn append(
    old: ProfileSet,
    input: &NativeInputConfig,
    registry: Arc<InputRegistry>,
    language: &str,
    path: &str,
    name: Option<&str>,
    common: Common,
    plugin: Option<&PluginConfig>,
    raw: bool,
    style: &str,
) -> Result<ProfileSet> {
    Ok(if language == "rust" {
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
    } else if language == "go" {
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
    } else if language == "python" {
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
    } else if language == "php" {
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
    } else if language == "symfony" {
        ensure!(
            plugin.is_none_or(|p| p.scalars.is_empty()),
            "Symfony GraphQL custom scalar mappings are not supported"
        );
        let input_provider = provider::<GraphqlOperations>(input, registry.clone());
        let generator = symfony::graphql(Some(input_provider.handle()))
            .groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
        ensure!(
            plugin.and_then(|p| p.subscriptions) != Some(true),
            "Symfony GraphQL subscriptions are unsupported; use the PHP SDK streaming generator"
        );
        let generator = if raw || style == "raw" {
            generator.raw()
        } else if style == "flat" {
            generator.flat()
        } else {
            generator.idiomatic()
        };
        let mut package = symfony::package(path)
            .common(common)
            .with(generator)
            .with(input_provider);
        if let Some(name) = name {
            package = package.name(name);
        }
        old.package(package)
    } else if language == "java" {
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
    } else if language == "csharp" {
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
    } else if language == "ruby" {
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
    } else if language == "swift" {
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
    } else if language == "elixir" {
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
    } else {
        anyhow::bail!("unsupported GraphQL output language {language}")
    })
}
