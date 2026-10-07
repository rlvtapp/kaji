//! Read-only GitHub delivery observations. Responses are deliberately projected:
//! secret/variable values, arbitrary API fields and subprocess errors never leave here.
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, process::Command};

const WORKFLOWS: [(&str, bool); 4] = [
    ("kaji-sdks.yml", false),
    ("kaji-sdk-ci.yml", false),
    ("kaji-sdk-release.yml", false),
    ("kaji-spec-sync.yml", true),
];

pub fn remote_status(repository: &str) -> Result<Value> {
    validate_repository(repository)?;
    let directory = api(
        &format!("repos/{repository}/contents/.github/workflows"),
        false,
    )
    .and_then(|value| workflow_names(&value));
    let workflows: Vec<Value> = WORKFLOWS
        .into_iter()
        .map(|(name, optional)| {
            workflow_observation(name, optional, &directory, || {
                api(
                    &format!("repos/{repository}/actions/workflows/{name}/runs?per_page=1"),
                    false,
                )
                .and_then(|value| latest_run(&value))
            })
        })
        .collect();
    let prs = api(
        &format!("repos/{repository}/pulls?state=open&per_page=100"),
        true,
    )
    .and_then(|value| automation_prs(&value));
    let secrets = api(
        &format!("repos/{repository}/actions/secrets?per_page=100"),
        true,
    )
    .and_then(|value| configured_names(&value, "secrets"));
    let variables = api(
        &format!("repos/{repository}/actions/variables?per_page=100"),
        true,
    )
    .and_then(|value| configured_names(&value, "variables"));
    let available = directory.is_ok()
        && prs.is_ok()
        && secrets.is_ok()
        && variables.is_ok()
        && workflows.iter().all(workflow_observed);
    Ok(json!({
        "repository": repository,
        "status": if available {"observed"} else {"partially_unavailable"},
        "workflows": workflows,
        "automation_prs": report(prs),
        "repository_secret_names": report(secrets),
        "repository_variable_names": report(variables),
        "scope": "repository; environment and organization settings are not inspected",
        "note": "Observations do not verify secret values, registry trust, branch protection, or successful publication."
    }))
}

fn workflow_observation(
    name: &str,
    optional: bool,
    directory: &std::result::Result<BTreeSet<String>, &'static str>,
    runs: impl FnOnce() -> Observation,
) -> Value {
    let existence = match directory {
        Ok(names) => json!({"status": "available", "exists": names.contains(name)}),
        Err(reason) => unavailable(reason),
    };
    let latest = if optional {
        match directory {
            Ok(names) if names.contains(name) => report(runs()),
            Ok(_) => json!({"status": "skipped", "reason": "optional_workflow_not_configured"}),
            Err(_) => unavailable("workflow_visibility_unavailable"),
        }
    } else {
        // Required workflow observations remain independent of contents visibility.
        report(runs())
    };
    json!({"file": name, "optional": optional, "existence": existence, "latest_run": latest})
}
fn workflow_observed(workflow: &Value) -> bool {
    workflow["latest_run"]["status"] == "available"
        || (workflow["optional"] == true
            && workflow["existence"]["exists"] == false
            && workflow["latest_run"]["status"] == "skipped")
}

fn validate_repository(repository: &str) -> Result<()> {
    let parts: Vec<_> = repository.split('/').collect();
    ensure!(
        parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty(),
        "repository must be OWNER/REPO"
    );
    ensure!(
        parts[0]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && parts[1]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && parts[1] != "."
            && parts[1] != "..",
        "repository must be OWNER/REPO"
    );
    Ok(())
}

