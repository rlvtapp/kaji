//! Workflow descriptions are validated and inspected, never executed on load.
use anyhow::{Context, Result, bail};
use poolster_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};
use roas_arazzo::validation::Validate;
use serde_json::Value;
pub mod blocks;
pub mod contracts;
mod workflow_sources;
mod workflow_values;
mod workflows;

#[derive(Debug)]
pub enum ArazzoModel {
    V1_0(roas_arazzo::v1_0::Description),
    V1_1(roas_arazzo::v1_1::Description),
}

#[derive(Debug)]
pub struct ArazzoDocument {
    pub model: ArazzoModel,
    pub source: Value,
    /// Source descriptions are retained but not fetched. Their operations and
    /// schemas cannot be verified by this document-only validation pass.
    pub unresolved_sources: Vec<String>,
}

pub fn parse(source: &str) -> Result<ArazzoDocument> {
    let source: Value = serde_yaml_ng::from_str(source).context("invalid Arazzo JSON/YAML")?;
    let version = source["arazzo"]
        .as_str()
        .context("Arazzo requires a string arazzo version")?;
    let model = if matches!(version, "1.0.0" | "1.0.1") {
        let doc: roas_arazzo::v1_0::Description =
            serde_json::from_value(source.clone()).context("invalid Arazzo 1.0 document")?;
        doc.validate(enumset::EnumSet::empty())
            .context("Arazzo validation failed")?;
        ArazzoModel::V1_0(doc)
    } else if version == "1.1.0" {
        let doc: roas_arazzo::v1_1::Description =
            serde_json::from_value(source.clone()).context("invalid Arazzo 1.1 document")?;
        doc.validate(enumset::EnumSet::empty())
            .context("Arazzo validation failed")?;
        ArazzoModel::V1_1(doc)
    } else {
        bail!("unsupported Arazzo version {version}; supported: 1.0.0, 1.0.1, 1.1.0");
    };
    // roas validates dependency syntax but does not verify workflow targets.
    let workflows = source["workflows"]
        .as_array()
        .context("Arazzo workflows must be an array")?;
    let ids: std::collections::HashSet<&str> = workflows
        .iter()
        .filter_map(|workflow| workflow["workflowId"].as_str())
        .collect();
    fn validate_reusables(root: &Value, object: &Value, lists: &[(&str, &str)]) -> Result<()> {
        for (field, category) in lists {
            for entry in object[*field].as_array().into_iter().flatten() {
                if let Some(reference) = entry.get("reference").and_then(Value::as_str) {
                    let prefix = format!("$components.{category}.");
                    let name = reference.strip_prefix(&prefix).with_context(|| format!("unsupported Arazzo reusable reference {reference:?}; expected {prefix}<name>"))?;
                    if root["components"][*category].get(name).is_none() {
                        bail!("unresolved Arazzo reusable reference {reference:?}");
                    }
                }
            }
        }
        Ok(())
    }
    for workflow in workflows {
        validate_reusables(
            &source,
            workflow,
            &[
                ("parameters", "parameters"),
                ("successActions", "successActions"),
                ("failureActions", "failureActions"),
            ],
        )?;
        for step in workflow["steps"].as_array().into_iter().flatten() {
            validate_reusables(
                &source,
                step,
                &[
                    ("parameters", "parameters"),
                    ("onSuccess", "successActions"),
                    ("onFailure", "failureActions"),
                ],
            )?;
        }
        for dependency in workflow["dependsOn"].as_array().into_iter().flatten() {
            let dependency = dependency
                .as_str()
                .context("Arazzo dependsOn entries must be strings")?;
            if dependency.starts_with('$') {
                bail!(
                    "external workflow dependency {dependency:?} requires source resolution, which is not yet supported"
                );
            }
            if !ids.contains(dependency) {
                bail!(
                    "workflow {:?} depends on unknown workflow {dependency:?}",
                    workflow["workflowId"]
                );
            }
            if workflow["workflowId"].as_str() == Some(dependency) {
                bail!("workflow {dependency:?} depends on itself");
            }
        }
    }
    fn visit<'a>(
        id: &'a str,
        workflows: &'a [Value],
        active: &mut std::collections::HashSet<&'a str>,
        complete: &mut std::collections::HashSet<&'a str>,
    ) -> Result<()> {
        if complete.contains(id) {
            return Ok(());
        }
        if !active.insert(id) {
            bail!("cyclic Arazzo workflow dependency at {id:?}");
        }
        let workflow = workflows
            .iter()
            .find(|workflow| workflow["workflowId"].as_str() == Some(id))
            .context("unknown workflow dependency")?;
        for dependency in workflow["dependsOn"].as_array().into_iter().flatten() {
            visit(
                dependency.as_str().context("invalid workflow dependency")?,
                workflows,
                active,
                complete,
            )?;
        }
        active.remove(id);
        complete.insert(id);
        Ok(())
    }
    let mut active = std::collections::HashSet::new();
    let mut complete = std::collections::HashSet::new();
    for id in ids {
        visit(id, workflows, &mut active, &mut complete)?;
    }
    let unresolved_sources = source["sourceDescriptions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["url"].as_str().map(str::to_owned))
        .collect();
    Ok(ArazzoDocument {
        model,
        source,
        unresolved_sources,
    })
}

