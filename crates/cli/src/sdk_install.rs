//! Reviewable installation of staged destination SDK automation.
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

const INVENTORY: &str = ".poolster/sdk-automation.json";
const MANIFEST: &str = ".release-please-manifest.json";
type Files = BTreeMap<PathBuf, String>;

fn allowed(path: &Path) -> bool {
    if path
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
    {
        return false;
    }
    let Ok(value) = crate::sdk_automation::git_tree_path(path) else {
        return false;
    };
    if value.contains('\\')
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| part == ".git")
    {
        return false;
    }
    if matches!(
        value.as_str(),
        INVENTORY
            | MANIFEST
            | "release-please-config.json"
            | ".poolster/SDK_AUTOMATION.md"
            | ".github/workflows/poolster-sdk-ci.yml"
            | ".github/workflows/poolster-sdk-release.yml"
    ) {
        return true;
    }
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() >= 4
        && parts[0] == ".github"
        && parts[1] == "actions"
        && parts[2].starts_with("poolster-")
        && parts[2].len() > 5
}

fn safe_path(root: &Path, path: &Path) -> Result<PathBuf> {
    ensure!(
        allowed(path),
        "unsupported SDK setup path: {}",
        path.display()
    );
    let mut target = root.to_path_buf();
    ensure!(
        !fs::symlink_metadata(root)?.file_type().is_symlink(),
        "setup root must not be a symlink"
    );
    for component in path.components() {
        target.push(component);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "symlink in SDK setup path: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(target)
}

fn staged(root: &Path) -> Result<Files> {
    ensure!(
        fs::symlink_metadata(root)?.is_dir(),
        "setup must be a directory"
    );
    fn walk(root: &Path, directory: &Path, files: &mut Files) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "symlink in setup directory"
            );
            if metadata.is_dir() {
                walk(root, &path, files)?;
            } else {
                ensure!(metadata.is_file(), "setup contains a nonregular file");
                let relative = path.strip_prefix(root)?;
                safe_path(root, relative)?;
                if relative != Path::new(INVENTORY) {
                    files.insert(
                        relative.to_path_buf(),
                        fs::read_to_string(path).context("setup files must be UTF-8")?,
                    );
                }
            }
        }
        Ok(())
    }
    let mut files = Files::new();
    walk(root, root, &mut files)?;
    ensure!(!files.is_empty(), "setup contains no automation files");
    Ok(files)
}

fn updates(root: &Path, files: Files) -> Result<Files> {
    let inventory = safe_path(root, Path::new(INVENTORY))?;
    let mut previous: Files = if inventory.exists() {
        serde_json::from_slice(&fs::read(inventory)?)
            .context("invalid destination SDK ownership inventory")?
    } else {
        Files::new()
    };
    for path in previous.keys() {
        ensure!(allowed(path), "unsafe destination ownership inventory path");
    }
    let mut writes = Files::new();
    for (path, contents) in files {
        let target = safe_path(root, &path)?;
        if target.exists() {
            let existing = fs::read_to_string(target)?;
            if path == Path::new(MANIFEST) {
                continue;
            }
            ensure!(
                existing == contents || previous.get(&path) == Some(&existing),
                "refusing to overwrite edited or unmanaged file: {}",
                path.display()
            );
            if existing == contents {
                previous.insert(path, contents);
                continue;
            }
        }
        previous.insert(path.clone(), contents.clone());
        writes.insert(path, contents);
    }
    writes.insert(
        PathBuf::from(INVENTORY),
        serde_json::to_string_pretty(&crate::sdk_automation::portable_inventory(&previous)?)?
            + "\n",
    );
    Ok(writes)
}

fn capture(cwd: &Path, config: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .with_context(|| format!("could not run {program}"))?;
    ensure!(
        output.status.success(),
        "{program} failed (status {}); inspect authentication and repository permissions",
        output.status
    );
    String::from_utf8(output.stdout).context("command returned non-UTF-8 output")
}

fn validate(repository: &str, base: &str, branch: &str) -> Result<()> {
    let parts = repository.split('/').collect::<Vec<_>>();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|p| !p.is_empty()
                && !matches!(*p, "." | "..")
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))),
        "repository must be OWNER/REPO"
    );
    for value in [base, branch] {
        ensure!(
            !value.is_empty()
                && !value.starts_with('-')
                && !value.ends_with(['/', '.'])
                && !value.contains([' ', '\\', ':'])
                && !value.contains("..")
                && !value.contains("//")
                && value.split('/').all(|p| !p.ends_with(".lock"))
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.')),
            "invalid Git branch name"
        );
    }
    ensure!(
        base != branch,
        "setup branch must differ from the destination base"
    );
    Ok(())
}

