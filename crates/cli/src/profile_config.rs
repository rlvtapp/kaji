use super::*;

pub(super) fn parse_style(value: Option<&str>) -> Result<SdkClientStyle> {
    match value.unwrap_or("namespaced") {
        "namespaced" | "idiomatic" => Ok(SdkClientStyle::Namespaced),
        "flat" => Ok(SdkClientStyle::Flat),
        other => bail!("client_style must be \"namespaced\" or \"flat\", got {other:?}"),
    }
}

pub(super) fn package_common(style: SdkClientStyle) -> Common {
    Common::default().client_style(style)
}

pub(super) fn configured_common(style: SdkClientStyle, package: &PackageConfig) -> Common {
    let mut common = package_common(style);
    common.package_version = package.version.clone();
    common.layout = package.layout.clone();
    common.source_quality = package.source_quality.clone();
    common
}

pub(super) fn profiles(options: &Generate) -> Result<ProfileSet> {
    if let Some(packages) = &options.config_packages {
        return config_profiles(options.style, packages);
    }

    let mut profiles = ProfileSet::new(".").common(Common::default().client_style(options.style));
    for target in &options.languages {
        profiles = match target.as_str() {
            "rust" => profiles.package(rust::package("rust").with(rust::sdk())),
            "rust-cli" => profiles.package(rust_cli::package(target).with(rust_cli::cli())),
            "go" => profiles.package(go::package("go").with(go::sdk().jobs(options.jobs))),
            "python" => profiles.package(python::package("python").with(python::sdk())),
            "php" => profiles.package(php::package("php").with(php::sdk())),
            "symfony" => profiles.package(symfony::package("symfony").with(symfony::sdk())),
            "java" => profiles.package(java::package("java").with(java::sdk())),
            "csharp" => profiles.package(csharp::package("csharp").with(csharp::sdk())),
            "elixir" => profiles.package(elixir::package("elixir").with(elixir::sdk())),
            "ruby" => profiles.package(ruby::package("ruby").with(ruby::sdk())),
            "swift" => profiles.package(swift::package("swift").with(swift::sdk())),
            "postman" => profiles.package(
                postman::package("postman")
                    .with(postman::collection())
                    .with(postman::environment()),
            ),
            "terraform" => {
                profiles.package(terraform::package("terraform").with(terraform::provider()))
            }
            "typescript" => {
                let mut sdk = match options
                    .typescript_transport
                    .unwrap_or(TypeScriptTransport::Fetch)
                {
                    TypeScriptTransport::Fetch => ts::sdk().fetch(),
                    TypeScriptTransport::Axios => ts::sdk().axios(),
                };
                if options.raw {
                    sdk = sdk.raw();
                }
                if let Some(name) = &options.client_name {
                    sdk = sdk.client_name(name);
                }
                profiles.package(ts::package(target).with(sdk))
            }
            "typescript-cli" => profiles.package(ts_cli::package(target).with(ts_cli::cli())),
            _ => unreachable!("validated target"),
        };
    }
    Ok(profiles)
}

pub(super) fn sdk_plugin(package: &PackageConfig) -> Result<&PluginConfig> {
    let plugins = package
        .plugins
        .iter()
        .filter(|plugin| plugin.name == "sdk")
        .collect::<Vec<_>>();
    match plugins.as_slice() {
        [plugin] => Ok(plugin),
        [] => bail!(
            "package {:?} ({}) requires exactly one {{\"name\":\"sdk\"}} plugin",
            package.path,
            package.language
        ),
        _ => bail!("package {:?} declares sdk more than once", package.path),
    }
}

pub(super) fn has_only_known_plugins(package: &PackageConfig, allowed: &[&str]) -> Result<()> {
    for plugin in &package.plugins {
        if !allowed.contains(&plugin.name.as_str()) {
            bail!(
                "package {:?} uses plugin {:?}, which is not bundled by this Poolster binary",
                package.path,
                plugin.name
            );
        }
    }
    Ok(())
}

pub(super) fn has_typescript_provider(package: &PackageConfig) -> bool {
    package.plugins.iter().any(|plugin| {
        matches!(
            plugin.name.as_str(),
            "sdk" | "models" | "transport" | "operations" | "client"
        )
    })
}

