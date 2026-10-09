use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::workflows::WorkflowOperations,
};
use poolster_plugin_typescript as ts;
use std::{process::Command, sync::Arc};
const WORKFLOW: &str = include_str!("../../../inputs/arazzo/tests/fixtures/runner/workflows.yaml");
const OPENAPI: &str =
    include_str!("../../../inputs/arazzo/tests/fixtures/runner/shop.openapi.yaml");
fn generate(dir: &std::path::Path) -> anyhow::Result<poolster_core::GeneratedTree> {
    std::fs::write(dir.join("workflow.yaml"), WORKFLOW)?;
    std::fs::write(dir.join("shop.yaml"), OPENAPI)?;
    generate_loaded(dir, "workflow.yaml", "shop", "shop.yaml")
}
fn generate_loaded(
    dir: &std::path::Path,
    file: &str,
    source: &str,
    schema: &str,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_arazzo::ArazzoInput)?;
    let input =
        InputProvider::<WorkflowOperations>::new(Arc::new(registry), "arazzo", dir.join(file))
            .with_options(InputOptions {
                workflow_sources: [(source.into(), dir.join(schema))].into(),
                ..Default::default()
            });
    Packages::new()
        .package(
            ts::package("sdk")
                .with(ts::workflow(Some(input.handle())))
                .with(input),
        )
        .generate_native()
}

#[test]
fn emits_source_resolved_runner_and_regenerates() {
    let dir = tempfile::tempdir().unwrap();
    let tree = generate(dir.path()).unwrap();
    assert!(
        tree.get("sdk/workflows.ts")
            .unwrap()
            .contains("export function runWorkflow")
    );
    assert!(
        tree.get("sdk/workflows.ts")
            .unwrap()
            .contains("\"itemId\": string;")
    );
    assert!(
        tree.get("sdk/workflows.ts")
            .unwrap()
            .contains("\"quantity\"?: number;")
    );
    assert_eq!(
        tree.get("sdk/workflows.ts"),
        generate(dir.path()).unwrap().get("sdk/workflows.ts")
    );
    tree.write_to(dir.path()).unwrap();
    assert!(
        generate(dir.path())
            .unwrap()
            .check(dir.path())
            .unwrap()
            .is_empty()
    );
}
#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS (TypeScript5.9.3)"]
fn generated_runner_compiles_and_executes_source_resolved_http_workflow() {
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let dir = tempfile::tempdir().unwrap();
    generate(dir.path()).unwrap().write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/consumer.ts"), "import { runWorkflow_636865636b6f7574 } from './index.js';\nvoid runWorkflow_636865636b6f7574({itemId:'1'});\n// @ts-expect-error required itemId\nvoid runWorkflow_636865636b6f7574({});\n// @ts-expect-error integer quantity\nvoid runWorkflow_636865636b6f7574({itemId:'1',quantity:'many'});\n").unwrap();
    let compiled = Command::new("node")
        .arg(compiler)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let probe = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/workflow_probe.mjs"
        ))
        .arg(dir.path().join("sdk/dist/index.js"))
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{}{}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    );
}

#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS (TypeScript5.9.3)"]
fn pinned_upstream_openapi_workflow_compiles_and_runs_locally() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("authorize.yaml"),
        include_str!("../../../inputs/arazzo/tests/fixtures/runner/authorize.yaml"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("oauth.yaml"),
        include_str!("../../../inputs/arazzo/tests/corpus/oauth.openapi.yaml"),
    )
    .unwrap();
    generate_loaded(dir.path(), "authorize.yaml", "oauth", "oauth.yaml")
        .unwrap()
        .write_to(dir.path())
        .unwrap();
    let compiler = std::env::var("POOLSTER_TSC_JS").expect("POOLSTER_TSC_JS");
    let compiled = Command::new("node")
        .arg(compiler)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let probe = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/workflow_pinned_probe.mjs"
        ))
        .arg(dir.path().join("sdk/dist/index.js"))
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{}{}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    );
}

