//! Assemble configured native packages and shared generation options.
use super::*;
pub(crate) fn build(options: &Generate, input: &NativeInputConfig) -> Result<ProfileSet> {
    let registry = Arc::new(poolster_inputs::default_registry()?);
    let mut profiles = ProfileSet::new(".");
    let mut append = |language: &str,
                      path: &str,
                      name: Option<&str>,
                      version: Option<&str>,
                      plugin: Option<&PluginConfig>,
                      package_config: Option<&PackageConfig>|
     -> Result<()> {
        let explicit_style = plugin
            .and_then(|p| p.style.as_deref())
            .or(package_config.and_then(|p| p.client_style.as_deref()));
        let raw = plugin.and_then(|p| p.raw).unwrap_or(false)
            || package_config.is_some_and(|p| p.sdk_raw)
            || options.raw;
        ensure!(
            !(raw && explicit_style.is_some()),
            "GraphQL raw and style are mutually exclusive"
        );
        let style = explicit_style.unwrap_or(if options.style == SdkClientStyle::Flat {
            "flat"
        } else {
            "idiomatic"
        });
        ensure!(
            ["raw", "flat", "idiomatic", "namespaced", "grouped"].contains(&style),
            "GraphQL style must be raw, flat, idiomatic, or namespaced"
        );
        let common = Common {
            package_version: Some(version.unwrap_or(&options.version).to_owned()),
            source_quality: package_config.and_then(|package| package.source_quality.clone()),
            ..Default::default()
        };
        let old = std::mem::replace(&mut profiles, ProfileSet::new("."));
        profiles = if input.format == "graphql" && input.options.graphql_incremental {
            incremental::append(
                old,
                input,
                registry.clone(),
                language,
                path,
                name,
                common,
                plugin,
                raw,
                style,
            )?
        } else if input.format == "graphql"
            && ["postman", "rust-cli", "typescript-cli"].contains(&language)
        {
            append_graphql_tools(
                old,
                input,
                registry.clone(),
                language,
                path,
                name,
                common,
                plugin,
                package_config,
            )?
        } else if input.format == "graphql" {
            graphql_sdk::append(
                old,
                input,
                registry.clone(),
                language,
                path,
                name,
                common,
                plugin,
                raw,
                style,
            )?
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
                    let output_plugin =
                        output_plugin.groups(plugin.map(|p| p.groups.clone()).unwrap_or_default());
                    let output_plugin = if raw || style == "raw" {
                        output_plugin.raw()
                    } else if style == "flat" {
                        output_plugin.flat()
                    } else {
                        output_plugin.idiomatic()
                    };
                    let output_plugin = if plugin
                        .and_then(|plugin| plugin.subscriptions)
                        .unwrap_or(false)
                    {
                        output_plugin.subscriptions()
                    } else {
                        output_plugin
                    };
                    let client = output_plugin.handle();
                    native_graphql_addons::attach(
                        ts::package(path).with(input_provider).with(output_plugin),
                        package_config
                            .map(|p| p.plugins.as_slice())
                            .unwrap_or_default(),
                        client,
                    )?
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
            .filter(|package| input_compatible(input, package))
        {
            append(
                &package.language,
                &package.path,
                package.name.as_deref(),
                package.version.as_deref(),
                package.plugins.iter().find(|p| {
                    matches!(
                        p.name.as_str(),
                        "graphql" | "sdk" | "workflow" | "asyncapi" | "grpc" | "cli" | "collection"
                    )
                }),
                Some(package),
            )?;
        }
    } else {
        for language in options
            .languages
            .iter()
            .filter(|language| input_language_compatible(input, language))
        {
            append(language, language, None, None, None, None)?;
        }
    }
    Ok(profiles)
}
