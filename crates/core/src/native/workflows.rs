//! Source-resolved workflow contracts; parser ASTs are intentionally absent.
use super::ModelField;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum WorkflowValue {
    Literal(serde_json::Value),
    Input(String),
    StepOutput { step: String, name: String },
    WorkflowOutput { workflow: String, name: String },
    ResponseBody(String),
    StatusCode,
    Object(BTreeMap<String, WorkflowValue>),
    Array(Vec<WorkflowValue>),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub location: String,
    pub value: WorkflowValue,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HttpWorkflowOperation {
    pub source: String,
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub base_url: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub operation: HttpWorkflowOperation,
    pub parameters: Vec<WorkflowParameter>,
    pub request_body: Option<WorkflowValue>,
    pub expected_statuses: Vec<u16>,
    pub outputs: BTreeMap<String, WorkflowValue>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub inputs: Vec<ModelField>,
    pub dependencies: Vec<String>,
    pub steps: Vec<WorkflowStep>,
    pub outputs: BTreeMap<String, WorkflowValue>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowOperations {
    pub title: String,
    pub workflows: Vec<Workflow>,
    /// Resolved local OpenAPI documents keyed by declared source name.
    pub source_documents: BTreeMap<String, serde_json::Value>,
    pub native_document: serde_json::Value,
}
impl crate::engine::Contract for WorkflowOperations {
    const NAME: &'static str = "poolster.workflow-operations.v1";
}

/// Optional step blocks retain their containing workflow context. Execute a whole
/// `WorkflowOperations` contract to preserve dependency and sequencing semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStepBlock {
    pub workflow_id: String,
    pub step_index: usize,
    pub workflow_inputs: Vec<ModelField>,
    pub workflow_dependencies: Vec<String>,
    pub step: WorkflowStep,
}
impl crate::blocks::Block for WorkflowStepBlock {
    const CONTRACT_NAME: &'static str = "poolster.workflow-step-blocks.v1";
}
impl WorkflowOperations {
    /// Caller supplies a stable document identity; IDs use workflow/step names,
    /// not array positions or generated symbols. Resolved Arazzo providers publish
    /// this view alongside the whole contract; other providers may opt in.
    pub fn step_blocks(
        &self,
        source: impl Into<String>,
    ) -> crate::blocks::Blocks<WorkflowStepBlock> {
        use crate::{
            blocks::{Block, BlockId, BlockMetadata, BlockReference, Blocks, BuildingBlock},
            engine::Contract,
        };
        let source = source.into();
        fn escape(value: &str) -> String {
            value.replace('~', "~0").replace('/', "~1")
        }
        fn references(
            value: &WorkflowValue,
            workflow: &str,
            found: &mut std::collections::BTreeSet<(String, String)>,
        ) {
            match value {
                WorkflowValue::StepOutput { step, .. } => {
                    found.insert((
                        WorkflowStepBlock::CONTRACT_NAME.into(),
                        format!("/workflows/{}/steps/{}", escape(workflow), escape(step)),
                    ));
                }
                WorkflowValue::WorkflowOutput { workflow, .. } => {
                    found.insert((
                        WorkflowOperations::NAME.into(),
                        format!("/workflows/{}", escape(workflow)),
                    ));
                }
                WorkflowValue::Object(object) => {
                    for value in object.values() {
                        references(value, workflow, found);
                    }
                }
                WorkflowValue::Array(array) => {
                    for value in array {
                        references(value, workflow, found);
                    }
                }
                _ => (),
            }
        }
        let mut items = Vec::new();
        for (workflow_index, workflow) in self.workflows.iter().enumerate() {
            for (step_index, step) in workflow.steps.iter().enumerate() {
                let mut found = std::collections::BTreeSet::new();
                found.insert((
                    Self::NAME.into(),
                    format!("/workflows/{}", escape(&workflow.id)),
                ));
                for dependency in &workflow.dependencies {
                    found.insert((
                        Self::NAME.into(),
                        format!("/workflows/{}", escape(dependency)),
                    ));
                }
                for parameter in &step.parameters {
                    references(&parameter.value, &workflow.id, &mut found);
                }
                if let Some(value) = &step.request_body {
                    references(value, &workflow.id, &mut found);
                }
                for value in step.outputs.values() {
                    references(value, &workflow.id, &mut found);
                }
                items.push(BuildingBlock {
                    metadata: BlockMetadata {
                        parent: Some(crate::blocks::ContractReference::from_bytes(
                            <Self as crate::engine::Contract>::NAME,
                            source.clone(),
                            &serde_json::to_vec(self).expect("owned contract is serializable"),
                        )),
                        id: BlockId {
                            source: source.clone(),
                            local: format!(
                                "/workflows/{}/steps/{}",
                                escape(&workflow.id),
                                escape(&step.id)
                            ),
                        },
                        capabilities: [
                            "poolster.workflow-step".into(),
                            "poolster.http-operation".into(),
                        ]
                        .into(),
                        references: found
                            .into_iter()
                            .map(|(contract, local)| BlockReference {
                                contract,
                                id: BlockId {
                                    source: source.clone(),
                                    local,
                                },
                            })
                            .collect(),
                        location: Some(format!("/workflows/{workflow_index}/steps/{step_index}")),
                    },
                    value: WorkflowStepBlock {
                        workflow_id: workflow.id.clone(),
                        step_index,
                        workflow_inputs: workflow.inputs.clone(),
                        workflow_dependencies: workflow.dependencies.clone(),
                        step: step.clone(),
                    },
                });
            }
        }
        Blocks {
            parent: Some(crate::blocks::ContractReference::from_bytes(
                <Self as crate::engine::Contract>::NAME,
                source.clone(),
                &serde_json::to_vec(self).expect("owned contract is serializable"),
            )),
            state: crate::blocks::CollectionState::Complete,
            items,
        }
    }
}
