//! Read-only delivery diagnostics. Never return subprocess output or credential values.
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn check(name: &str, status: &str, action: &str) -> Value {
    json!({"name":name,"status":status,"action":action})
}

/// Diagnose an existing generated output without building, writing or publishing.
pub fn doctor(root: &Path, repository: Option<&str>) -> Result<Value> {
    let mut checks = Vec::new();
    let inventory = root.join(poolster_core::files::OWNERSHIP_PATH);
    let inventory_safe = !root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
        && !inventory
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        && !root
            .join(".poolster")
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
    let ownership = inventory_safe
        .then(|| fs::read(&inventory).ok())
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let valid = ownership
        .as_ref()
        .is_some_and(|value| value["version"] == 1 && value["files"].is_object());
    checks.push(check(
        "ownership",
        if valid { "ready" } else { "needs_action" },
        "Generate into a dedicated output root; retain its .poolster ownership inventory.",
    ));
    let mut packages = Vec::new();
    if let Some(files) = ownership
        .as_ref()
        .filter(|_| valid)
        .and_then(|value| value["files"].as_object())
    {
        for path in files.keys().filter(|path| {
            path.ends_with("/.poolster/package.json") || path.as_str() == ".poolster/package.json"
        }) {
            let relative = Path::new(path);
            let safe = !relative.is_absolute()
                && relative
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
                && relative
                    .ancestors()
                    .filter(|path| !path.as_os_str().is_empty())
                    .all(|path| {
                        !root
                            .join(path)
                            .symlink_metadata()
                            .is_ok_and(|m| m.file_type().is_symlink())
                    });
            let metadata = safe
                .then(|| fs::read(root.join(relative)).ok())
                .flatten()
                .and_then(|bytes| {
                    serde_json::from_slice::<poolster_core::release::PackageMetadata>(&bytes).ok()
                })
                .filter(|metadata| metadata.validate().is_ok());
            match metadata {
                Some(metadata) => packages.push(json!({"path":path,"language":metadata.language,"publisher_configured":metadata.publisher.is_some(),"build_configured":!metadata.build.is_empty(),"test_configured":!metadata.test.is_empty()})),
                None => checks.push(check("package_metadata", "needs_action", "Regenerate valid owned package metadata; symlink and unsafe paths are refused.")),
            }
        }
    }
    checks.push(check(
        "packages",
        if packages.is_empty() {
            "needs_action"
        } else {
            "ready"
        },
        "Configure package release metadata, including native build/test commands and publisher.",
    ));
    for package in &packages {
        if package["publisher_configured"] != true
            || package["build_configured"] != true
            || package["test_configured"] != true
        {
            checks.push(check(
                "package_delivery_commands",
                "needs_action",
                "Add build, test and publisher metadata; doctor never executes package commands.",
            ));
        }
    }
    for (tool, args, action) in [
        (
            "git",
            vec!["--version"],
            "Install Git for isolated SDK repository updates.",
        ),
        (
            "gh",
            vec!["--version"],
            "Install GitHub CLI; configure repository-scoped authentication outside doctor.",
        ),
        (
            "oasdiff",
            vec!["--version"],
            "Install oasdiff for API-aware release sizing, or choose --bump explicitly.",
        ),
    ] {
        let ready = Command::new(tool)
            .args(args)
            .output()
            .is_ok_and(|result| result.status.success());
        checks.push(check(
            tool,
            if ready { "ready" } else { "unavailable" },
            action,
        ));
    }
    let remote = repository
        .map(crate::sdk_status::remote_status)
        .transpose()?;
    if let Some(remote) = &remote {
        let observed = remote["status"] == "observed";
        let required_present = remote["workflows"].as_array().is_some_and(|workflows| {
            workflows.iter().all(|workflow| {
                workflow["optional"] == true || workflow["existence"]["exists"] == true
            })
        });
        checks.push(check("remote_delivery", if observed && required_present {"ready"} else {"needs_action"}, "Review sdk status --remote; install missing workflows and grant read visibility to repository Actions and configuration names."));
    }
    let ready = checks.iter().all(|check| check["status"] == "ready");
    Ok(
        json!({"schema_version":1,"status":if ready {"ready_for_review"} else {"needs_action"},"checks":checks,"packages":packages,"remote":remote,
        "next_steps":["generate --config poolster.json","generate --config poolster.json --check","sdk run --root generated --package PATH --phase build","sdk run --root generated --package PATH --phase test","sdk init --root generated --dry-run","sdk pr --config poolster.json --repository OWNER/REPO --dry-run"],
        "limits":"Tool presence and metadata checks do not verify credential values, registry trust, branch protection, workflow health or publication. No package commands are executed."}),
    )
}

