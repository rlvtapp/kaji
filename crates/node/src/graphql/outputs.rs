use super::*;

pub(super) fn append(
    mut profiles: ProfileSet,
    request: &Request,
    package: SdkPackage,
    input: InputProvider<GraphqlOperations>,
    subscriptions: bool,
) -> AnyResult<ProfileSet> {
    let style = package.style.as_deref().unwrap_or("idiomatic").to_owned();
    if !["raw", "flat", "idiomatic", "namespaced", "grouped"].contains(&style.as_str()) {
        bail!("GraphQL style must be raw, flat, idiomatic, or namespaced");
    }
    let common = Common {
        package_version: package.version.clone(),
        source_quality: package.source_quality.clone(),
        ..Default::default()
    };
    if ["postman", "rust-cli", "typescript-cli"].contains(&package.language.as_str()) {
        if subscriptions
            || package.transport.is_some()
            || package.style.is_some()
            || package.raw.is_some()
            || !package.groups.is_empty()
            || !package.scalars.is_empty()
        {
            bail!(
                "GraphQL collections/CLIs do not support SDK styles, scalar mappings, groups or subscriptions"
            );
        }
        profiles = tools::append(profiles, package, common, input)?;
        return Ok(profiles);
    }
    if package.command_name.is_some() || package.endpoint.is_some() {
        bail!("commandName and endpoint are GraphQL collection/CLI options");
    }
    if package.language == "rust" {
        if package.transport.is_some() {
            bail!("rust GraphQL does not support transport options");
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
        if subscriptions {
            generator = generator.subscriptions();
        }
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
        return Ok(profiles);
    }
    if package.language == "go" {
        if package.transport.is_some() {
            bail!("go GraphQL does not support transport options");
        }
        let generator = go::graphql(Some(input.handle()))
            .groups(package.groups.clone())
            .scalars(
                package
                    .scalars
                    .iter()
                    .map(|(name, m)| {
                        (
                            name.clone(),
                            go::GraphqlScalarMapping::new(&m.input, &m.output),
                        )
                    })
                    .collect(),
            );
        let generator = if subscriptions {
            generator.subscriptions()
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "python" {
        if package.transport.is_some() {
            bail!("python GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("python GraphQL custom scalar mappings are not supported");
        }
        let generator = python::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions()
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "php" {
        if package.transport.is_some() {
            bail!("php GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("php GraphQL custom scalar mappings are not supported");
        }
        let generator = php::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions()
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "symfony" {
        if package.transport.is_some() {
            bail!("Symfony GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("Symfony GraphQL custom scalar mappings are not supported");
        }
        let generator = symfony::graphql(Some(input.handle())).groups(package.groups.clone());
        if subscriptions {
            bail!(
                "Symfony GraphQL subscriptions are unsupported; use the PHP SDK streaming generator"
            );
        }
        let generator = if package.raw.unwrap_or(false) || style == "raw" {
            generator.raw()
        } else if style == "flat" {
            generator.flat()
        } else {
            generator.idiomatic()
        };
        let mut target = symfony::package(package.path)
            .common(common)
            .with(generator)
            .with(input);
        if let Some(name) = package.name {
            target = target.name(name);
        }
        profiles = profiles.package(target);
        return Ok(profiles);
    }
    if package.language == "java" {
        if package.transport.is_some() {
            bail!("java GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("java GraphQL custom scalar mappings are not supported");
        }
        let generator = java::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions(true)
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "csharp" {
        if package.transport.is_some() {
            bail!("csharp GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("csharp GraphQL custom scalar mappings are not supported");
        }
        let generator = csharp::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions(true)
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "ruby" {
        if package.transport.is_some() {
            bail!("ruby GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("ruby GraphQL custom scalar mappings are not supported");
        }
        let generator = ruby::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions()
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "swift" {
        if package.transport.is_some() {
            bail!("swift GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("swift GraphQL custom scalar mappings are not supported");
        }
        let generator = swift::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions(true)
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    if package.language == "elixir" {
        if package.transport.is_some() {
            bail!("elixir GraphQL does not support transport options");
        }
        if !package.scalars.is_empty() {
            bail!("elixir GraphQL custom scalar mappings are not supported");
        }
        let generator = elixir::graphql(Some(input.handle())).groups(package.groups.clone());
        let generator = if subscriptions {
            generator.subscriptions()
        } else {
            generator
        };
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
        return Ok(profiles);
    }
    typescript::append(
        profiles,
        request,
        package,
        common,
        input,
        subscriptions,
        &style,
    )
}