#[test]
fn substituted_provider_and_post_plugin_consume_owned_workflow_contracts() {
    use poolster_core::{
        GeneratedFile,
        engine::{Enforce, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
        native::workflows::{Workflow, WorkflowValue},
    };
    struct CustomInput {
        meta: Meta,
    }
    impl Plugin<ts::TypeScript> for CustomInput {
        fn kind(&self) -> &'static str {
            "custom-workflow-input"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<WorkflowOperations>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            cx.publish(WorkflowOperations {
                title: "Custom parser replacement".into(),
                source_documents: Default::default(),
                native_document: serde_json::json!({"custom":true}),
                workflows: vec![Workflow {
                    id: "custom".into(),
                    inputs: vec![],
                    dependencies: vec![],
                    steps: vec![],
                    outputs: [(
                        "result".into(),
                        WorkflowValue::Literal(serde_json::json!("custom parser result")),
                    )]
                    .into(),
                }],
            })
        }
    }
    struct Hook {
        meta: Meta,
        source: Handle<ts::WorkflowClient>,
    }
    impl Plugin<ts::TypeScript> for Hook {
        fn kind(&self) -> &'static str {
            "workflow-post-hook"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn enforce(&self) -> Enforce {
            Enforce::Post
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.source))]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let output = cx.inputs.get::<ts::WorkflowClient>()?;
            let symbol = &output.workflows["custom"];
            cx.files.emit(GeneratedFile::new(
                "hooks/custom.ts",
                format!(
                    "export {{ {} }} from '{}.js';",
                    symbol.name,
                    symbol.import_from("hooks/custom.ts")?
                ),
            )?)
        }
    }
    let input = CustomInput { meta: Meta::new() };
    let output = ts::workflow(Some(input.meta.handle()));
    let hook = Hook {
        meta: Meta::new(),
        source: output.handle(),
    };
    let tree = Packages::new()
        .package(ts::package("sdk").with(hook).with(output).with(input))
        .generate_native()
        .unwrap();
    assert!(
        tree.get("sdk/hooks/custom.ts")
            .unwrap()
            .contains("from '../workflows.js'")
    );
    assert!(
        tree.get("sdk/workflows.ts")
            .unwrap()
            .contains("custom parser result")
    );
}

#[test]
fn custom_consumer_selects_optional_workflow_step_blocks() {
    use poolster_core::{
        GeneratedFile,
        blocks::Blocks,
        engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
        native::workflows::WorkflowStepBlock,
    };
    struct Extract {
        meta: Meta,
        source: Handle<WorkflowOperations>,
    }
    impl Plugin<ts::TypeScript> for Extract {
        fn kind(&self) -> &'static str {
            "optional-workflow-step-transformer"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Blocks<WorkflowStepBlock>>()]
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.source))]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            cx.publish(
                cx.inputs
                    .get::<WorkflowOperations>()?
                    .step_blocks("checkout-source"),
            )
        }
    }
    struct Catalog {
        meta: Meta,
        source: Handle<Blocks<WorkflowStepBlock>>,
    }
    impl Plugin<ts::TypeScript> for Catalog {
        fn kind(&self) -> &'static str {
            "custom-workflow-step-catalog"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.source))]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let steps = cx.inputs.get::<Blocks<WorkflowStepBlock>>()?;
            let catalog: Vec<_> = steps
                .with_capability("poolster.http-operation")
                .map(|block| &block.metadata)
                .collect();
            cx.files.emit(GeneratedFile::new(
                "workflow-step-catalog.json",
                serde_json::to_string(&catalog)?,
            )?)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("workflow.yaml"), WORKFLOW).unwrap();
    std::fs::write(dir.path().join("shop.yaml"), OPENAPI).unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_arazzo::ArazzoInput)
        .unwrap();
    let input = InputProvider::<WorkflowOperations>::new(
        Arc::new(registry),
        "arazzo",
        dir.path().join("workflow.yaml"),
    )
    .with_options(InputOptions {
        workflow_sources: [("shop".into(), dir.path().join("shop.yaml"))].into(),
        ..Default::default()
    });
    let transform = Extract {
        meta: Meta::new(),
        source: input.handle(),
    };
    let catalog = Catalog {
        meta: Meta::new(),
        source: transform.meta.handle(),
    };
    let tree = Packages::new()
        .package(
            ts::package("catalog")
                .with(catalog)
                .with(transform)
                .with(input),
        )
        .generate_native()
        .unwrap();
    let catalog: serde_json::Value =
        serde_json::from_str(tree.get("catalog/workflow-step-catalog.json").unwrap()).unwrap();
    assert_eq!(catalog.as_array().unwrap().len(), 2);
    assert_eq!(
        catalog[1]["id"]["local"],
        "/workflows/checkout/steps/create"
    );
    assert!(tree.get("catalog/workflows.ts").is_none());
}
