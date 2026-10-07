//! Import existing generator projects without executing their hooks or publishing.
use super::*;
use serde_json::{Value, json};

const CONFIGS: &[&str] = &[
    "stainless.yml",
    "stainless.yaml",
    "fern/generators.yml",
    "fern/generators.yaml",
    ".speakeasy/workflow.yaml",
    ".speakeasy/workflow.yml",
    "gen.yaml",
    ".speakeasy/gen.yaml",
];

fn detect(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return std::fs::canonicalize(path).context("read migration configuration");
    }
    let mut found = CONFIGS
        .iter()
        .map(|name| path.join(name))
        .filter(|p| p.is_file())
        .collect::<Vec<_>>();
    if found.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == "workflow.yaml" || name == "workflow.yml")
    }) {
        found.retain(|path| path.file_name().is_none_or(|name| name != "gen.yaml"));
    }
    ensure!(
        found.len() == 1,
        "expected one vendor config in {}; found {}. Pass its filename explicitly",
        path.display(),
        found.len()
    );
    std::fs::canonicalize(&found[0]).context("read migration configuration")
}
fn read_document(path: &Path, bundle: bool) -> Result<Value> {
    let output = Command::new(compiler_path(None)?)
        .arg(if bundle {
            "--export-openapi"
        } else {
            "--read-document"
        })
        .arg(path)
        .output()
        .context("run configuration reader")?;
    ensure!(
        output.status.success(),
        "read {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).context("read compiler JSON document")
}
fn language(name: &str) -> Option<&str> {
    let name = name
        .strip_prefix("fern-")
        .unwrap_or(name)
        .trim_end_matches("-sdk");
    match name {
        "typescript" | "python" | "go" | "rust" | "ruby" | "php" | "java" | "csharp" | "swift"
        | "elixir" => Some(name),
        _ => None,
    }
}
fn add_package(
    packages: &mut Vec<Value>,
    lang: &str,
    name: Option<&str>,
    report: &mut kaji_core::vendor::MigrationReport,
) {
    if let Some(lang) = language(lang) {
        let mut package = json!({"language":lang,"path":format!("{lang}/{}",packages.len()+1),"plugins":[{"name":"sdk"}]});
        if let Some(name) = name {
            package["name"] = json!(name);
        }
        packages.push(package);
    } else {
        report
            .manual
            .push(format!("unsupported generator target {lang:?}"));
    }
}
fn scan_settings(
    value: &Value,
    prefix: &str,
    allowed: &[&str],
    report: &mut kaji_core::vendor::MigrationReport,
) {
    if let Some(object) = value.as_object() {
        for key in object.keys().filter(|key| !allowed.contains(&key.as_str())) {
            report.manual.push(format!(
                "{prefix}.{key}: review in Kaji; setting was not imported"
            ));
        }
    }
}
fn candidates(base: &Path) -> Vec<PathBuf> {
    [
        "openapi.yaml",
        "openapi.yml",
        "openapi.json",
        "api.yaml",
        "api.yml",
    ]
    .iter()
    .map(|name| base.join(name))
    .filter(|p| p.is_file())
    .collect()
}
fn stainless_resources(
    resources: &Value,
    document: &mut Value,
    prefix: &str,
    report: &mut kaji_core::vendor::MigrationReport,
) {
    if let Some(resources) = resources.as_object() {
        for (name, resource) in resources {
            let group = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            if let Some(methods) = resource.get("methods").and_then(Value::as_object) {
                for (name, binding) in methods {
                    if let Some(binding) = binding.as_str() {
                        if let Some((method, path)) = binding.split_once(' ') {
                            if let Some(operation) = document
                                .get_mut("paths")
                                .and_then(|paths| paths.get_mut(path))
                                .and_then(|item| item.get_mut(method.to_ascii_lowercase()))
                            {
                                operation["x-stainless-method"] = json!(format!("{group}.{name}"));
                                continue;
                            }
                        }
                    }
                    report.manual.push(format!(
                        "resources.{group}.methods.{name}: unsupported or missing endpoint binding"
                    ));
                }
            }
            if let Some(children) = resource.get("subresources") {
                stainless_resources(children, document, &group, report);
            }
            scan_settings(
                resource,
                &format!("resources.{group}"),
                &["methods", "subresources"],
                report,
            );
        }
    }
}

fn import(
    path: &Path,
    input: Option<&Path>,
    destination: &Path,
    strict: bool,
    announce: bool,
) -> Result<PathBuf> {
    ensure!(
        !destination.exists(),
        "migration output {} already exists; choose a new directory",
        destination.display()
    );
    let config_path = detect(path)?;
    let base = config_path.parent().context("config has no parent")?;
    let config = read_document(&config_path, false)?;
    let mut report = kaji_core::vendor::MigrationReport::default();
    let mut packages = Vec::new();
    let mut sources = Vec::new();
    let vendor;
    if config.get("groups").is_some() {
        vendor = "fern";
        if let Some(specs) = config.pointer("/api/specs").and_then(Value::as_array) {
            for spec in specs {
                if let Some(source) = spec.get("openapi").and_then(Value::as_str) {
                    sources.push(base.join(source));
                }
                scan_settings(spec, "api.specs", &["openapi"], &mut report);
            }
        }
        if let Some(groups) = config.get("groups").and_then(Value::as_object) {
            for (group, settings) in groups {
                if let Some(generators) = settings.get("generators").and_then(Value::as_array) {
                    for generator in generators {
                        if let Some(name) = generator.get("name").and_then(Value::as_str) {
                            add_package(
                                &mut packages,
                                name,
                                generator
                                    .pointer("/output/package-name")
                                    .and_then(Value::as_str),
                                &mut report,
                            );
                        } else {
                            report.manual.push(format!(
                                "groups.{group}: custom generator needs a Kaji plugin"
                            ));
                        }
                        scan_settings(
                            generator,
                            &format!("groups.{group}.generators"),
                            &["name", "version", "output"],
                            &mut report,
                        );
                        if generator.get("output").is_some() {
                            report.manual.push(format!("groups.{group}.output: package name imported; configure publishing with kaji sdk init"));
                        }
                    }
                }
                scan_settings(
                    settings,
                    &format!("groups.{group}"),
                    &["generators"],
                    &mut report,
                );
            }
        }
        scan_settings(
            &config,
            "config",
            &["api", "groups", "default-group"],
            &mut report,
        );
        if let Some(api) = config.get("api") {
            scan_settings(api, "api", &["specs"], &mut report);
        }
    } else if config.get("sources").is_some() && config.get("targets").is_some() {
        vendor = "speakeasy";
        let project = if base.file_name().is_some_and(|n| n == ".speakeasy") {
            base.parent().unwrap_or(base)
        } else {
            base
        };
        if let Some(targets) = config.get("targets").and_then(Value::as_object) {
            for (name, target) in targets {
                if let Some(lang) = target.get("target").and_then(Value::as_str) {
                    add_package(&mut packages, lang, None, &mut report);
                }
                if let Some(source_name) = target.get("source").and_then(Value::as_str) {
                    if let Some(source) = config.get("sources").and_then(|s| s.get(source_name)) {
                        if let Some(inputs) = source.get("inputs").and_then(Value::as_array) {
                            for item in inputs {
                                if let Some(location) = item.get("location").and_then(Value::as_str)
                                {
                                    sources.push(project.join(location));
                                }
                                scan_settings(
                                    item,
                                    &format!("sources.{source_name}.inputs"),
                                    &["location"],
                                    &mut report,
                                );
                            }
                        }
                        scan_settings(
                            source,
                            &format!("sources.{source_name}"),
                            &["inputs"],
                            &mut report,
                        );
                    }
                }
                scan_settings(
                    target,
                    &format!("targets.{name}"),
                    &["target", "source"],
                    &mut report,
                );
            }
        }
        scan_settings(
            &config,
            "config",
            &["sources", "targets", "workflowVersion", "speakeasyVersion"],
            &mut report,
        );
    } else if config.get("resources").is_some()
        || config.get("organization").is_some()
        || config.get("targets").is_some()
    {
        vendor = "stainless";
        if let Some(targets) = config.get("targets").and_then(Value::as_object) {
            for (lang, target) in targets {
                add_package(
                    &mut packages,
                    lang,
                    target.get("package_name").and_then(Value::as_str),
                    &mut report,
                );
                scan_settings(
                    target,
                    &format!("targets.{lang}"),
                    &["package_name"],
                    &mut report,
                );
            }
        }
        scan_settings(
            &config,
            "config",
            &["resources", "targets", "organization"],
            &mut report,
        );
        if config.get("organization").is_some() {
            report
                .manual
                .push("organization: review branding in Kaji configuration".into());
        }
    } else if config.get("configVersion").is_some() {
        vendor = "speakeasy";
        for lang in LANGUAGES {
            if config.get(*lang).is_some() {
                add_package(
                    &mut packages,
                    lang,
                    config
                        .get(*lang)
                        .and_then(|s| s.get("packageName"))
                        .and_then(Value::as_str),
                    &mut report,
                );
                scan_settings(&config[*lang], lang, &["packageName"], &mut report);
            }
        }
        report.manual.push("gen.yaml: generation settings require review; pass --input if the OpenAPI file is elsewhere".into());
    } else {
        bail!(
            "unrecognized generator configuration {}; expected Stainless, Fern or Speakeasy",
            config_path.display()
        );
    }
    sources.sort();
    sources.dedup();
    let source = if let Some(input) = input {
        std::fs::canonicalize(input)?
    } else {
        if sources.is_empty() {
            sources = candidates(base);
            if sources.is_empty() && base.file_name().is_some_and(|n| n == ".speakeasy") {
                sources = candidates(base.parent().unwrap_or(base));
            }
        }
        ensure!(
            sources.len() == 1 && sources[0].is_file(),
            "cannot resolve one local OpenAPI source; pass --input <file> (remote sources, merged specs and overlays must be exported first)"
        );
        sources.remove(0)
    };
    ensure!(
        !packages.is_empty(),
        "no supported SDK targets found in vendor config"
    );
    let mut document = read_document(&source, true)?;
    if vendor == "stainless" {
        if let Some(resources) = config.get("resources") {
            stainless_resources(resources, &mut document, "", &mut report);
        }
    }
    let annotations = kaji_core::vendor::normalize_openapi(&mut document);
    report.converted.extend(annotations.converted);
    report.manual.extend(annotations.manual);
    report.converted.push(format!(
        "{vendor}: imported {} SDK target(s)",
        packages.len()
    ));
    let native = json!({"openapi":{"input":"./openapi.json","name":document.pointer("/info/title").and_then(Value::as_str).unwrap_or("API"),"version":default_sdk_version()},"output":{"path":"./generated"},"packages":packages});
    let _: ProjectConfig =
        serde_json::from_value(native.clone()).context("validate imported Kaji config")?;
    if strict {
        ensure!(
            report.manual.is_empty(),
            "migration needs manual review: {}",
            report.manual.join("; ")
        );
    }
    // Stage the complete result so errors leave no partial destination.
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    for (name, value) in [
        ("kaji.json", native),
        ("openapi.json", document),
        ("migration-report.json", serde_json::to_value(&report)?),
    ] {
        std::fs::write(
            staging.path().join(name),
            serde_json::to_vec_pretty(&value)?,
        )?;
    }
    std::fs::rename(staging.path(), destination)?;
    if announce {
        eprintln!(
            "Imported {vendor} project to {} ({} setting(s) need review). Originals preserved.",
            destination.display(),
            report.manual.len()
        );
        eprintln!(
            "Next: kaji generate --config {}",
            destination.join("kaji.json").display()
        );
    }
    Ok(destination.join("kaji.json"))
}

pub fn run(arguments: Vec<OsString>) -> Result<()> {
    let mut args = arguments.into_iter();
    let mut project = PathBuf::from(".");
    let mut input = None;
    let mut output = PathBuf::from("kaji-migration");
    let mut strict = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--input") => {
                input = Some(PathBuf::from(
                    args.next().context("--input requires a file")?,
                ))
            }
            Some("--output") => {
                output = PathBuf::from(args.next().context("--output requires a directory")?)
            }
            Some("--strict") => strict = true,
            Some("--help" | "-h") => {
                println!(
                    "kaji migrate [project-or-config] [--input openapi-file] [--output new-directory] [--strict]"
                );
                return Ok(());
            }
            Some(value) if !value.starts_with('-') => project = PathBuf::from(arg),
            _ => bail!("unknown migration argument {:?}", arg),
        }
    }
    import(&project, input.as_deref(), &output, strict, true).map(|_| ())
}

pub fn generate(path: &Path, color: ColorChoice, check: bool, json_changes: bool) -> Result<()> {
    let config = detect(path)?;
    let root = config.parent().context("vendor config has no parent")?;
    let project = if root
        .file_name()
        .is_some_and(|name| name == "fern" || name == ".speakeasy")
    {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let temporary = tempfile::tempdir()?;
    let imported = import(
        &config,
        None,
        &temporary.path().join("import"),
        false,
        false,
    )?;
    let mut native: Value = serde_json::from_slice(&std::fs::read(&imported)?)?;
    native["output"]["path"] = json!(project.join("kaji-generated"));
    std::fs::write(&imported, serde_json::to_vec_pretty(&native)?)?;
    let report: kaji_core::vendor::MigrationReport = serde_json::from_slice(&std::fs::read(
        imported.parent().unwrap().join("migration-report.json"),
    )?)?;
    for diagnostic in report.manual {
        eprintln!("kaji migration: {diagnostic}");
    }
    super::generate_from_config(&imported, color, check, json_changes)
}
