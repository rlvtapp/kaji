//! Explicit incremental contract dispatch for native recipes.
use super::*;
use poolster_core::native::GraphqlIncrementalOperations;
#[allow(clippy::too_many_arguments)]
pub(super) fn append(
    mut profiles: ProfileSet,
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
    ensure!(
        plugin.is_none_or(|p| p.subscriptions != Some(true)),
        "Incremental GraphQL and subscriptions must be separate input configurations"
    );
    let groups = plugin.map(|p| p.groups.clone()).unwrap_or_default();
    let scalars_config = plugin.map(|p| p.scalars.clone()).unwrap_or_default();
    let input = provider::<GraphqlIncrementalOperations>(input, registry);
    match language {
        "typescript" => {
            if style != "raw" || !groups.is_empty() {
                bail!("TypeScript incremental execution currently exposes raw operation functions");
            }
            let mut scalars = BTreeMap::new();
            for (name, mapping) in &scalars_config {
                scalars.insert(
                    name.clone(),
                    ts::GraphqlScalarMapping::new(&mapping.input, &mapping.output),
                );
            }
            let generator = ts::graphql_incremental(Some(input.handle())).scalars(scalars);
            let mut target = ts::package(path).common(common).with(input).with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "rust" => {
            let generator = rust::graphql_incremental(Some(input.handle())).groups(groups.clone());
            let mut scalars = BTreeMap::new();
            for (name, mapping) in &scalars_config {
                scalars.insert(
                    name.clone(),
                    rust::GraphqlScalarMapping::new(&mapping.input, &mapping.output),
                );
            }
            let generator = generator.scalars(scalars);
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = rust::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "go" => {
            let generator = go::graphql_incremental(Some(input.handle())).groups(groups.clone());
            let mut scalars = BTreeMap::new();
            for (name, mapping) in &scalars_config {
                scalars.insert(
                    name.clone(),
                    go::GraphqlScalarMapping::new(&mapping.input, &mapping.output),
                );
            }
            let generator = generator.scalars(scalars);
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = go::package(path).common(common).with(input).with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "python" => {
            let generator =
                python::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = python::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "php" => {
            let generator = php::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = php::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "java" => {
            let generator = java::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = java::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "csharp" => {
            let generator =
                csharp::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = csharp::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "ruby" => {
            let generator = ruby::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = ruby::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "swift" => {
            let generator = swift::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = swift::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "elixir" => {
            let generator =
                elixir::graphql_incremental(Some(input.handle())).groups(groups.clone());
            if !scalars_config.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if raw || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = elixir::package(path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        other => bail!("Incremental GraphQL SDK output is unsupported for {other}"),
    }
    Ok(profiles)
}