impl ArazzoDocument {
    pub fn summary(&self) -> ContractSummary {
        let operations = self.source["workflows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|workflow| OperationSummary {
                name: workflow["workflowId"].as_str().unwrap_or_default().into(),
                kind: "workflow".into(),
            })
            .collect();
        let mut types: Vec<String> = self.source["components"]["inputs"]
            .as_object()
            .map(|v| v.keys().cloned().collect())
            .unwrap_or_default();
        types.sort();
        ContractSummary {
            format: "arazzo".into(),
            title: self.source["info"]["title"]
                .as_str()
                .unwrap_or_default()
                .into(),
            version: self.source["info"]["version"].as_str().map(str::to_owned),
            types,
            operations,
        }
    }
}

impl poolster_core::engine::Contract for ArazzoDocument {
    const NAME: &'static str = "poolster.arazzo";
}

/// Native arazzo input provider.
pub struct ArazzoInput;
impl poolster_core::input::InputPlugin for ArazzoInput {
    fn id(&self) -> &str {
        "arazzo.roas"
    }
    fn format(&self) -> &str {
        "arazzo"
    }
    fn load_with_options(
        &self,
        path: &std::path::Path,
        options: &poolster_core::input::InputOptions,
    ) -> Result<poolster_core::input::InputContract> {
        if options == &poolster_core::input::InputOptions::default() {
            return self.load(path);
        }
        let document = parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("read Arazzo {}", path.display()))?,
        )?;
        let contract = workflows::resolve(
            &document,
            options,
            path.parent().unwrap_or(std::path::Path::new(".")),
        )?;
        let mut input = poolster_core::input::InputContract::new(document.summary());
        let source = std::fs::canonicalize(path)
            .with_context(|| format!("resolve Arazzo document identity {}", path.display()))?;
        input.publish(blocks::step_blocks(
            &contract,
            source.to_string_lossy().into_owned(),
        ))?;
        input.publish_with_reference(
            contract.clone(),
            poolster_core::blocks::ContractReference::from_bytes(
                <crate::contracts::WorkflowOperations as poolster_core::engine::Contract>::NAME,
                source.to_string_lossy().into_owned(),
                &serde_json::to_vec(&contract)?,
            ),
        )?;
        input.publish(document)?;
        Ok(input)
    }
    fn load(&self, path: &std::path::Path) -> anyhow::Result<poolster_core::input::InputContract> {
        let document = parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("cannot read contract {}", path.display()))?,
        )?;
        let mut input = poolster_core::input::InputContract::new(document.summary());
        for source in &document.unresolved_sources {
            input.diagnostics.push(poolster_core::input::InputDiagnostic { code: "unresolved-source".into(), message: format!("Source contract {source} was retained but not loaded; its operation references remain unverified") });
        }
        input.publish(blocks::WorkflowStepBlocks { parent: Some(poolster_core::blocks::ContractReference::from_bytes(<ArazzoDocument as poolster_core::engine::Contract>::NAME, path.canonicalize()?.to_string_lossy(), &serde_json::to_vec(&document.source)?)), state: poolster_core::blocks::CollectionState::Unavailable { diagnostics: vec!["Workflow operations require explicit local source mappings and supported resolution".into()] }, items: vec![] })?;
        let reference = poolster_core::blocks::ContractReference::from_bytes(
            <ArazzoDocument as poolster_core::engine::Contract>::NAME,
            path.canonicalize()?.to_string_lossy(),
            &serde_json::to_vec(&document.source)?,
        );
        input.publish_with_reference(document, reference)?;
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &str = include_str!("../tests/fixtures/workflows.yaml");
    #[test]
    fn preserves_dependencies_steps_and_reports_unloaded_sources() {
        let document = parse(EXAMPLE).unwrap();
        assert_eq!(document.source["workflows"][1]["dependsOn"][0], "login");
        assert_eq!(
            document.source["workflows"][1]["steps"][0]["operationId"],
            "createOrder"
        );
        assert_eq!(
            document.unresolved_sources,
            ["https://example.test/openapi.yaml"]
        );
        assert_eq!(document.summary().operations.len(), 2);
        assert!(matches!(document.model, ArazzoModel::V1_0(_)));
        parse(&EXAMPLE.replace("1.0.1", "1.1.0")).unwrap();
    }
    #[test]
    fn rejects_missing_or_wrong_kind_reusable_components() {
        let mut value: Value = serde_yaml_ng::from_str(EXAMPLE).unwrap();
        value["workflows"][0]["steps"][0]["parameters"] =
            serde_json::json!([{"reference":"$components.parameters.token"}]);
        assert!(
            format!("{:#}", parse(&value.to_string()).unwrap_err())
                .contains("unresolved Arazzo reusable")
        );
        value["components"] = serde_json::json!({"parameters":{"token":{"name":"token","in":"header","value":"secret"}}});
        parse(&value.to_string()).unwrap();
        value["workflows"][0]["steps"][0]["parameters"][0]["reference"] =
            serde_json::json!("$components.successActions.token");
        assert!(parse(&value.to_string()).is_err());
    }
    #[test]
    fn rejects_malformed_and_future_versions() {
        for version in ["1.0.bad", "1.0.2", "1.1.1", "1.1.0-beta", "1.0.01", "2.0.0"] {
            let input = EXAMPLE.replace("arazzo: 1.0.1", &format!("arazzo: {version}"));
            assert!(
                format!("{:#}", parse(&input).unwrap_err()).contains("unsupported Arazzo version"),
                "{version}"
            );
        }
        parse(&EXAMPLE.replace("arazzo: 1.0.1", "arazzo: 1.0.0")).unwrap();
    }
    #[test]
    fn rejects_duplicate_steps_and_missing_dependency() {
        assert!(parse(&EXAMPLE.replace("dependsOn: [login]", "dependsOn: [missing]")).is_err());
        let mut cycle: Value = serde_yaml_ng::from_str(EXAMPLE).unwrap();
        cycle["workflows"][0]["dependsOn"] = serde_json::json!(["checkout"]);
        assert!(format!("{:#}", parse(&cycle.to_string()).unwrap_err()).contains("cyclic"));
        let duplicate = EXAMPLE.replace(
            "operationId: createOrder",
            "operationId: createOrder\n      - stepId: create\n        operationId: createOrder",
        );
        assert!(parse(&duplicate).is_err());
        assert!(parse(&EXAMPLE.replace("arazzo: 1.0.1", "arazzo: 2.0.0")).is_err());
    }
}
