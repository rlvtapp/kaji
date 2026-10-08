use super::*;

pub(super) fn apply_elixir(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder =
            elixir::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let mut package_builder = package_builder.with(elixir::sdk());
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(elixir::webhooks());
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(elixir::operation_tests());
        }
        if package.plugins.iter().any(|p| p.name == "oauth") {
            package_builder = package_builder.with(elixir::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_ruby(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder =
            ruby::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let mut package_builder = package_builder.with(ruby::sdk());
        for consumer in &package.plugins {
            if consumer.name == "webhooks" {
                package_builder = package_builder.with(ruby::webhooks());
            }
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(ruby::operation_tests());
        }
        if package.plugins.iter().any(|p| p.name == "oauth") {
            package_builder = package_builder.with(ruby::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_swift(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder =
            swift::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let plugin = sdk_plugin(package)?;
        let mut package_builder =
            package_builder.with(swift::sdk().open_enums(plugin.open_enums.unwrap_or(false)));
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(swift::webhooks());
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(swift::operation_tests());
        }
        if package.plugins.iter().any(|p| p.name == "oauth") {
            package_builder = package_builder.with(swift::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_postman(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["collection", "environment"])?;
        let mut builder = postman::package(&package.path).common(configured_common(style, package));
        if let Some(name) = &package.name {
            builder = builder.name(name);
        }
        ensure!(
            package
                .plugins
                .iter()
                .filter(|plugin| plugin.name == "collection")
                .count()
                == 1,
            "postman package requires exactly one collection plugin"
        );
        ensure!(
            package
                .plugins
                .iter()
                .filter(|plugin| plugin.name == "environment")
                .count()
                <= 1,
            "postman package accepts one environment plugin"
        );
        for plugin in &package.plugins {
            ensure!(
                plugin.id.is_none() && plugin.uses.is_empty(),
                "Postman recipe handles are not exposed yet; use the native plugin API"
            );
            if plugin.name == "collection" {
                let mut collection = postman::collection()
                    .strict(plugin.strict.unwrap_or(true))
                    .group_by_tag(plugin.group_by_tag.unwrap_or(true))
                    .split_by_group(plugin.split_by_group.unwrap_or(false));
                if let Some(output) = &plugin.output {
                    collection = collection.output(output);
                }
                if let Some(base_url) = &plugin.base_url {
                    builder = builder.base_url(base_url);
                }
                builder = builder.with(collection);
            } else {
                let mut environment = postman::environment();
                if let Some(output) = &plugin.output {
                    environment = environment.output(output);
                }
                builder = builder.with(environment);
            }
        }
        profiles.package(with_configured_middleware(builder, package))
    })
}

pub(super) fn apply_terraform(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["provider", "release-scaffold"])?;
        let plugins = package
            .plugins
            .iter()
            .filter(|plugin| plugin.name == "provider")
            .collect::<Vec<_>>();
        let [plugin] = plugins.as_slice() else {
            bail!("terraform package requires exactly one provider plugin")
        };
        ensure!(
            plugin.id.is_none() && plugin.uses.is_empty(),
            "Terraform recipe handles are not exposed yet; use the native plugin API"
        );
        let mut builder =
            terraform::package(&package.path).common(configured_common(style, package));
        if let Some(module) = &plugin.module {
            builder = builder.module(module);
        }
        if let Some(name) = plugin.provider_name.as_ref().or(package.name.as_ref()) {
            builder = builder.provider_name(name);
        }
        if let Some(namespace) = &plugin.registry_namespace {
            builder = builder.registry_namespace(namespace);
        }
        let mut provider = terraform::provider()
            .infer(plugin.infer.unwrap_or(true))
            .data_sources(plugin.data_sources.unwrap_or(false));
        for resource in &plugin.resources {
            provider = provider.resource(resource.clone());
        }
        builder = builder.with(provider);
        if package.plugins.iter().any(|p| p.name == "release-scaffold") {
            builder = builder.with(terraform::release_scaffold());
        }
        profiles.package(with_configured_middleware(builder, package))
    })
}

pub(super) fn apply_mock(
    profiles: ProfileSet,
    package: &PackageConfig,
    _style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["server"])?;
        let servers = package
            .plugins
            .iter()
            .filter(|plugin| plugin.name == "server")
            .collect::<Vec<_>>();
        let [server] = servers.as_slice() else {
            bail!(
                "mock package {:?} requires exactly one server plugin",
                package.path
            )
        };
        let mut server_builder = mock::server();
        if let Some(image) = &server.image {
            server_builder = server_builder.image(image);
        }
        if let Some(port) = server.port {
            server_builder = server_builder.port(port);
        }
        profiles.package(with_configured_middleware(
            mock::package(&package.path).with(server_builder),
            package,
        ))
    })
}

pub(super) fn apply_artifacts(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["redoc", "mcp"])?;
        if package.plugins.is_empty() {
            bail!(
                "artifacts package {:?} must declare at least one plugin",
                package.path
            );
        }
        // This package is a typed output-root reservation. ReDoc and MCP files
        // are added by the artifact pass after SDK packages are composed.
        profiles.package(ts::package(&package.path).common(configured_common(style, package)))
    })
}
