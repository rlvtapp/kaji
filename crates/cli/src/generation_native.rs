//! Native generation shares package assembly and generated-file ownership with HTTP.
use super::native_profiles::{compatible as package_compatible, language_compatible, pipeline};
use super::*;

fn skipped_outputs(options: &Generate, input: &NativeInputConfig) -> Vec<serde_json::Value> {
    let supported_format = pipeline(&input.format).is_some();
    let reason = if supported_format {
        "output does not consume the selected native contract"
    } else {
        "no usable output pipeline is bundled for this input format"
    };
    if let Some(packages) = &options.config_packages {
        packages.iter().filter(|package| !package_compatible(&input.format,package)).map(|package| serde_json::json!({
            "input_format": input.format, "package": package.path, "language": package.language,
            "plugins": package.plugins.iter().map(|plugin| plugin.name.as_str()).collect::<Vec<_>>(), "reason": reason
        })).collect()
    } else {
        options.languages.iter().filter(|language| !language_compatible(&input.format, language)).map(|language| serde_json::json!({
            "input_format": input.format, "package": language, "language": language,
            "plugins": [pipeline(&input.format).filter(|(target,_)|language.as_str()==*target).map(|(_,plugin)|plugin).unwrap_or("sdk")], "reason": reason
        })).collect()
    }
}

