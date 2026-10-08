//! Native generation shares package assembly and generated-file ownership with HTTP.
use super::*;
use poolster_core::{input::InputProvider, native::GraphqlOperations};

fn skipped_outputs(options: &Generate, input: &NativeInputConfig) -> Vec<serde_json::Value> {
    let supported_format = input.format == "graphql";
    let reason = if supported_format {
        "output does not consume GraphqlOperations"
    } else {
        "no usable output pipeline is bundled for this input format"
    };
    if let Some(packages) = &options.config_packages {
        packages.iter().filter(|package| !supported_format || package.language != "typescript" || package.plugins.len() != 1 || package.plugins[0].name != "graphql").map(|package| serde_json::json!({
            "input_format": input.format, "package": package.path, "language": package.language,
            "plugins": package.plugins.iter().map(|plugin| plugin.name.as_str()).collect::<Vec<_>>(), "reason": reason
        })).collect()
    } else {
        options.languages.iter().filter(|language| !supported_format || language.as_str() != "typescript").map(|language| serde_json::json!({
            "input_format": input.format, "package": language, "language": language,
            "plugins": [if language == "typescript" { "graphql" } else { "sdk" }], "reason": reason
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
    if input.format != "graphql" {
        eprintln!(
            "warning: no usable output pipeline is bundled for input format {:?}; no files exported",
            input.format
        );
        return report_empty();
    }
    let compatible = |package: &PackageConfig| {
        package.language == "typescript"
            && package.plugins.len() == 1
            && package.plugins[0].name == "graphql"
    };
    if let Some(packages) = &options.config_packages {
        ensure!(
            !packages.is_empty(),
            "native generation requires at least one package"
        );
        for package in packages.iter().filter(|package| !compatible(package)) {
            eprintln!(
                "warning: GraphQL input is incompatible with package {:?} ({}); package skipped",
                package.path, package.language
            );
        }
        if !packages.iter().any(compatible) {
            return report_empty();
        }
    } else if !options
        .languages
        .iter()
        .any(|language| language == "typescript")
    {
        eprintln!("warning: GraphQL input supports only TypeScript; no files exported");
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
        "GraphQL supports the HTTP fetch transport; OpenAPI surface and client options are unsupported"
    );
    ensure!(
        options.jobs == 0 && matches!(options.style, SdkClientStyle::Namespaced),
        "GraphQL does not support OpenAPI client-style or Go worker options"
    );
    let registry = std::sync::Arc::new(poolster_inputs::default_registry()?);
    let mut profiles = ProfileSet::new(".");
    let mut add_package = |path: &str,
                           name: Option<&str>,
                           version: Option<&str>,
                           subscriptions: bool|
     -> Result<()> {
        let mut provider =
            InputProvider::<GraphqlOperations>::new(registry.clone(), &input.format, &input.path)
                .with_options(input.options.clone());
        if let Some(id) = &input.provider {
            provider = provider.using(id);
        }
        let handle = provider.handle();
        let mut graphql = ts::graphql(Some(handle));
        if subscriptions {
            graphql = graphql.subscriptions();
        }
        let mut package = ts::package(path).with(provider).with(graphql);
        if let Some(name) = name {
            package = package.name(name);
        }
        let common = Common {
            package_version: Some(version.unwrap_or(&options.version).to_owned()),
            ..Default::default()
        };
        profiles =
            std::mem::replace(&mut profiles, ProfileSet::new(".")).package(package.common(common));
        Ok(())
    };
    if let Some(packages) = &options.config_packages {
        ensure!(
            !packages.is_empty(),
            "native generation requires at least one package"
        );
        // Validate compatible packages before loading any source or running plugins.
        for package in packages.iter().filter(|package| compatible(package)) {
            ensure!(
                package.layout.is_none() && package.middleware.is_empty() && !package.api_reference,
                "GraphQL does not support HTTP layout, middleware or API-reference settings"
            );
            let plugin = &package.plugins[0];
            ensure!(
                plugin
                    .transport
                    .as_deref()
                    .is_none_or(|value| value == "fetch"),
                "GraphQL HTTP transport must be fetch"
            );
            ensure!(
                plugin.uses.is_empty() && plugin.id.is_none(),
                "GraphQL recipe provider bindings use input.provider"
            );
        }
        for package in packages.iter().filter(|package| compatible(package)) {
            add_package(
                &package.path,
                package.name.as_deref(),
                package.version.as_deref(),
                package.plugins[0].subscriptions.unwrap_or(false),
            )?;
        }
    } else {
        for language in options
            .languages
            .iter()
            .filter(|language| language.as_str() != "typescript")
        {
            eprintln!("warning: GraphQL input is incompatible with {language}; package skipped");
        }
        add_package("typescript", None, None, false)?;
    }
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
    let provenance = serde_json::json!({
        "version": 1, "generator": env!("CARGO_PKG_VERSION"),
        "input": input, "sources": sources, "config_sha256": options.config_sha256,
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
            .filter(|language| language.as_str() != "typescript")
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
mod tests {
    use super::*;

    fn recipe(root: &Path, packages: serde_json::Value) -> PathBuf {
        std::fs::write(root.join("schema.graphql"), "type Query { hello: String! }").unwrap();
        std::fs::write(root.join("operations.graphql"), "query Hello { hello }").unwrap();
        let path = root.join("poolster.json");
        std::fs::write(&path, serde_json::to_vec(&serde_json::json!({
            "input": {"format":"graphql", "provider":"graphql.apollo", "path":"schema.graphql", "options":{"operation_files":["operations.graphql"]}},
            "output":{"path":"generated"}, "packages":packages
        })).unwrap()).unwrap();
        path
    }

    #[test]
    fn native_recipe_resolves_operation_paths_and_checks_regeneration() {
        let directory = tempfile::tempdir().unwrap();
        let path = recipe(
            directory.path(),
            serde_json::json!([{"language":"typescript","path":"sdk","name":"@example/graphql","plugins":[{"name":"graphql"}]}]),
        );
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        assert!(directory.path().join("generated/sdk/package.json").exists());
        generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
        std::fs::write(
            directory.path().join("operations.graphql"),
            "query Renamed { hello }",
        )
        .unwrap();
        assert!(generate_from_config(&path, ColorChoice::Never, true, false).is_err());
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    }

    #[test]
    fn incompatible_outputs_warn_without_reading_schema_or_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = recipe(
            directory.path(),
            serde_json::json!([{"language":"go","path":"sdk","plugins":[{"name":"sdk"}]}]),
        );
        std::fs::remove_file(directory.path().join("schema.graphql")).unwrap();
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        assert!(!directory.path().join("generated").exists());
    }

    #[test]
    fn mixed_recipe_preserves_skipped_owned_files_including_local_edits() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("generated");
        let mut previous = GeneratedTree::default();
        previous
            .insert(GeneratedFile::new("go/client.go", "old generated content").unwrap())
            .unwrap();
        previous.set_owner("go/client.go", "go-sdk").unwrap();
        previous.write_to(&output).unwrap();
        std::fs::write(output.join("go/client.go"), "local edits must survive").unwrap();
        let path = recipe(
            directory.path(),
            serde_json::json!([
                {"language":"typescript","path":"ts","plugins":[{"name":"graphql"}]},
                {"language":"go","path":"go","plugins":[{"name":"sdk"}]}
            ]),
        );
        generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(output.join("go/client.go")).unwrap(),
            "local edits must survive"
        );
        assert!(output.join("ts/package.json").exists());
        generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    }

    #[test]
    fn empty_native_recipe_is_configuration_error_without_output() {
        let directory = tempfile::tempdir().unwrap();
        let path = recipe(directory.path(), serde_json::json!([]));
        let error = generate_from_config(&path, ColorChoice::Never, false, false).unwrap_err();
        assert!(format!("{error:#}").contains("at least one package"));
        assert!(!directory.path().join("generated").exists());
    }

    #[test]
    fn unsupported_options_and_provider_selection_fail_without_export() {
        let directory = tempfile::tempdir().unwrap();
        let path = recipe(
            directory.path(),
            serde_json::json!([{"language":"typescript","path":"ts","plugins":[{"name":"graphql"}]}]),
        );
        let mut config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        config["input"]["provider"] = "graphql.missing".into();
        std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let error = generate_from_config(&path, ColorChoice::Never, false, false).unwrap_err();
        assert!(format!("{error:#}").contains("unknown input provider"));
        config["input"]["provider"] = "graphql.apollo".into();
        config["input"]["options"]["broker"] = serde_json::json!({"kind":"nats"});
        std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(generate_from_config(&path, ColorChoice::Never, false, false).is_err());
        assert!(!directory.path().join("generated").exists());
    }

    #[test]
    fn native_flags_parse_without_changing_openapi_default() {
        let Action::Generate(options) = parse("generate schema.graphql --input-format graphql --provider graphql.apollo --operation a.graphql --operation b.graphql -o out -l typescript".split_whitespace().map(OsString::from)).unwrap() else { panic!() };
        let input = options.native_input.unwrap();
        assert_eq!(input.format, "graphql");
        assert_eq!(input.options.operation_files.len(), 2);
        let Action::Generate(options) = parse(
            "generate api.yaml -o out -l go"
                .split_whitespace()
                .map(OsString::from),
        )
        .unwrap() else {
            panic!()
        };
        assert!(options.native_input.is_none());
        assert!(options.source.is_some());
    }
}