pub(super) fn typescript_models(plugin: &PluginConfig) -> Result<ts::ModelOptions> {
    let int64_type = match plugin.int64.as_deref().unwrap_or("number") {
        "number" => ts::Int64Type::Number,
        "string" => ts::Int64Type::String,
        "bigint" => ts::Int64Type::BigInt,
        other => bail!("TypeScript int64 must be number, string, or bigint; got {other:?}"),
    };
    Ok(ts::ModelOptions {
        open_enums: plugin.open_enums.unwrap_or(false),
        integer_as_string: plugin.integer_as_string.unwrap_or(false),
        int64_type,
        ..Default::default()
    })
}

pub(super) fn with_configured_middleware<L: poolster_core::engine::Language>(
    builder: Package<L>,
    package: &PackageConfig,
) -> Package<L> {
    let builder = builder.idempotency(package.idempotency.clone());
    let builder = if package.api_reference {
        builder.with(poolster_core::api_reference::<L>())
    } else {
        builder
    };
    package
        .resolved_middleware
        .iter()
        .cloned()
        .fold(builder, |builder, middleware| {
            builder.middleware(middleware)
        })
}

pub(super) fn config_profiles(
    default_style: SdkClientStyle,
    packages: &[PackageConfig],
) -> Result<ProfileSet> {
    if packages.is_empty() {
        bail!("poolster.json must declare at least one package");
    }
    let mut profiles = ProfileSet::new(".").common(Common::default().client_style(default_style));
    for package in packages {
        ensure!(
            package
                .plugins
                .iter()
                .all(|plugin| plugin.open_unions.is_none()
                    || (package.language == "rust" && plugin.name == "sdk")),
            "open_unions is only supported by the Rust SDK plugin"
        );
        if !matches!(package.language.as_str(), "java" | "csharp") {
            ensure!(
                package
                    .plugins
                    .iter()
                    .all(|plugin| plugin.preserve_presence.is_none()),
                "preserve_presence is only supported by Java and C# SDK plugins"
            );
        }
        if !matches!(package.language.as_str(), "typescript" | "ts") {
            ensure!(
                package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "oauth")
                    .all(|plugin| plugin.uses.is_empty()),
                "OAuth recipe bindings are supported only for TypeScript; use the native library API for other targets"
            );
        }
        if matches!(
            package.language.as_str(),
            "php" | "java" | "csharp" | "elixir" | "ruby" | "swift"
        ) {
            for consumer in &package.plugins {
                if consumer.name == "operation-tests" {
                    ensure!(
                        consumer.uses.is_empty(),
                        "{} operation-tests recipes infer the bundled SDK; explicit uses bindings require the typed Rust API",
                        package.language
                    );
                }
            }
        }
        ensure!(
            !package.sdk_raw
                && package.plugins.iter().all(|p| p.scalars.is_empty()
                    && p.groups.is_empty()
                    && p.style.is_none()
                    && p.raw.is_none()),
            "GraphQL scalar/style/raw plugin options require GraphQL input"
        );
        let style = match package.client_style.as_deref() {
            Some(style) => parse_style(Some(style))?,
            None => default_style,
        };
        profiles = match package.language.as_str() {
            "typescript" => config_profile_cli::apply_typescript(profiles, package, style)?,
            "typescript-cli" => config_profile_cli::apply_typescript_cli(profiles, package, style)?,
            "rust-cli" => config_profile_cli::apply_rust_cli(profiles, package, style)?,
            "rust" => config_profile_sdk::apply_rust(profiles, package, style)?,
            "go" => config_profile_sdk::apply_go(profiles, package, style)?,
            "python" => config_profile_sdk::apply_python(profiles, package, style)?,
            "php" => config_profile_sdk::apply_php(profiles, package, style)?,
            "symfony" => config_profile_sdk::apply_symfony(profiles, package, style)?,
            "java" => config_profile_sdk::apply_java(profiles, package, style)?,
            "csharp" => config_profile_sdk::apply_csharp(profiles, package, style)?,
            "elixir" => config_profile_other::apply_elixir(profiles, package, style)?,
            "ruby" => config_profile_other::apply_ruby(profiles, package, style)?,
            "swift" => config_profile_other::apply_swift(profiles, package, style)?,
            "postman" => config_profile_other::apply_postman(profiles, package, style)?,
            "terraform" => config_profile_other::apply_terraform(profiles, package, style)?,
            "mock" => config_profile_other::apply_mock(profiles, package, style)?,
            "artifacts" => config_profile_other::apply_artifacts(profiles, package, style)?,
            other => bail!(
                "unknown config language {other:?}; use typescript, typescript-cli, rust, rust-cli, go, python, php, symfony, java, csharp, elixir, ruby, swift, postman, terraform, mock, or artifacts"
            ),
        };
    }
    Ok(profiles)
}
