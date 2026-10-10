use super::*;
#[test]
fn paths_and_refs_are_restricted() {
    assert!(allowed(Path::new(
        ".github/actions/poolster-check/check.mjs"
    )));
    for path in [
        "../escape",
        ".github/workflows/other.yml",
        ".github/actions/evil/action.yml",
        ".git/config",
        "README.md",
    ] {
        assert!(!allowed(Path::new(path)));
    }
    assert!(validate("acme/sdk", "main", "codex/setup").is_ok());
    assert!(validate("acme/sdk", "main", "main").is_err());
    assert!(validate("acme/sdk", "main", "--evil").is_err());
}
#[test]
fn native_staged_action_paths_install_and_inventory_keys_are_portable() -> Result<()> {
    let source = tempfile::tempdir()?;
    let relative = Path::new(".github")
        .join("actions")
        .join("poolster-check")
        .join("action.yml");
    assert!(allowed(&relative));
    assert!(!allowed(
        &Path::new(".github")
            .join("actions")
            .join("poolster-check")
            .join(".git")
            .join("config")
    ));
    let action = source.path().join(&relative);
    fs::create_dir_all(action.parent().unwrap())?;
    fs::write(action, "name: Check\n")?;
    let files = staged(source.path())?;
    assert_eq!(
        files.get(&relative).map(String::as_str),
        Some("name: Check\n")
    );
    let destination = tempfile::tempdir()?;
    let writes = updates(destination.path(), files)?;
    let inventory: serde_json::Value =
        serde_json::from_str(writes.get(Path::new(INVENTORY)).unwrap())?;
    assert_eq!(
        inventory[".github/actions/poolster-check/action.yml"],
        "name: Check\n"
    );
    install(source.path(), "acme/sdk", "main", "sdk-setup", true)?;
    Ok(())
}

#[test]
fn edits_are_protected_and_versions_preserved() -> Result<()> {
    let root = tempfile::tempdir()?;
    let path = PathBuf::from("release-please-config.json");
    fs::write(root.path().join(&path), "manual")?;
    assert!(updates(root.path(), Files::from([(path.clone(), "new".into())])).is_err());
    fs::create_dir(root.path().join(".poolster"))?;
    fs::write(
        root.path().join(INVENTORY),
        serde_json::to_vec(&Files::from([(path.clone(), "manual".into())]))?,
    )?;
    fs::write(root.path().join(MANIFEST), "{\".\":\"2.0.0\"}")?;
    let writes = updates(
        root.path(),
        Files::from([
            (path.clone(), "new".into()),
            (PathBuf::from(MANIFEST), "{\".\":\"0.1.0\"}".into()),
        ]),
    )?;
    assert_eq!(writes.get(&path).unwrap(), "new");
    assert!(!writes.contains_key(Path::new(MANIFEST)));
    Ok(())
}
#[test]
fn staged_setup_is_read_only_and_rejects_unknown_files() -> Result<()> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("release-please-config.json"), "{}")?;
    assert_eq!(staged(root.path())?.len(), 1);
    install(root.path(), "acme/sdk", "main", "codex/setup", true)?;
    assert!(!root.path().join(INVENTORY).exists());
    fs::write(root.path().join("unexpected"), "no")?;
    assert!(staged(root.path()).is_err());
    Ok(())
}
#[test]
fn existing_branch_keeps_its_commit_ancestry() -> Result<()> {
    let root = tempfile::tempdir()?;
    let config = root.path().join("config");
    fs::write(&config, "")?;
    capture(
        root.path(),
        &config,
        "git",
        &["init", "--initial-branch=main", "repository"],
    )?;
    let repository = root.path().join("repository");
    fs::write(repository.join("README.md"), "existing")?;
    capture(&repository, &config, "git", &["add", "README.md"])?;
    let commit = |message: &str| {
        capture(
            &repository,
            &config,
            "git",
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                message,
            ],
        )
    };
    commit("existing main")?;
    capture(
        &repository,
        &config,
        "git",
        &["checkout", "-b", "codex/setup"],
    )?;
    fs::write(repository.join("customer.txt"), "preserved branch work")?;
    capture(&repository, &config, "git", &["add", "customer.txt"])?;
    commit("existing branch work")?;
    let previous = capture(&repository, &config, "git", &["rev-parse", "HEAD"])?;
    let writes = updates(
        &repository,
        Files::from([(PathBuf::from("release-please-config.json"), "{}".into())]),
    )?;
    for (path, contents) in writes {
        let target = safe_path(&repository, &path)?;
        fs::create_dir_all(target.parent().unwrap())?;
        fs::write(target, contents)?;
        capture(
            &repository,
            &config,
            "git",
            &["add", "--", path.to_str().unwrap()],
        )?;
    }
    commit("install automation")?;
    assert_eq!(
        capture(&repository, &config, "git", &["rev-parse", "HEAD^"])?,
        previous
    );
    assert_eq!(
        fs::read_to_string(repository.join("customer.txt"))?,
        "preserved branch work"
    );
    Ok(())
}
#[cfg(unix)]
#[test]
fn symlinks_are_rejected() -> Result<()> {
    let root = tempfile::tempdir()?;
    std::os::unix::fs::symlink(
        "/etc/passwd",
        root.path().join("release-please-config.json"),
    )?;
    assert!(staged(root.path()).is_err());
    Ok(())
}
