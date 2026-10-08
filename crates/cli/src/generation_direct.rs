use super::*;

pub(super) fn generate(mut options: Generate) -> Result<()> {
    if let Some(config) = &options.config {
        return generate_from_config(config, options.color, options.check, options.json_changes);
    }
    if options.native_input.is_some() {
        return super::generation_native::generate(options);
    }
    let reporter = Reporter::new(options.color);
    reporter.started();
    let temporary;
    let artifacts = if let Some(artifacts) = &options.artifacts {
        artifacts.as_path()
    } else {
        let source = options.source.as_ref().expect("validated source");
        temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
        let remote = match source {
            OpenApiInput::Remote(remote) => Some(remote.clone()),
            OpenApiInput::Path(path) => remote_spec_url(path).map(|url| RemoteInput {
                url: url.to_owned(),
                headers: BTreeMap::new(),
                auth: None,
            }),
        };
        let compiler_source_url = remote.as_ref().map(compiler_source_origin).transpose()?;
        let source = if let Some(remote) = remote {
            let downloaded = temporary.path().join("openapi.downloaded.yaml");
            reporter.downloading(&remote.url);
            let started = Instant::now();
            download_openapi(&remote, &downloaded)?;
            reporter.phase("Download", started.elapsed());
            options.source_sha256 = Some(sha256_file(&downloaded)?);
            downloaded
        } else {
            let OpenApiInput::Path(source) = source else {
                unreachable!("remote inputs were handled above")
            };
            let source = std::fs::canonicalize(source)
                .with_context(|| format!("cannot read OpenAPI source {}", source.display()))?;
            if !source.is_file() {
                bail!("OpenAPI source must be a file")
            }
            options.source_sha256 = Some(sha256_file(&source)?);
            source
        };
        let helper = compiler_path(options.compiler.clone())?;
        reporter.compiling(&source);
        let started = Instant::now();
        let mut compiler = Command::new(&helper);
        compiler.arg("--out").arg(temporary.path());
        if let Some(source_url) = &compiler_source_url {
            compiler.arg("--source-url").arg(source_url);
        }
        let status = compiler
            .arg(&source)
            .status()
            .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
        if !status.success() {
            bail!("OpenAPI compiler failed ({status}); no SDK files were written")
        }
        reporter.phase("OpenAPI", started.elapsed());
        // The compiler hashes the complete local reference closure. A root-only
        // digest would miss changes in referenced files in generation provenance.
        let source_manifest = temporary.path().join("source.json");
        if source_manifest.is_file() {
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&source_manifest).context("read compiler source manifest")?,
            )
            .context("decode compiler source manifest")?;
            let digest = manifest
                .get("sha256")
                .and_then(serde_json::Value::as_str)
                .filter(|value| {
                    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .context("compiler source manifest must contain a SHA-256 digest")?;
            options.source_sha256 = Some(digest.to_owned());
        }
        temporary.path()
    };
    write_sdk(artifacts, &options, &reporter)
}

fn write_sdk(artifacts: &Path, options: &Generate, reporter: &Reporter) -> Result<()> {
    let started = Instant::now();
    let api = poolster_core::adapter::openapi_sidecar::load_operations(
        artifacts,
        options.name.clone(),
        options.version.clone(),
    )?;
    if let Some(report) = api.annotations.get("poolster.vendor.report") {
        if let Some(manual) = report.get("manual").and_then(serde_json::Value::as_array) {
            for diagnostic in manual {
                if let Some(message) = diagnostic.as_str() {
                    eprintln!("poolster compatibility: {message}");
                }
            }
        }
    }
    let api = slice_api_paths(api, &options.path_selection)?;
    let security_schemes =
        poolster_core::adapter::openapi_sidecar::load_security_schemes(artifacts)?;
    let mut tree = poolster::generate_with_security_catalog(
        &api,
        profiles(options)?,
        Some(&security_schemes),
    )?;
    if let Some(packages) = &options.config_packages {
        append_config_artifacts(&api, &mut tree, packages)?;
        add_typescript_artifact_dependencies(&mut tree, packages)?;
        for package in packages {
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
            .flat_map(|package| {
                package
                    .resolved_customizations
                    .iter()
                    .map(|code| code.prefixed(Path::new(&package.path)))
            })
            .collect::<Vec<_>>();
        poolster_core::customization::apply_code_customizations(&mut tree, &customizations)?;
    }
    reporter.generated(options, started.elapsed());
    let started = Instant::now();
    tree.insert(GeneratedFile::new(
        GENERATION_LOCK_PATH,
        generation_lock(artifacts, options, &api)?,
    )?)?;
    let changes = tree.check(&options.output)?;
    if options.json_changes {
        println!("{}", serde_json::to_string(&changes)?);
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
        if !changes.is_empty() {
            bail!("generated output has drift");
        }
        return Ok(());
    }
    tree.write_to(&options.output)?;
    reporter.phase("Writing files", started.elapsed());
    reporter.completed(
        configured_plugin_count(options),
        tree.iter().count() + 1,
        &options.output,
    );
    Ok(())
}
