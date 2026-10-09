//! Lower validated Arazzo workflows into a conservative executable contract.
use super::{workflow_sources::operations, workflow_values::*};
use anyhow::{Context, Result, bail, ensure};
use poolster_core::{
    input::InputOptions,
    native::{ModelKind, workflows::*},
};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};
pub(super) fn resolve(
    document: &super::ArazzoDocument,
    options: &InputOptions,
    base: &Path,
) -> Result<WorkflowOperations> {
    ensure!(
        options.operation_files.is_empty()
            && options.import_roots.is_empty()
            && options.broker.is_none(),
        "Arazzo runner accepts only workflow_sources options"
    );
    let native = &document.source;
    let mut sources = BTreeMap::new();
    let mut resolved = Vec::new();
    for source in native["sourceDescriptions"]
        .as_array()
        .context("sourceDescriptions required")?
    {
        let name = nonempty(source, "name")?;
        ensure!(
            source["type"] == "openapi",
            "only OpenAPI workflow source descriptions are executable"
        );
        let path = options
            .workflow_sources
            .get(name)
            .with_context(|| format!("explicit workflow_sources mapping missing for {name:?}"))?;
        let path = if path.is_absolute() {
            path.clone()
        } else {
            base.join(path)
        };
        let parsed: Value = serde_yaml_ng::from_str(
            &std::fs::read_to_string(&path)
                .with_context(|| format!("read workflow source {}", path.display()))?,
        )
        .context("invalid workflow OpenAPI source")?;
        resolved.extend(operations(name, &parsed)?);
        ensure!(
            sources.insert(name.to_owned(), parsed).is_none(),
            "duplicate workflow source name {name}"
        );
    }
    ensure!(
        options
            .workflow_sources
            .keys()
            .all(|name| sources.contains_key(name)),
        "workflow_sources contains undeclared source mapping"
    );
    let all_workflows = native["workflows"]
        .as_array()
        .context("workflows required")?;
    let workflow_outputs: BTreeMap<String, Vec<String>> = all_workflows
        .iter()
        .map(|workflow| {
            Ok((
                nonempty(workflow, "workflowId")?.to_owned(),
                workflow["outputs"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(name, _)| name.clone())
                    .collect(),
            ))
        })
        .collect::<Result<_>>()?;
    let mut workflows = Vec::new();
    for workflow in all_workflows {
        reject(
            workflow,
            &["parameters", "successActions", "failureActions"],
        )?;
        let id = nonempty(workflow, "workflowId")?.to_owned();
        let dependencies = string_list(&workflow["dependsOn"])?;
        let input_schema = if let Some(reference) = workflow["inputs"]["$ref"].as_str() {
            ensure!(
                workflow["inputs"]
                    .as_object()
                    .is_some_and(|object| object.len() == 1),
                "workflow input $ref siblings are unsupported"
            );
            let name = reference
                .strip_prefix("#/components/inputs/")
                .context("only local input component references are supported")?;
            &native["components"]["inputs"][name]
        } else {
            &workflow["inputs"]
        };
        let inputs = if input_schema.is_null() {
            Vec::new()
        } else {
            match schema_type(input_schema)?.kind {
                ModelKind::Object(fields) => fields,
                _ => bail!("workflow inputs must have object type"),
            }
        };
        let mut steps = Vec::new();
        let mut prior_steps = BTreeMap::new();
        for step in workflow["steps"].as_array().context("steps required")? {
            reject(step, &["workflowId", "onSuccess", "onFailure"])?;
            let step_id = nonempty(step, "stepId")?.to_owned();
            let candidates: Vec<_> = if let Some(operation_id) = step["operationId"].as_str() {
                if let Some(reference) = operation_id.strip_prefix("$sourceDescriptions.") {
                    let (source, operation) = reference
                        .split_once('.')
                        .context("qualified operationId requires source and operation")?;
                    resolved
                        .iter()
                        .filter(|(_, op, _)| op.source == source && op.operation_id == operation)
                        .collect()
                } else {
                    resolved
                        .iter()
                        .filter(|(_, op, _)| op.operation_id == operation_id)
                        .collect()
                }
            } else if let Some(operation_path) = step["operationPath"].as_str() {
                let reference = operation_path
                    .strip_prefix("{$sourceDescriptions.")
                    .context("operationPath requires explicit source expression")?;
                let (source, pointer) = reference
                    .split_once(".url}#")
                    .context("operationPath requires source URL plus JSON pointer")?;
                resolved
                    .iter()
                    .filter(|(path, op, _)| op.source == source && path == pointer)
                    .collect()
            } else {
                bail!("step {step_id:?} requires operationId or operationPath");
            };
            ensure!(
                candidates.len() == 1,
                "step {step_id:?} operation reference is unresolved or ambiguous"
            );
            let (_, operation, operation_document) = candidates[0];
            ensure!(
                operation_document["security"]
                    .as_array()
                    .is_none_or(|security| security.is_empty()),
                "authenticated OpenAPI operations are unsupported by this workflow runner slice"
            );
            let declared = operation_document["parameters"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut parameters = Vec::new();
            for parameter in step["parameters"].as_array().into_iter().flatten() {
                reject(parameter, &["reference"])?;
                let name = nonempty(parameter, "name")?;
                let location = nonempty(parameter, "in")?;
                ensure!(
                    ["path", "query", "header"].contains(&location),
                    "unsupported workflow parameter location {location}"
                );
                let declared_param = declared
                    .iter()
                    .find(|v| v["name"] == name && v["in"] == location)
                    .with_context(|| {
                        format!(
                            "parameter {location}:{name} absent from resolved OpenAPI operation"
                        )
                    })?;
                reject(
                    declared_param,
                    &["$ref", "content", "style", "explode", "allowReserved"],
                )?;
                ensure!(
                    ["string", "number", "integer", "boolean"].contains(
                        &declared_param["schema"]["type"]
                            .as_str()
                            .unwrap_or_default()
                    ),
                    "workflow parameters must have primitive schemas"
                );
                ensure!(
                    !parameters
                        .iter()
                        .any(|param: &WorkflowParameter| param.name == name
                            && param.location == location),
                    "duplicate workflow parameter"
                );
                let value = expression(
                    parameter
                        .get("value")
                        .context("workflow parameter requires value")?,
                )?;
                validate_value(
                    &value,
                    &inputs,
                    &prior_steps,
                    &dependencies,
                    &workflow_outputs,
                    false,
                )?;
                parameters.push(WorkflowParameter {
                    name: name.to_owned(),
                    location: location.to_owned(),
                    value,
                });
            }
            for parameter in &declared {
                ensure!(
                    parameter.get("$ref").is_none(),
                    "referenced OpenAPI parameters are unsupported"
                );
                if parameter["required"] == true {
                    ensure!(
                        parameters
                            .iter()
                            .any(|v| parameter["name"] == v.name && parameter["in"] == v.location),
                        "required OpenAPI parameter missing from workflow step"
                    );
                }
            }
            let request_body = if let Some(body) = step.get("requestBody") {
                reject(body, &["replacements"])?;
                ensure!(
                    body["contentType"].as_str().unwrap_or("application/json")
                        == "application/json",
                    "only JSON workflow request bodies are supported"
                );
                ensure!(
                    operation_document["requestBody"]["content"]
                        .get("application/json")
                        .is_some(),
                    "JSON request body absent from resolved OpenAPI operation"
                );
                let value = expression(
                    body.get("payload")
                        .context("requestBody requires payload")?,
                )?;
                validate_value(
                    &value,
                    &inputs,
                    &prior_steps,
                    &dependencies,
                    &workflow_outputs,
                    false,
                )?;
                Some(value)
            } else {
                ensure!(
                    operation_document["requestBody"]["required"] != true,
                    "required OpenAPI requestBody missing"
                );
                None
            };
            let mut expected_statuses = Vec::new();
            for criterion in step["successCriteria"].as_array().into_iter().flatten() {
                reject(criterion, &["context", "type"])?;
                let condition = nonempty(criterion, "condition")?;
                let status = condition
                    .strip_prefix("$statusCode == ")
                    .context("only $statusCode == <integer> success criteria are supported")?
                    .parse::<u16>()
                    .context("invalid success status")?;
                ensure!(
                    (100..=599).contains(&status),
                    "invalid HTTP status criterion"
                );
                expected_statuses.push(status);
            }
            ensure!(
                expected_statuses.len() <= 1,
                "multiple status criteria cannot simultaneously match"
            );
            let outputs = output_map(&step["outputs"])?;
            for value in outputs.values() {
                validate_value(
                    value,
                    &inputs,
                    &prior_steps,
                    &dependencies,
                    &workflow_outputs,
                    true,
                )?;
            }
            prior_steps.insert(step_id.clone(), outputs.keys().cloned().collect());
            steps.push(WorkflowStep {
                id: step_id,
                operation: operation.clone(),
                parameters,
                request_body,
                expected_statuses,
                outputs,
            });
        }
        let outputs = output_map(&workflow["outputs"])?;
        for value in outputs.values() {
            validate_value(
                value,
                &inputs,
                &prior_steps,
                &dependencies,
                &workflow_outputs,
                false,
            )?;
        }
        workflows.push(Workflow {
            id,
            inputs,
            dependencies,
            steps,
            outputs,
        });
    }
    Ok(WorkflowOperations {
        title: document.summary().title,
        workflows,
        source_documents: sources,
        native_document: native.clone(),
    })
}