fn report_changes(
    changes: &poolster_core::files::OutputChanges,
    skipped: &[serde_json::Value],
) -> Result<()> {
    let mut report = serde_json::to_value(changes)?;
    report
        .as_object_mut()
        .expect("changes serialize as an object")
        .insert("skipped".into(), serde_json::to_value(skipped)?);
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

pub(super) fn generate(options: Generate) -> Result<()> {
    let input = options
        .native_input
        .as_ref()
        .context("native input is required")?;
    let skipped = skipped_outputs(&options, input);
    let report_empty = || -> Result<()> {
        if options.json_changes {
            report_changes(&Default::default(), &skipped)?;
        }
        Ok(())
    };
    if pipeline(&input.format).is_none() {
        eprintln!(
            "warning: no usable output pipeline is bundled for input format {:?}; no files exported",
            input.format
        );
        return report_empty();
    }
    let compatible = |package: &PackageConfig| package_compatible(&input.format, package);
    if let Some(packages) = &options.config_packages {
        ensure!(
            !packages.is_empty(),
            "native generation requires at least one package"
        );
        for package in packages.iter().filter(|package| !compatible(package)) {
            eprintln!(
                "warning: {} input is incompatible with package {:?} ({}); package skipped",
                input.format, package.path, package.language
            );
        }
        if !packages.iter().any(compatible) {
            return report_empty();
        }
    } else if !options
        .languages
        .iter()
        .any(|language| language_compatible(&input.format, language))
    {
        eprintln!(
            "warning: {} input does not support the requested output languages; no files exported",
            input.format
        );
        return report_empty();
    }
    ensure!(
        options.artifacts.is_none() && options.compiler.is_none(),
        "native input cannot use OpenAPI artifacts or compiler"
    );
    ensure!(
        options.path_selection == PathSelection::default(),
        "HTTP path filters are unsupported for native inputs"
    );
    ensure!(
        !options.raw
            && options.client_name.is_none()
            && !matches!(
                options.typescript_transport,
                Some(TypeScriptTransport::Axios)
            ),
        "native generation does not support OpenAPI surface/client or axios options"
    );
    ensure!(
        options.jobs == 0 && matches!(options.style, SdkClientStyle::Namespaced),
        "native generation does not support OpenAPI client-style or Go worker options"
    );
    ensure!(
        input.format != "graphql"
            || options.config_packages.is_some()
            || !options.languages.iter().any(|language| language == "rust")
            || options.typescript_transport.is_none(),
        "Rust GraphQL does not support TypeScript transport options"
    );
    ensure!(
        input.format == "graphql" || options.typescript_transport.is_none(),
        "HTTP TypeScript transport options apply only to GraphQL native generation"
    );
    ensure!(
        input.format == "protobuf" || options.native_output == NativeOutputConfig::default(),
        "gRPC module/toolchain/mapping options require Protobuf input"
    );
    for language in options
        .languages
        .iter()
        .filter(|language| !language_compatible(&input.format, language))
    {
        eprintln!(
            "warning: {} input is incompatible with {language}; package skipped",
            input.format
        );
    }
    let profiles = native_profiles::build(&options, input)?;
    let mut tree = poolster::generate_native(profiles)?;
    if let Some(packages) = &options.config_packages {
        for package in packages.iter().filter(|package| compatible(package)) {
            if let Some(configured) = &package.release {
                let mut metadata = configured.clone();
                metadata.language = package.language.clone();
                if metadata.name.is_empty() {
                    metadata.name = package.name.clone().unwrap_or_else(|| package.path.clone());
                }
                metadata.version = package
                    .version
                    .clone()
                    .unwrap_or_else(|| options.version.clone());
                let path =
                    Path::new(&package.path).join(poolster_core::release::PACKAGE_METADATA_PATH);
                tree.insert(GeneratedFile::new(&path, metadata.to_json()?)?)?;
                tree.set_owner(&path, format!("package-metadata:{}", package.path))?;
            }
        }
        let customizations = packages
            .iter()
            .filter(|package| compatible(package))
            .flat_map(|package| {
                package
                    .resolved_customizations
                    .iter()
                    .map(|code| code.prefixed(Path::new(&package.path)))
            })
            .collect::<Vec<_>>();
        poolster_core::customization::apply_code_customizations(&mut tree, &customizations)?;
    }
    let mut sources = BTreeMap::new();
    sources.insert(input.path.display().to_string(), sha256_file(&input.path)?);
    for path in &input.options.operation_files {
        sources.insert(path.display().to_string(), sha256_file(path)?);
    }
    for path in input.options.workflow_sources.values() {
        sources.insert(path.display().to_string(), sha256_file(path)?);
    }
    if input.format == "protobuf" {
        let loaded = poolster_inputs::default_registry()?.load_with_options(
            &input.format,
            input.provider.as_deref(),
            &input.path,
            &input.options,
        )?;
        for file in &loaded
            .contract
            .get::<poolster_core::native::rpc::RpcContract>()?
            .files
        {
            if let Some(source) = &file.source {
                sources.insert(format!("protobuf:{}", file.name), sha256(source.as_bytes()));
            }
        }
    }
    let provenance = serde_json::json!({
        "version": 1, "generator": env!("CARGO_PKG_VERSION"),
        "input": input, "native_output":options.native_output, "sources": sources, "config_sha256": options.config_sha256,
        "replay": "regenerate using the original recipe or native command; poolster update supports HTTP replay only"
    });
    let provenance_path = ".poolster-native-generation.json";
    tree.insert(GeneratedFile::new(
        provenance_path,
        format!("{}\n", serde_json::to_string_pretty(&provenance)?),
    )?)?;
    tree.set_owner(provenance_path, "native-generation-provenance")?;
    if let Some(packages) = &options.config_packages {
        for package in packages.iter().filter(|package| !compatible(package)) {
            tree.preserve_owned_prefix(&options.output, Path::new(&package.path))?;
        }
    } else {
        for language in options
            .languages
            .iter()
            .filter(|language| !language_compatible(&input.format, language))
        {
            tree.preserve_owned_prefix(&options.output, Path::new(language))?;
        }
    }
    let changes = tree.check(&options.output)?;
    if options.json_changes {
        report_changes(&changes, &skipped)?;
    } else if options.check {
        for path in &changes.added {
            println!("added {}", path.display());
        }
        for path in &changes.modified {
            println!("modified {}", path.display());
        }
        for path in &changes.removed {
            println!("removed {}", path.display());
        }
        if changes.is_empty() {
            println!("Generated output is up to date.");
        }
    }
    if options.check {
        ensure!(changes.is_empty(), "generated output has drift");
    } else {
        tree.write_to(&options.output)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "generation_native_tests.rs"]
mod tests;
