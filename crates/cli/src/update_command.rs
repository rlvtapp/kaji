//! Replay direct generation from secret-free inventories.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::{
    ColorChoice, GENERATION_LOCK_PATH, GENERATION_LOCK_VERSION, Generate, GenerationReplayLock,
    OpenApiInput, TypeScriptTransport, Update, UpdateInputLock, UpdateLock, generate, parse_style,
    sha256_directory, sha256_file,
};

pub(super) fn update(options: Update) -> Result<()> {
    let root = std::fs::canonicalize(&options.output)
        .with_context(|| format!("cannot read update output {}", options.output.display()))?;
    if !root.is_dir() {
        bail!("update output must be a directory")
    }
    let mut locks = Vec::new();
    find_generation_locks(&root, &mut locks)?;
    if locks.is_empty() {
        bail!(
            "no {GENERATION_LOCK_PATH} files were found below {}; run poolster generate first",
            root.display()
        )
    }
    let mut updated = 0;
    let mut unchanged = 0;
    let mut unsupported = 0;
    for path in locks {
        let document = std::fs::read_to_string(&path)
            .with_context(|| format!("read generation lock {}", path.display()))?;
        let lock: UpdateLock = serde_json::from_str(&document)
            .with_context(|| format!("parse generation lock {}", path.display()))?;
        if lock.version != GENERATION_LOCK_VERSION {
            bail!(
                "generation lock {} has unsupported version {}; expected {}",
                path.display(),
                lock.version,
                GENERATION_LOCK_VERSION
            )
        }
        let Some(replay) = lock.replay else {
            eprintln!(
                "poolster update: skipping {} (created by config generation; run poolster generate --config instead)",
                path.display()
            );
            unsupported += 1;
            continue;
        };
        let output = path
            .parent()
            .and_then(Path::parent)
            .expect("generation lock is always nested below .poolster")
            .to_path_buf();
        if !options.force && replay_input_is_unchanged(&replay, &lock.input)? {
            println!("Unchanged: {}", output.display());
            unchanged += 1;
            continue;
        }
        println!("Updating: {}", output.display());
        generate(replay_generate_options(replay, output)?)?;
        updated += 1;
    }
    println!(
        "Update complete: {updated} regenerated, {unchanged} unchanged, {unsupported} need their config recipe"
    );
    Ok(())
}

fn find_generation_locks(directory: &Path, locks: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("read update output {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("read update output {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect update path {}", path.display()))?;
        if file_type.is_dir() {
            if entry.file_name() != ".git" {
                find_generation_locks(&path, locks)?;
            }
        } else if file_type.is_file() && path.ends_with(GENERATION_LOCK_PATH) {
            locks.push(path);
        }
    }
    Ok(())
}

pub(super) fn replay_input_is_unchanged(
    replay: &GenerationReplayLock,
    input: &UpdateInputLock,
) -> Result<bool> {
    if let Some(source) = &replay.source {
        let source = Path::new(source);
        // Remote URLs are intentionally re-fetched. Their lock records the
        // downloaded hash, but a local check cannot establish freshness.
        return Ok(source.is_file()
            && input
                .source_sha256
                .as_ref()
                .is_some_and(|hash| sha256_file(source).is_ok_and(|actual| actual == *hash)));
    }
    if let Some(artifacts) = &replay.artifacts {
        let artifacts = Path::new(artifacts);
        return Ok(artifacts.is_dir() && sha256_directory(artifacts)? == input.artifacts_sha256);
    }
    Ok(false)
}

fn replay_generate_options(replay: GenerationReplayLock, output: PathBuf) -> Result<Generate> {
    let source = replay
        .source
        .map(|source| OpenApiInput::Path(PathBuf::from(source)));
    let artifacts = replay.artifacts.map(PathBuf::from);
    if source.is_some() == artifacts.is_some() {
        bail!("generation replay must contain exactly one source or artifact directory")
    }
    let style = parse_style(Some(&replay.client_style))?;
    let typescript_transport = replay
        .typescript_transport
        .as_deref()
        .map(TypeScriptTransport::parse)
        .transpose()?;
    if replay.typescript_surface != "client" && replay.typescript_surface != "raw" {
        bail!("generation replay has invalid TypeScript surface")
    }
    Ok(Generate {
        native_input: None,
        source,
        config: None,
        config_packages: None,
        artifacts,
        output,
        languages: replay.languages,
        name: replay.name,
        version: replay.version,
        style,
        raw: replay.typescript_surface == "raw",
        typescript_transport,
        client_name: replay.typescript_client_name,
        compiler: replay.compiler.map(PathBuf::from),
        path_selection: replay.paths,
        config_sha256: None,
        source_sha256: None,
        jobs: replay.go_jobs.unwrap_or_default(),
        color: ColorChoice::Auto,
        check: false,
        json_changes: false,
    })
}