/// Inspect emitted artifact ownership; this is not a plugin execution planner.
pub fn inspect(root: &Path) -> Result<Value> {
    for path in [
        root.to_path_buf(),
        root.join(".poolster"),
        root.join(poolster_core::files::OWNERSHIP_PATH),
    ] {
        anyhow::ensure!(
            !path.symlink_metadata()?.file_type().is_symlink(),
            "artifact inspection refuses symlink paths"
        );
    }
    let inventory: Value =
        serde_json::from_slice(&fs::read(root.join(poolster_core::files::OWNERSHIP_PATH))?)?;
    anyhow::ensure!(inventory["version"] == 1, "unsupported ownership inventory");
    let files = inventory["files"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("missing ownership files"))?;
    let artifacts: Vec<_> = files.iter().map(|(path, value)| json!({"path":path,"owner":value.get("owner"),"create_once":value.get("create_once"),"content_hash":value.get("sha256")})).collect();
    Ok(
        json!({"schema_version":1,"kind":"emitted_artifact_inspection","artifacts":artifacts,
        "limits":"Describes existing emitted ownership only; does not execute plugins or infer their dependency graph."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doctor_projects_metadata_without_executing_commands_and_inspection_lists_owners() {
        let root = tempfile::tempdir().unwrap();
        let mut metadata = poolster_core::release::PackageMetadata::new("probe");
        metadata.language = "typescript".into();
        metadata.version = "0.1.0".into();
        metadata.build.push(poolster_core::release::PackageCommand {
            program: "MUST_NOT_EXECUTE".into(),
            args: vec!["SECRET_ARG".into()],
        });
        let mut tree = poolster_core::GeneratedTree::default();
        tree.insert(
            poolster_core::GeneratedFile::new(
                "sdk/.poolster/package.json",
                metadata.to_json().unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        tree.write_to(root.path()).unwrap();
        let report = doctor(root.path(), None).unwrap();
        assert_eq!(report["packages"][0]["build_configured"], true);
        assert!(!report.to_string().contains("SECRET_ARG"));
        assert!(!report.to_string().contains("MUST_NOT_EXECUTE"));
        let report = inspect(root.path()).unwrap();
        assert_eq!(report["kind"], "emitted_artifact_inspection");
        assert!(report["artifacts"][0]["content_hash"].is_string());
    }
    #[test]
    fn absent_and_malicious_inventory_have_actionable_redacted_results() {
        let root = tempfile::tempdir().unwrap();
        let report = doctor(root.path(), None).unwrap();
        assert_eq!(report["status"], "needs_action");
        fs::create_dir_all(root.path().join(".poolster")).unwrap();
        fs::write(
            root.path().join(poolster_core::files::OWNERSHIP_PATH),
            br#"{"version":1,"files":{"../secret/.poolster/package.json":{}}}"#,
        )
        .unwrap();
        let report = doctor(root.path(), None).unwrap();
        assert!(report["packages"].as_array().unwrap().is_empty());
        assert!(!report.to_string().contains("../secret"));
        assert!(
            report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|check| check["name"] == "package_metadata")
        );
    }
}