/// Install local staged files through a destination pull request. Dry runs are local only.
pub fn install(
    setup: &Path,
    repository: &str,
    base: &str,
    branch: &str,
    dry_run: bool,
) -> Result<()> {
    validate(repository, base, branch)?;
    let files = staged(setup)?;
    if dry_run {
        for path in files.keys() {
            println!(
                "Would install {} into {repository} through {branch}",
                path.display()
            );
        }
        return Ok(());
    }
    let temporary = tempfile::tempdir()?;
    let config = temporary.path().join("gitconfig");
    fs::write(&config, "")?;
    capture(
        temporary.path(),
        &config,
        "gh",
        &["auth", "setup-git", "--hostname", "github.com"],
    )?;
    let checkout = temporary.path().join("repository");
    let url = format!("https://github.com/{repository}.git");
    capture(
        temporary.path(),
        &config,
        "git",
        &["clone", "--", &url, "repository"],
    )?;
    capture(
        &checkout,
        &config,
        "git",
        &["check-ref-format", "--branch", branch],
    )?;
    let reference = format!("refs/heads/{branch}");
    let existing = capture(
        &checkout,
        &config,
        "git",
        &["ls-remote", "--heads", "origin", &reference],
    )?;
    if existing.trim().is_empty() {
        let base_ref = format!("origin/{base}");
        capture(
            &checkout,
            &config,
            "git",
            &["checkout", "-b", branch, &base_ref],
        )?;
    } else {
        capture(&checkout, &config, "git", &["fetch", "origin", &reference])?;
        capture(
            &checkout,
            &config,
            "git",
            &["checkout", "-b", branch, "FETCH_HEAD"],
        )?;
    }
    for (path, contents) in updates(&checkout, files)? {
        let target = safe_path(&checkout, &path)?;
        fs::create_dir_all(target.parent().context("missing parent")?)?;
        fs::write(target, contents)?;
        capture(
            &checkout,
            &config,
            "git",
            &["add", "--", &crate::sdk_automation::git_tree_path(&path)?],
        )?;
    }
    let changes = capture(
        &checkout,
        &config,
        "git",
        &["diff", "--cached", "--name-only"],
    )?;
    if !changes.trim().is_empty() {
        capture(
            &checkout,
            &config,
            "git",
            &[
                "-c",
                "user.name=Poolster SDK setup",
                "-c",
                "user.email=poolster-sdk-setup@users.noreply.github.com",
                "commit",
                "-m",
                "chore: install Poolster SDK automation",
            ],
        )?;
        let push_ref = format!("HEAD:refs/heads/{branch}");
        capture(&checkout, &config, "git", &["push", "origin", &push_ref])?;
    }
    let base_ref = format!("origin/{base}..HEAD");
    if capture(
        &checkout,
        &config,
        "git",
        &["rev-list", "--count", &base_ref],
    )?
    .trim()
        == "0"
    {
        println!("SDK automation is already installed");
        return Ok(());
    }
    let prs: serde_json::Value = serde_json::from_str(&capture(
        &checkout,
        &config,
        "gh",
        &[
            "pr", "list", "--repo", repository, "--base", base, "--head", branch, "--state",
            "open", "--json", "url",
        ],
    )?)?;
    if let Some(url) = prs
        .get(0)
        .and_then(|pr| pr.get("url"))
        .and_then(|v| v.as_str())
    {
        println!("SDK setup PR: {url}");
        return Ok(());
    }
    let body = temporary.path().join("pr.md");
    fs::write(
        &body,
        "Installs staged Poolster SDK build and release automation. Existing release versions are preserved. Review workflow permissions, publishing configuration, and generated action sources before merging.\n",
    )?;
    let url = capture(
        &checkout,
        &config,
        "gh",
        &[
            "pr",
            "create",
            "--repo",
            repository,
            "--base",
            base,
            "--head",
            branch,
            "--title",
            "chore: install Poolster SDK automation",
            "--body-file",
            body.to_str().context("invalid temporary path")?,
        ],
    )?;
    println!("SDK setup PR: {}", url.trim());
    Ok(())
}

#[cfg(test)]
#[path = "sdk_install_tests.rs"]
mod tests;
