//! Select the explicit incremental contract; never coerce it into unary operations.
use super::*;
use poolster_core::native::GraphqlIncrementalOperations;
pub(super) fn append(
    mut profiles: ProfileSet,
    request: &Request,
    package: SdkPackage,
    registry: Arc<poolster_core::input::InputRegistry>,
) -> AnyResult<ProfileSet> {
    if package.subscriptions.unwrap_or(request.subscriptions)
        || package.transport.is_some()
        || !package.plugins.is_empty()
        || package.command_name.is_some()
        || package.endpoint.is_some()
    {
        bail!(
            "Incremental GraphQL does not support subscriptions, HTTP transport overrides, auxiliary plugins or CLI options"
        );
    }
    if package.raw == Some(true) && package.style.is_some() {
        bail!("GraphQL raw and style are mutually exclusive");
    }
    let style = package.style.as_deref().unwrap_or("idiomatic");
    if !["raw", "flat", "idiomatic", "namespaced", "grouped"].contains(&style) {
        bail!("Invalid GraphQL style");
    }
    let input =
        InputProvider::<GraphqlIncrementalOperations>::new(registry, "graphql", &request.source)
            .using(&request.provider)
            .with_options(InputOptions {
                operation_files: request.operation_files.iter().map(Into::into).collect(),
                import_roots: request.import_roots.iter().map(Into::into).collect(),
                graphql_incremental: true,
                ..Default::default()
            });
    let common = Common {
        package_version: package.version.clone(),
        ..Default::default()
    };
    match package.language.as_str() {
        "typescript" => {
            if package.style.as_deref().is_some_and(|s| s != "raw") || !package.groups.is_empty() {
                bail!("TypeScript incremental execution currently exposes raw operation functions");
            }
            let mut scalars = request.scalars.clone();
            for (name, mapping) in &package.scalars {
                if scalars.get(name).is_some_and(|old| old != mapping) {
                    bail!("conflicting TypeScript scalar mapping {name}");
                }
                scalars.insert(name.clone(), mapping.clone());
            }
            let generator = ts::graphql_incremental(Some(input.handle())).scalars(scalars);
            let mut target = ts::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "rust" => {
            let generator =
                rust::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            let mut scalars = request.rust_scalars.clone();
            for (name, mapping) in &package.scalars {
                scalars.insert(
                    name.clone(),
                    rust::GraphqlScalarMapping::new(&mapping.input, &mapping.output),
                );
            }
            let generator = generator.scalars(scalars);
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = rust::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "go" => {
            let generator =
                go::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            let mut scalars = BTreeMap::new();
            for (name, mapping) in &package.scalars {
                scalars.insert(
                    name.clone(),
                    go::GraphqlScalarMapping::new(&mapping.input, &mapping.output),
                );
            }
            let generator = generator.scalars(scalars);
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = go::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "python" => {
            let generator =
                python::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = python::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "php" => {
            let generator =
                php::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = php::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "java" => {
            let generator =
                java::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = java::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "csharp" => {
            let generator =
                csharp::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = csharp::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "dotnet" => {
            let generator =
                dotnet::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = dotnet::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "ruby" => {
            let generator =
                ruby::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = ruby::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "swift" => {
            let generator =
                swift::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = swift::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        "elixir" => {
            let generator =
                elixir::graphql_incremental(Some(input.handle())).groups(package.groups.clone());
            if !package.scalars.is_empty() {
                bail!(
                    "This GraphQL output uses runtime scalar callbacks, not source type mappings"
                );
            }
            let generator = if package.raw == Some(true) || style == "raw" {
                generator.raw()
            } else if style == "flat" {
                generator.flat()
            } else {
                generator.idiomatic()
            };
            let mut target = elixir::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target);
        }
        other => bail!("Incremental GraphQL SDK output is unsupported for {other}"),
    }
    Ok(profiles)
}