type Observation = std::result::Result<Value, &'static str>;
fn api(endpoint: &str, paginate: bool) -> Observation {
    let mut command = Command::new("gh");
    command.args(["api", "--method", "GET", endpoint]);
    if paginate {
        command.args(["--paginate", "--slurp"]);
    }
    let output = command.output().map_err(|_| "gh_unavailable")?;
    if !output.status.success() {
        return Err("request_unavailable");
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "invalid_response")
}
fn unavailable(reason: &str) -> Value {
    json!({"status": "unavailable", "reason": reason})
}
fn report(result: Observation) -> Value {
    match result {
        Ok(data) => json!({"status": "available", "data": data}),
        Err(reason) => unavailable(reason),
    }
}
fn workflow_names(value: &Value) -> std::result::Result<BTreeSet<String>, &'static str> {
    let files = value.as_array().ok_or("unexpected_response")?;
    let mut names = BTreeSet::new();
    for file in files {
        let name = file
            .get("name")
            .and_then(Value::as_str)
            .ok_or("unexpected_response")?;
        if file.get("type").and_then(Value::as_str) == Some("file") {
            names.insert(name.into());
        }
    }
    Ok(names)
}
fn configured_names(value: &Value, key: &str) -> Observation {
    // gh --paginate --slurp wraps each response object in an outer array.
    let pages = value.as_array().ok_or("unexpected_response")?;
    if pages.is_empty() {
        return Err("unexpected_response");
    }
    let mut names = BTreeSet::new();
    for page in pages {
        for item in page
            .get(key)
            .and_then(Value::as_array)
            .ok_or("unexpected_response")?
        {
            names.insert(
                item.get("name")
                    .and_then(Value::as_str)
                    .ok_or("unexpected_response")?
                    .to_owned(),
            );
        }
    }
    Ok(json!(names))
}
fn latest_run(value: &Value) -> Observation {
    let runs = value
        .get("workflow_runs")
        .and_then(Value::as_array)
        .ok_or("unexpected_response")?;
    let Some(run) = runs.first() else {
        return Ok(Value::Null);
    };
    let id = run
        .get("id")
        .and_then(Value::as_u64)
        .ok_or("unexpected_response")?;
    let status = run
        .get("status")
        .and_then(Value::as_str)
        .ok_or("unexpected_response")?;
    let conclusion = match run.get("conclusion") {
        Some(Value::Null) | None => Value::Null,
        Some(Value::String(value)) => json!(value),
        _ => return Err("unexpected_response"),
    };
    Ok(json!({"id": id, "status": status, "conclusion": conclusion,
        "url": run.get("html_url").and_then(Value::as_str),
        "event": run.get("event").and_then(Value::as_str),
        "head_sha": run.get("head_sha").and_then(Value::as_str),
        "created_at": run.get("created_at").and_then(Value::as_str),
        "updated_at": run.get("updated_at").and_then(Value::as_str)}))
}
fn automation_prs(value: &Value) -> Observation {
    let pages = value.as_array().ok_or("unexpected_response")?;
    if pages.is_empty() {
        return Err("unexpected_response");
    }
    let mut prs = Vec::new();
    for page in pages {
        for pr in page.as_array().ok_or("unexpected_response")? {
            let branch = pr
                .pointer("/head/ref")
                .and_then(Value::as_str)
                .ok_or("unexpected_response")?;
            let release = branch.starts_with("release-please--")
                || pr
                    .get("labels")
                    .and_then(Value::as_array)
                    .is_some_and(|labels| {
                        labels.iter().any(|label| {
                            label.get("name").and_then(Value::as_str)
                                == Some("autorelease: pending")
                        })
                    });
            let sdk = branch == "codex/kaji-sdks" || branch.starts_with("codex/kaji-sdks/");
            let spec =
                branch == "codex/kaji-spec-sync" || branch.starts_with("codex/kaji-spec-sync/");
            if !release && !sdk && !spec {
                continue;
            }
            let number = pr
                .get("number")
                .and_then(Value::as_u64)
                .ok_or("unexpected_response")?;
            prs.push(
                json!({"number": number, "url": pr.get("html_url").and_then(Value::as_str),
                "head_branch": branch, "draft": pr.get("draft").and_then(Value::as_bool),
                "kind": if release {"release"} else if spec {"spec_sync"} else {"generated_sdk"}}),
            );
        }
    }
    prs.sort_by_key(|pr| pr["number"].as_u64());
    Ok(json!({"pull_requests": prs,
        "recognition": "default codex/kaji-sdks or codex/kaji-spec-sync branches, release-please branches, or autorelease: pending label; custom SDK branch names may be omitted"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_sorted_paginated_and_values_never_reported() {
        let value = json!([{"variables":[{"name":"Z","value":"private"},{"name":"A","value":"credential"}]}, {"variables":[{"name":"A","value":"different"}]}]);
        let output = configured_names(&value, "variables").unwrap();
        assert_eq!(output, json!(["A", "Z"]));
        assert!(!output.to_string().contains("private"));
        assert_eq!(
            configured_names(
                &json!([{"secrets":[{"name":"TOKEN","encrypted_value":"hidden"}]}]),
                "secrets"
            )
            .unwrap(),
            json!(["TOKEN"])
        );
    }
    #[test]
    fn unavailable_is_not_an_empty_success_or_a_healthy_run() {
        assert_eq!(
            report(Err("request_unavailable")),
            json!({"status":"unavailable","reason":"request_unavailable"})
        );
        assert!(configured_names(&json!({"variables":[]}), "variables").is_err());
        assert!(latest_run(&json!({"message":"auth secret"})).is_err());
        assert_eq!(
            latest_run(&json!({"workflow_runs":[]})).unwrap(),
            Value::Null
        );
        let run = latest_run(&json!({"workflow_runs":[{"id":7,"status":"completed","conclusion":"failure","token":"hidden","jobs":["private"],"head_sha":"abc"}]})).unwrap();
        assert_eq!(run["conclusion"], "failure");
        assert!(run.get("token").is_none() && run.get("jobs").is_none());
    }
    #[test]
    fn recognizes_only_automation_and_projects_safe_pr_fields() {
        let prs = json!([[{"number":3,"head":{"ref":"custom-feature"}}, {"number":2,"head":{"ref":"release-please--branches--main"},"body":"secret","html_url":"https://github.com/owner/repo/pull/2"}], [{"number":1,"head":{"ref":"codex/kaji-sdks"},"title":"private","draft":false}]]);
        let output = automation_prs(&prs).unwrap();
        assert_eq!(output["pull_requests"].as_array().unwrap().len(), 2);
        assert_eq!(output["pull_requests"][0]["kind"], "generated_sdk");
        assert!(!output.to_string().contains("private") && !output.to_string().contains("secret"));
        assert!(automation_prs(&json!([{"unexpected":"shape"}])).is_err());
    }
    #[test]
    fn repository_and_workflow_validation_are_read_only_and_conservative() {
        for repository in ["owner/repo", "owner-name/repo.name_1"] {
            assert!(validate_repository(repository).is_ok());
        }
        for repository in [
            "../repo",
            "owner/..",
            "https://github.com/o/r",
            "o/r/extra",
            "o/r?token=secret",
            "--help",
        ] {
            assert!(validate_repository(repository).is_err());
        }
        let names = workflow_names(&json!([{"name":"kaji-sdks.yml","type":"file","content":"ignored"},{"name":"directory","type":"dir"}])).unwrap();
        assert!(names.contains("kaji-sdks.yml") && !names.contains("directory"));
        assert!(workflow_names(&json!({"message":"not found"})).is_err());
    }
    #[test]
    fn optional_missing_workflow_is_skipped_without_querying_runs() {
        let directory = Ok(BTreeSet::new());
        let optional = workflow_observation("kaji-spec-sync.yml", true, &directory, || {
            panic!("must not query missing optional workflow")
        });
        assert_eq!(optional["optional"], true);
        assert_eq!(optional["latest_run"]["status"], "skipped");
        assert!(workflow_observed(&optional));
        let hidden = workflow_observation(
            "kaji-spec-sync.yml",
            true,
            &Err("request_unavailable"),
            || panic!("must not query unknown optional workflow"),
        );
        assert!(!workflow_observed(&hidden));
        let present = Ok(BTreeSet::from(["kaji-spec-sync.yml".into()]));
        let failed = workflow_observation("kaji-spec-sync.yml", true, &present, || {
            Err("request_unavailable")
        });
        assert!(!workflow_observed(&failed));
        let available =
            workflow_observation("kaji-spec-sync.yml", true, &present, || Ok(Value::Null));
        assert!(workflow_observed(&available));
        let required = workflow_observation("kaji-sdk-ci.yml", false, &directory, || {
            Err("request_unavailable")
        });
        assert!(!workflow_observed(&required));
    }
    #[test]
    fn recognizes_spec_sync_review_branch() {
        let output =
            automation_prs(&json!([[{"number":5,"head":{"ref":"codex/kaji-spec-sync"}}]])).unwrap();
        assert_eq!(output["pull_requests"][0]["kind"], "spec_sync");
    }
}
