use std::fs;

use super::{GeneratedFile, GeneratedTree, OWNERSHIP_PATH};

#[test]
fn skipped_package_preserves_bytes_and_prior_ownership_without_adopting_edits() {
    let root = tempfile::tempdir().unwrap();
    let mut initial = GeneratedTree::default();
    initial
        .insert(GeneratedFile::new("skipped/model.ts", "original").unwrap())
        .unwrap();
    initial
        .set_owner("skipped/model.ts", "original-owner")
        .unwrap();
    initial
        .insert(GeneratedFile::new("active/model.ts", "old").unwrap())
        .unwrap();
    initial.write_to(root.path()).unwrap();
    let previous: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.path().join(OWNERSHIP_PATH)).unwrap())
            .unwrap();
    fs::write(root.path().join("skipped/model.ts"), "local edit").unwrap();
    let mut next = GeneratedTree::default();
    next.insert(GeneratedFile::new("active/model.ts", "new").unwrap())
        .unwrap();
    next.preserve_owned_prefix(root.path(), "skipped").unwrap();
    assert!(next.check(root.path()).unwrap().removed.is_empty());
    next.write_to(root.path()).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("skipped/model.ts")).unwrap(),
        "local edit"
    );
    let current: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.path().join(OWNERSHIP_PATH)).unwrap())
            .unwrap();
    assert_eq!(
        previous["files"]["skipped/model.ts"],
        current["files"]["skipped/model.ts"]
    );
    assert!(next.preserve_owned_prefix(root.path(), "active").is_err());
    assert!(
        next.preserve_owned_prefix(root.path(), "../outside")
            .is_err()
    );
}

#[test]
fn ownership_checks_drift_removes_stale_files_and_preserves_custom() {
    let root = tempfile::tempdir().unwrap();
    let mut first = GeneratedTree::default();
    first
        .insert(GeneratedFile::new("old.ts", "old").unwrap())
        .unwrap();
    first
        .insert_custom(GeneratedFile::new("custom.ts", "starter").unwrap())
        .unwrap();
    first.write_to(root.path()).unwrap();
    fs::write(root.path().join("custom.ts"), "user").unwrap();
    let mut next = GeneratedTree::default();
    next.insert(GeneratedFile::new("new.ts", "new").unwrap())
        .unwrap();
    let changes = next.check(root.path()).unwrap();
    assert_eq!(changes.added, vec![std::path::PathBuf::from("new.ts")]);
    assert_eq!(changes.removed, vec![std::path::PathBuf::from("old.ts")]);
    assert!(!root.path().join("new.ts").exists());
    next.write_to(root.path()).unwrap();
    assert!(!root.path().join("old.ts").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("custom.ts")).unwrap(),
        "user"
    );
    assert!(next.check(root.path()).unwrap().is_empty());
    fs::write(root.path().join("new.ts"), "local edit").unwrap();
    assert!(next.write_to(root.path()).is_err());
}

#[test]
fn check_refuses_unowned_collisions_and_malicious_manifest_without_writes() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("handwritten.ts"), "handwritten").unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("handwritten.ts", "generated").unwrap())
        .unwrap();
    assert!(tree.check(root.path()).is_err());
    fs::create_dir(root.path().join(".poolster")).unwrap();
    fs::write(
        root.path().join(super::OWNERSHIP_PATH),
        r#"{"version":1,"files":{"../outside":{"owner":"x","sha256":"x"}}}"#,
    )
    .unwrap();
    assert!(tree.check(root.path()).is_err());
    assert_eq!(
        fs::read_to_string(root.path().join("handwritten.ts")).unwrap(),
        "handwritten"
    );
}

#[cfg(unix)]
#[test]
fn check_and_write_reject_symlinks_without_touching_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("linked")).unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("linked/a.ts", "generated").unwrap())
        .unwrap();
    assert!(tree.check(root.path()).is_err());
    assert!(tree.write_to(root.path()).is_err());
    assert!(!outside.path().join("a.ts").exists());
}

