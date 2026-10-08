use super::*;

pub(super) fn config_path(base: &Path, value: PathBuf) -> PathBuf {
    if value.is_absolute() {
        value
    } else {
        base.join(value)
    }
}

fn resolve_config_input(base: &Path, input: OpenApiInput) -> OpenApiInput {
    match input {
        OpenApiInput::Path(path) if remote_spec_url(&path).is_some() => OpenApiInput::Path(path),
        OpenApiInput::Path(path) => OpenApiInput::Path(config_path(base, path)),
        OpenApiInput::Remote(remote) => OpenApiInput::Remote(remote),
    }
}

pub(super) fn generate_from_config(
    path: &Path,
    color: ColorChoice,
    check: bool,
    json_changes: bool,
) -> Result<()> {
    if path
        .extension()
        .is_some_and(|extension| extension == "yml" || extension == "yaml")
        || !path.exists() && path == Path::new("poolster.json")
    {
        return migration::generate(
            if path.exists() { path } else { Path::new(".") },
            color,
            check,
            json_changes,
        );
    }
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("cannot read Poolster config {}", path.display()))?;
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("read Poolster config {}", path.display()))?;
    let mut config: ProjectConfig = serde_json::from_str(&source)
        .with_context(|| format!("parse Poolster JSON config {}", path.display()))?;
    let base = path.parent().expect("config path has parent");
    if config.output.path.as_os_str().is_empty() {
        bail!("output.path cannot be empty");
    }
    let has_input = config.openapi.input.is_some();
    let has_artifacts = config.openapi.artifacts.is_some();
    if has_input == has_artifacts {
        bail!("openapi must set exactly one of input or artifacts");
    }
    validate_path_selection(&config.openapi.paths)?;
    config_defaults::apply(&mut config)?;
    let style = parse_style(config.defaults.client_style.as_deref())?;
    let output = config_path(base, config.output.path.clone());
    for package in &mut config.packages {
        GeneratedFile::new(&package.path, "")?;
        package.resolved_customizations = package
            .customizations
            .iter()
            .map(|code| code.load(base))
            .collect::<Result<Vec<_>>>()?;
        package.resolved_middleware = package
            .middleware
            .iter()
            .map(|middleware| middleware.load(base))
            .collect::<Result<Vec<_>>>()?;
        if package.release.is_some() && package.version.is_none() {
            let metadata_path = output
                .join(&package.path)
                .join(poolster_core::release::PACKAGE_METADATA_PATH);
            if metadata_path.exists() {
                let canonical_root = std::fs::canonicalize(&output)?;
                let canonical_metadata = std::fs::canonicalize(&metadata_path)?;
                if !canonical_metadata.starts_with(&canonical_root) {
                    bail!("release metadata escapes output root");
                }
                let previous: poolster_core::release::PackageMetadata =
                    serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
                previous.validate()?;
                package.version = Some(previous.version);
            } else if let Some(release) = &package.release {
                if !release.version.is_empty() {
                    package.version = Some(release.version.clone());
                }
            }
        }
    }
    let options = Generate {
        source: config
            .openapi
            .input
            .map(|input| resolve_config_input(base, input)),
        artifacts: config
            .openapi
            .artifacts
            .map(|artifacts| config_path(base, artifacts)),
        config: None,
        config_packages: Some(config.packages),
        output: config_path(base, config.output.path),
        languages: Vec::new(),
        name: config.openapi.name,
        version: config.openapi.version,
        style,
        raw: false,
        typescript_transport: None,
        client_name: None,
        compiler: config
            .openapi
            .compiler
            .map(|compiler| config_path(base, compiler)),
        path_selection: config.openapi.paths,
        config_sha256: Some(sha256(source.as_bytes())),
        source_sha256: None,
        jobs: 0,
        color,
        check,
        json_changes,
    };
    generate(options)
}

pub(super) fn init_config(init: Init) -> Result<()> {
    if init.config.exists() {
        bail!(
            "refusing to overwrite {}; edit it or choose --config <new-file>",
            init.config.display()
        );
    }
    let document = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/poolster.schema.json",
        "openapi": {
            "input": init.input,
            "name": init.name,
            "version": init.version,
        },
        "output": { "path": init.output },
        "defaults": { "client_style": "namespaced" },
        "packages": [
            {
                "language": "typescript",
                "path": "typescript",
                "name": "@acme/api",
                "plugins": [
                    { "name": "sdk", "transport": "fetch", "client_name": "Api" },
                    { "name": "zod" },
                    { "name": "tanstack-react-query" },
                    { "name": "msw" }
                ]
            },
            {
                "language": "go",
                "path": "go",
                "plugins": [{ "name": "sdk", "jobs": 4 }]
            }
        ]
    });
    if let Some(parent) = init
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create config directory {}", parent.display()))?;
    }
    std::fs::write(
        &init.config,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )
    .with_context(|| format!("write {}", init.config.display()))?;
    println!(
        "Created {}. Edit its package plugins, then run poolster generate.",
        init.config.display()
    );
    Ok(())
}
