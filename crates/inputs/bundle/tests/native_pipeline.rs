#![cfg(any(
    feature = "graphql",
    feature = "asyncapi",
    feature = "arazzo",
    feature = "protobuf",
    feature = "capnproto"
))]
use anyhow::Result;
use poolster_core::{
    Api, GeneratedFile,
    engine::{
        Contract, Handle, Language, Meta, Package, Packages, Plugin, PluginContext, Requirement,
    },
};
use poolster_inputs::{InputProvider, InputSummary, default_registry};
use std::{marker::PhantomData, path::PathBuf, sync::Arc};
struct Docs;
impl Language for Docs {
    const NAME: &'static str = "contract-docs";
    type Settings = ();
    type Workspace = ();
}
struct Render<C: Contract> {
    meta: Meta,
    input: Handle<C>,
    summarize: fn(&C) -> InputSummary,
    marker: PhantomData<fn() -> C>,
}
impl<C: Contract> Plugin<Docs> for Render<C> {
    fn kind(&self) -> &'static str {
        "native-contract-docs"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.input))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Docs>) -> Result<()> {
        let summary = (self.summarize)(cx.inputs.get::<C>()?);
        let mut text = format!("# {}\nFormat: {}\n", summary.title, summary.format);
        for operation in summary.operations {
            text.push_str(&format!("{} {}\n", operation.kind, operation.name));
        }
        cx.files.emit(GeneratedFile::new("contract.md", text)?)
    }
}
fn pipeline<C: Contract>(
    format: &str,
    path: PathBuf,
    summarize: fn(&C) -> InputSummary,
    expected: &[&str],
) {
    let provider = InputProvider::<C>::new(Arc::new(default_registry().unwrap()), format, path);
    let renderer = Render {
        meta: Meta::new(),
        input: provider.handle(),
        summarize,
        marker: PhantomData,
    };
    let tree = Packages::new()
        .package(Package::<Docs>::new("docs").with(renderer).with(provider))
        .generate(&Api::default(), None)
        .unwrap();
    let artifact = tree.get("docs/contract.md").unwrap();
    for text in expected {
        assert!(artifact.contains(text), "missing {text:?} in {artifact}");
    }
}
fn fixture(format: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../{format}/tests/fixtures/{file}"))
}
#[cfg(feature = "graphql")]
#[test]
fn graphql_source_reaches_output_with_all_operation_kinds() {
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), "type Query { read: String } type Mutation { write: String } type Subscription { changed: String }").unwrap();
    pipeline::<poolster_inputs::graphql::GraphqlDocument>(
        "graphql",
        source.path().into(),
        |d| d.summary(),
        &["query read", "mutation write", "subscription changed"],
    );
}
#[cfg(feature = "asyncapi")]
#[test]
fn asyncapi_source_reaches_output_without_losing_event_semantics() {
    pipeline::<poolster_inputs::asyncapi::AsyncApiDocument>(
        "asyncapi",
        fixture("asyncapi", "events.yaml"),
        |d| d.summary(),
        &["Format: asyncapi", "send emitCreated"],
    );
}
#[cfg(feature = "arazzo")]
#[test]
fn arazzo_source_reaches_output_as_workflows() {
    pipeline::<poolster_inputs::arazzo::ArazzoDocument>(
        "arazzo",
        fixture("arazzo", "workflows.yaml"),
        |d| d.summary(),
        &["Format: arazzo", "workflow"],
    );
}
#[cfg(feature = "protobuf")]
#[test]
fn protobuf_source_reaches_output_with_each_streaming_mode() {
    pipeline::<poolster_inputs::protobuf::ProtobufDocument>(
        "protobuf",
        fixture("protobuf", "rpc/service.proto"),
        |d| d.summary(),
        &[
            "unary demo.Store.Get",
            "server_streaming demo.Store.Watch",
            "client_streaming demo.Store.Upload",
            "bidirectional_streaming demo.Store.Chat",
        ],
    );
}
#[cfg(feature = "capnproto")]
#[test]
fn capnproto_source_reaches_output_when_official_compiler_is_available() {
    if std::process::Command::new("capnp")
        .arg("--version")
        .output()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        eprintln!("SKIPPED Cap'n Proto source-to-output pipeline: capnp is unavailable");
        return;
    }
    pipeline::<poolster_inputs::capnproto::CapnProtoDocument>(
        "capnproto",
        fixture("capnproto", "rpc/service.capnp"),
        |d| d.summary(),
        &["Format: capnproto", "capability_rpc", "Store.get"],
    );
}

#[cfg(feature = "graphql")]
#[test]
fn large_github_schema_reaches_documentation_output() {
    pipeline::<poolster_inputs::graphql::GraphqlDocument>(
        "graphql",
        fixture("graphql", "github/schema.graphql"),
        |d| d.summary(),
        &["Format: graphql", "query repository", "mutation addStar"],
    );
}
#[cfg(feature = "asyncapi")]
#[test]
fn official_slack_events_reach_documentation_output() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../asyncapi/tests/corpus/slack-rtm-3.1.yaml");
    pipeline::<poolster_inputs::asyncapi::AsyncApiDocument>(
        "asyncapi",
        path,
        |d| d.summary(),
        &["Format: asyncapi", "send", "receive"],
    );
}
#[cfg(feature = "arazzo")]
#[test]
fn official_bnpl_workflow_reaches_documentation_output() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../arazzo/tests/corpus/bnpl-1.0.yaml");
    pipeline::<poolster_inputs::arazzo::ArazzoDocument>(
        "arazzo",
        path,
        |d| d.summary(),
        &["Format: arazzo", "workflow ApplyForLoanAtCheckout"],
    );
}
