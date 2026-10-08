#![cfg(feature = "graphql")]
use anyhow::Result;
use kaji_core::{
    Api, GeneratedFile,
    engine::{
        Contract, Handle, Language, Meta, Package, Packages, Plugin, PluginContext, Requirement,
    },
};
use kaji_inputs::{
    InputContract, InputPlugin, InputProvider, default_registry, graphql::GraphqlDocument,
};
use std::{path::Path, sync::Arc};

struct Reference;
impl Language for Reference {
    const NAME: &'static str = "reference";
    type Settings = ();
    type Workspace = ();
}
struct Render {
    meta: Meta,
    source: Handle<GraphqlDocument>,
}
impl Plugin<Reference> for Render {
    fn kind(&self) -> &'static str {
        "graphql-reference"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Reference>) -> Result<()> {
        let schema = cx.inputs.get::<GraphqlDocument>()?;
        let summary = schema.summary();
        let operations = summary
            .operations
            .iter()
            .map(|operation| format!("{} {}", operation.kind, operation.name))
            .collect::<Vec<_>>()
            .join("\n");
        cx.files
            .emit(GeneratedFile::new("operations.md", operations)?)
    }
}

#[test]
fn input_plugin_drives_output_through_actual_typed_graph() {
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        source.path(),
        "type Query { hello: String! } type Mutation { change: Boolean! }",
    )
    .unwrap();
    let provider = InputProvider::<GraphqlDocument>::new(
        Arc::new(default_registry().unwrap()),
        "graphql",
        source.path(),
    )
    .using("graphql.apollo");
    let renderer = Render {
        meta: Meta::new(),
        source: provider.handle(),
    };
    // Consumer is registered first: declared contracts establish execution order.
    let generated = Packages::new()
        .package(
            Package::<Reference>::new("docs")
                .with(renderer)
                .with(provider),
        )
        .generate(&Api::default(), None)
        .unwrap();
    assert_eq!(
        generated.get("docs/operations.md"),
        Some("query hello\nmutation change")
    );
}

struct Replacement;
impl InputPlugin for Replacement {
    fn id(&self) -> &str {
        "graphql.custom"
    }
    fn format(&self) -> &str {
        "graphql"
    }
    fn load(&self, _: &Path) -> Result<InputContract> {
        let document = kaji_inputs::graphql::parse("type Query { replacement: String }")?;
        let mut contract = InputContract::new(document.summary());
        contract.publish(document)?;
        Ok(contract)
    }
}

#[test]
fn substitute_provider_changes_consumer_without_changes_to_core_or_output() {
    let mut registry = default_registry().unwrap();
    registry.register(Replacement).unwrap();
    let provider =
        InputProvider::<GraphqlDocument>::new(Arc::new(registry), "graphql", "unused.graphql")
            .using("graphql.custom");
    let renderer = Render {
        meta: Meta::new(),
        source: provider.handle(),
    };
    let generated = Packages::new()
        .package(
            Package::<Reference>::new("docs")
                .with(provider)
                .with(renderer),
        )
        .generate(&Api::default(), None)
        .unwrap();
    assert_eq!(
        generated.get("docs/operations.md"),
        Some("query replacement")
    );
}

#[test]
fn wrong_native_contract_fails_before_emission() {
    struct Unsupported;
    impl Contract for Unsupported {
        const NAME: &'static str = "example.unsupported";
    }
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), "type Query { hello: String }").unwrap();
    let input = default_registry()
        .unwrap()
        .load("graphql", None, source.path())
        .unwrap();
    assert!(
        input
            .contract
            .get::<Unsupported>()
            .err()
            .unwrap()
            .to_string()
            .contains("example.unsupported")
    );
}

#[test]
fn two_packages_keep_their_input_contracts_isolated() {
    let directory = tempfile::tempdir().unwrap();
    let registry = Arc::new(default_registry().unwrap());
    let mut packages = Packages::new();
    for (name, operation) in [("first", "alpha"), ("second", "beta")] {
        let path = directory.path().join(format!("{name}.graphql"));
        std::fs::write(&path, format!("type Query {{ {operation}: String }}")).unwrap();
        let provider = InputProvider::<GraphqlDocument>::new(registry.clone(), "graphql", path);
        let renderer = Render {
            meta: Meta::new(),
            source: provider.handle(),
        };
        packages = packages.package(
            Package::<Reference>::new(name)
                .with(renderer)
                .with(provider),
        );
    }
    let tree = packages.generate(&Api::default(), None).unwrap();
    assert_eq!(tree.get("first/operations.md"), Some("query alpha"));
    assert_eq!(tree.get("second/operations.md"), Some("query beta"));
}

#[test]
fn graph_propagates_parser_failure_before_consumer_runs() {
    struct MustNotRun {
        meta: Meta,
        source: Handle<GraphqlDocument>,
    }
    impl Plugin<Reference> for MustNotRun {
        fn kind(&self) -> &'static str {
            "must-not-run"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.source))]
        }
        fn generate(&self, _: &mut PluginContext<'_, Reference>) -> Result<()> {
            panic!("consumer executed after input failure")
        }
    }
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), "type Query { hello: Unknown }").unwrap();
    let provider = InputProvider::<GraphqlDocument>::new(
        Arc::new(default_registry().unwrap()),
        "graphql",
        source.path(),
    );
    let renderer = MustNotRun {
        meta: Meta::new(),
        source: provider.handle(),
    };
    let error = Packages::new()
        .package(
            Package::<Reference>::new("docs")
                .with(renderer)
                .with(provider),
        )
        .generate(&Api::default(), None)
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("invalid GraphQL schema"));
}
