use super::*;

pub(super) fn apply_rust(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "operation-tests", "webhooks", "oauth"])?;
        let package_builder =
            rust::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let open_unions = package
            .plugins
            .iter()
            .find(|plugin| plugin.name == "sdk")
            .and_then(|plugin| plugin.open_unions)
            .unwrap_or(false);
        let open_enums = package
            .plugins
            .iter()
            .find(|plugin| plugin.name == "sdk")
            .and_then(|plugin| plugin.open_enums)
            .unwrap_or(false);
        let mut package_builder = package_builder
            .open_unions(open_unions)
            .open_enums(open_enums)
            .with(rust::sdk());
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(rust::operation_tests());
        }
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(rust::webhooks());
        }
        if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
            package_builder = package_builder.with(rust::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_go(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "operation-tests", "webhooks", "oauth"])?;
        let plugin = sdk_plugin(package)?;
        let mut sdk = go::sdk();
        if let Some(jobs) = plugin.jobs {
            sdk = sdk.jobs(jobs);
        }
        let package_builder = go::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let mut package_builder = package_builder.with(sdk);
        for consumer in &package.plugins {
            package_builder = match consumer.name.as_str() {
                "operation-tests" => package_builder.with(go::operation_tests()),
                "webhooks" => package_builder.with(go::webhooks()),
                "oauth" => package_builder.with(go::oauth()),
                _ => package_builder,
            };
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_python(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(
            package,
            &["sdk", "webhooks", "roundtrips", "operation-tests"],
        )?;
        let package_builder =
            python::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let plugin = sdk_plugin(package)?;
        let mut package_builder = package_builder
            .open_enums(plugin.open_enums.unwrap_or(false))
            .with(python::sdk().async_client(plugin.async_client.unwrap_or(false)));
        for consumer in &package.plugins {
            match consumer.name.as_str() {
                "webhooks" => package_builder = package_builder.with(python::webhooks()),
                "roundtrips" => package_builder = package_builder.with(python::roundtrips()),
                "operation-tests" => {
                    package_builder = package_builder.with(python::operation_tests())
                }
                _ => {}
            }
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_php(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder = php::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let mut package_builder = package_builder.with(php::sdk());
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(php::webhooks());
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(php::operation_tests());
        }
        if package.plugins.iter().any(|p| p.name == "oauth") {
            package_builder = package_builder.with(php::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_symfony(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk"])?;
        let plugin = sdk_plugin(package)?;
        let package_builder =
            symfony::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let package_builder = if let Some(sdk_package) = &plugin.sdk_package {
            package_builder.sdk_package(sdk_package)
        } else {
            package_builder
        };
        profiles.package(with_configured_middleware(
            package_builder.with(symfony::sdk()),
            package,
        ))
    })
}

pub(super) fn apply_java(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder =
            java::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let plugin = sdk_plugin(package)?;
        let mut package_builder = package_builder.with(
            java::sdk()
                .open_enums(plugin.open_enums.unwrap_or(false))
                .preserve_presence(plugin.preserve_presence.unwrap_or(false)),
        );
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(java::webhooks());
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(java::operation_tests());
        }
        if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
            package_builder = package_builder.with(java::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}

pub(super) fn apply_csharp(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
        let package_builder =
            csharp::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        let plugin = sdk_plugin(package)?;
        let mut package_builder = package_builder.with(
            csharp::sdk()
                .open_enums(plugin.open_enums.unwrap_or(false))
                .preserve_presence(plugin.preserve_presence.unwrap_or(false)),
        );
        if package.plugins.iter().any(|p| p.name == "webhooks") {
            package_builder = package_builder.with(csharp::webhooks());
        }
        if package.plugins.iter().any(|p| p.name == "operation-tests") {
            package_builder = package_builder.with(csharp::operation_tests());
        }
        if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
            package_builder = package_builder.with(csharp::oauth());
        }
        profiles.package(with_configured_middleware(package_builder, package))
    })
}