#[cfg(unix)]
#[test]
fn missing_root_under_symlink_ancestor_is_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("linked")).unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("file.ts", "code").unwrap())
        .unwrap();
    let missing_root = root.path().join("linked/missing/output");
    assert!(tree.check(&missing_root).is_err());
    assert!(tree.write_to(&missing_root).is_err());
    assert!(!outside.path().join("missing").exists());
}

#[test]
fn npm_manifest_preserves_custom_settings_and_dependency_categories() {
    let existing = r#"{
            "name":"old", "version":"0.0.1", "private":true,
            "scripts":{"build":"custom-build", "test":"test-command"},
            "devDependencies":{"typescript":"5.9.3"},
            "peerDependencies":{"@tanstack/react-query":"^5.100.0"},
            "peerDependenciesMeta":{"@tanstack/react-query":{"optional":true}},
            "exports":{"./custom":"./custom.js", ".":"./old.js"},
            "files":["custom.js"], "repository":{"url":"custom"}
        }"#;
    let generated = r#"{
            "name":"@relevate/sdk", "version":"0.2.0", "type":"module",
            "scripts":{"build":"tsc", "generate":"poolster generate"},
            "devDependencies":{"typescript":"^7.0.0"},
            "dependencies":{"@tanstack/react-query":"^5.0.0", "zod":"^4.0.0"},
            "exports":{".":{"import":"./dist/index.js"}}, "files":["dist"]
        }"#;
    let merged = super::merge_npm_manifest(existing, generated).unwrap();
    let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(value["name"], "@relevate/sdk");
    assert_eq!(value["version"], "0.2.0");
    assert_eq!(value["scripts"]["build"], "custom-build");
    assert_eq!(value["scripts"]["generate"], "poolster generate");
    assert_eq!(value["devDependencies"]["typescript"], "5.9.3");
    assert!(value["dependencies"].get("@tanstack/react-query").is_none());
    assert_eq!(
        value["peerDependencies"]["@tanstack/react-query"],
        "^5.100.0"
    );
    assert_eq!(value["dependencies"]["zod"], "^4.0.0");
    assert_eq!(value["exports"]["./custom"], "./custom.js");
    assert_eq!(value["exports"]["."]["import"], "./dist/index.js");
    assert_eq!(value["files"], serde_json::json!(["custom.js", "dist"]));
    assert_eq!(value["repository"]["url"], "custom");
    assert_eq!(
        super::merge_npm_manifest(&merged, generated).unwrap(),
        merged
    );
}

#[test]
fn fresh_npm_manifest_matches_repeat_bytes_and_preserves_user_additions() {
    let root = tempfile::tempdir().unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(
        GeneratedFile::new(
            "ts/package.json",
            r#"{
  "name": "probe", "version": "1.0.0", "files": ["dist", "README.md"],
  "scripts": {"build": "tsc"}, "dependencies": {"commander": "^13"}
}"#,
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    let path = root.path().join("ts/package.json");
    let fresh = fs::read_to_string(&path).unwrap();
    assert!(tree.check(root.path()).unwrap().is_empty());
    tree.write_to(root.path()).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), fresh);
    let mut edited: serde_json::Value = serde_json::from_str(&fresh).unwrap();
    edited["scripts"]["test"] = serde_json::json!("customer-test");
    edited["dependencies"]["commander"] = serde_json::json!("^14");
    edited["files"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("custom.js"));
    fs::write(&path, serde_json::to_string(&edited).unwrap()).unwrap();
    tree.write_to(root.path()).unwrap();
    let merged = fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(value["scripts"]["test"], "customer-test");
    assert_eq!(value["dependencies"]["commander"], "^14");
    assert!(
        value["files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("custom.js"))
    );
    tree.write_to(root.path()).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), merged);
    assert!(tree.check(root.path()).unwrap().is_empty());
}

#[test]
fn npm_manifest_is_merged_when_materialized() {
    let output = tempfile::tempdir().unwrap();
    fs::create_dir(output.path().join("ts")).unwrap();
    fs::write(
        output.path().join("ts/package.json"),
        r#"{"scripts":{"test":"test"}}"#,
    )
    .unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(
        GeneratedFile::new(
            "ts/package.json",
            r#"{"name":"@scope/sdk","scripts":{"build":"tsc"}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(output.path()).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.path().join("ts/package.json")).unwrap())
            .unwrap();
    assert_eq!(value["scripts"]["test"], "test");
    assert_eq!(value["scripts"]["build"], "tsc");
}

#[test]
fn invalid_npm_manifest_fails_before_writing_generated_files() {
    for invalid in [
        "invalid JSON",
        "[]",
        r#"{"dependencies":[]}"#,
        r#"{"dependencies":{"react":42}}"#,
    ] {
        let output = tempfile::tempdir().unwrap();
        fs::write(output.path().join("package.json"), invalid).unwrap();
        let mut tree = GeneratedTree::default();
        tree.insert(GeneratedFile::new("a.ts", "new code").unwrap())
            .unwrap();
        tree.insert(GeneratedFile::new("package.json", "{}").unwrap())
            .unwrap();
        assert!(tree.write_to(output.path()).is_err());
        assert!(!output.path().join("a.ts").exists());
        assert_eq!(
            fs::read_to_string(output.path().join("package.json")).unwrap(),
            invalid
        );
    }
}

#[test]
fn append_merges_isolated_trees_and_rejects_collisions() {
    let mut left = GeneratedTree::default();
    left.insert(GeneratedFile::new("rust/lib.rs", "left").unwrap())
        .unwrap();
    let mut right = GeneratedTree::default();
    right
        .insert(GeneratedFile::new("python/__init__.py", "right").unwrap())
        .unwrap();
    left.append(right).unwrap();
    assert_eq!(left.get("python/__init__.py"), Some("right"));

    let mut collision = GeneratedTree::default();
    collision
        .insert(GeneratedFile::new("rust/lib.rs", "other").unwrap())
        .unwrap();
    assert!(left.append(collision).is_err());
}

#[test]
fn custom_files_are_created_once_and_preserved_on_regeneration() {
    let output = tempfile::tempdir().unwrap();
    let path = "typescript/custom/index.ts";
    let mut first = GeneratedTree::default();
    first
        .insert_custom(GeneratedFile::new(path, "export const first = true\n").unwrap())
        .unwrap();
    first.write_to(output.path()).unwrap();
    fs::write(output.path().join(path), "export const userOwned = true\n").unwrap();

    let mut regenerated = GeneratedTree::default();
    regenerated
        .insert_custom(GeneratedFile::new(path, "export const replacement = true\n").unwrap())
        .unwrap();
    regenerated.write_to(output.path()).unwrap();

    assert_eq!(
        fs::read_to_string(output.path().join(path)).unwrap(),
        "export const userOwned = true\n"
    );
}

#[test]
fn replace_updates_an_owned_file_without_creating_new_ownership() {
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("typescript/package.json", "{}\n").unwrap())
        .unwrap();
    tree.replace(GeneratedFile::new("typescript/package.json", "{\"name\":\"sdk\"}\n").unwrap())
        .unwrap();
    assert_eq!(
        tree.get("typescript/package.json"),
        Some("{\"name\":\"sdk\"}\n")
    );
    assert!(
        tree.replace(GeneratedFile::new("typescript/missing.json", "{}\n").unwrap())
            .is_err()
    );
}

#[test]
fn create_once_ownership_is_stable_without_adopting_author_edits() {
    let root = tempfile::tempdir().unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert_custom(GeneratedFile::new("extension.ts", "export {};\n").unwrap())
        .unwrap();
    tree.set_owner("extension.ts", "sdk:extension").unwrap();
    tree.write_to(root.path()).unwrap();
    let first = fs::read_to_string(root.path().join(OWNERSHIP_PATH)).unwrap();
    tree.write_to(root.path()).unwrap();
    assert_eq!(
        first,
        fs::read_to_string(root.path().join(OWNERSHIP_PATH)).unwrap()
    );
    fs::write(
        root.path().join("extension.ts"),
        "export const customized = true;\n",
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    assert_eq!(
        first,
        fs::read_to_string(root.path().join(OWNERSHIP_PATH)).unwrap()
    );
    assert!(
        fs::read_to_string(root.path().join("extension.ts"))
            .unwrap()
            .contains("customized")
    );
}
