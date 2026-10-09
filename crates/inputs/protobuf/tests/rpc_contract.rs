use poolster_core::{
    input::{InputOptions, InputPlugin, InputRegistry},
    native::rpc::{RpcContract, RpcStreaming},
};
use poolster_input_protobuf::{ProtobufDocument, ProtobufInput};
use std::path::Path;
#[test]
fn owned_rpc_preserves_streaming_imports_and_native_wire_semantics() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let loaded = ProtobufInput.load(&path).unwrap();
    let owned = loaded.get::<RpcContract>().unwrap();
    let native = loaded.get::<ProtobufDocument>().unwrap();
    assert_eq!(owned.root_files, vec!["service.proto"]);
    assert_eq!(owned.services[0].full_name, "demo.Store");
    assert_eq!(
        owned.services[0]
            .methods
            .iter()
            .map(|m| m.streaming)
            .collect::<Vec<_>>(),
        vec![
            RpcStreaming::Unary,
            RpcStreaming::Server,
            RpcStreaming::Client,
            RpcStreaming::Bidirectional
        ]
    );
    assert!(
        owned
            .files
            .iter()
            .find(|f| f.name == "service.proto")
            .unwrap()
            .source
            .as_ref()
            .unwrap()
            .contains("rpc Chat")
    );
    let decoded =
        protox::prost_reflect::DescriptorPool::decode(owned.descriptor_set.as_slice()).unwrap();
    let item = decoded.get_message_by_name("demo.Item").unwrap();
    assert!(item.get_field_by_name("attributes").unwrap().is_map());
    assert_eq!(decoded.encode_to_vec(), native.descriptors.encode_to_vec());
}
#[test]
fn imports_resolve_through_configured_roots_and_substitution_publishes_owned_capability() {
    let root = tempfile::tempdir().unwrap();
    let schemas = root.path().join("schemas");
    let includes = root.path().join("includes");
    std::fs::create_dir_all(&schemas).unwrap();
    std::fs::create_dir_all(&includes).unwrap();
    let path = schemas.join("api.proto");
    std::fs::write(&path,r#"syntax="proto3"; package demo; import "types.proto"; option go_package="example.com/demo/api;api"; service Store {rpc Get(Item) returns(Item);}"#).unwrap();
    std::fs::write(includes.join("types.proto"),r#"syntax="proto3";package demo;option go_package="example.com/demo/api;api";message Item {optional string name=1;oneof value{int32 count=2; string text=3;} map<string,string> labels=4;}"#).unwrap();
    assert!(ProtobufInput.load(&path).is_err());
    let options = InputOptions {
        import_roots: vec![includes],
        ..Default::default()
    };
    struct Replacement;
    impl InputPlugin for Replacement {
        fn id(&self) -> &str {
            "protobuf.replacement"
        }
        fn format(&self) -> &str {
            "protobuf"
        }
        fn load(&self, p: &Path) -> anyhow::Result<poolster_core::input::InputContract> {
            ProtobufInput.load(p)
        }
        fn load_with_options(
            &self,
            p: &Path,
            o: &InputOptions,
        ) -> anyhow::Result<poolster_core::input::InputContract> {
            ProtobufInput.load_with_options(p, o)
        }
    }
    let mut registry = InputRegistry::new();
    registry.register(ProtobufInput).unwrap();
    registry.register(Replacement).unwrap();
    assert!(
        registry
            .load_with_options("protobuf", None, &path, &options)
            .is_err()
    );
    let loaded = registry
        .load_with_options("protobuf", Some("protobuf.replacement"), &path, &options)
        .unwrap();
    let rpc = loaded.contract.get::<RpcContract>().unwrap();
    assert_eq!(rpc.files.len(), 2);
    assert!(
        rpc.files
            .iter()
            .all(|f| f.go_package.as_deref() == Some("example.com/demo/api;api"))
    );
    let pool =
        protox::prost_reflect::DescriptorPool::decode(rpc.descriptor_set.as_slice()).unwrap();
    let item = pool.get_message_by_name("demo.Item").unwrap();
    assert!(item.get_field_by_name("name").unwrap().supports_presence());
    assert!(
        item.get_field_by_name("count")
            .unwrap()
            .containing_oneof()
            .is_some()
    );
    assert!(item.get_field_by_name("labels").unwrap().is_map());
    let invalid = InputOptions {
        operation_files: vec![path.clone()],
        ..options
    };
    assert!(ProtobufInput.load_with_options(&path, &invalid).is_err());
}

#[test]
fn optional_rpc_blocks_preserve_service_identity_and_are_consumed_through_typed_graph() {
    use poolster_core::{GeneratedFile, blocks::Blocks, engine::*, native::rpc::RpcMethodBlock};
    use std::sync::Arc;
    struct Language;
    impl poolster_core::engine::Language for Language {
        const NAME: &'static str = "test";
        type Settings = ();
        type Workspace = ();
        fn finalize(_: &mut FinalizeContext<'_, Self>) -> anyhow::Result<()> {
            Ok(())
        }
    }
    struct Extract(Meta);
    impl Plugin<Language> for Extract {
        fn kind(&self) -> &'static str {
            "rpc-block-extractor"
        }
        fn meta(&self) -> &Meta {
            &self.0
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(None::<Handle<RpcContract>>)]
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Blocks<RpcMethodBlock>>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
            cx.publish(
                cx.inputs
                    .get::<RpcContract>()?
                    .method_blocks("store.schema"),
            )
        }
    }
    struct Output(Meta);
    impl Plugin<Language> for Output {
        fn kind(&self) -> &'static str {
            "rpc-block-output"
        }
        fn meta(&self) -> &Meta {
            &self.0
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(None::<Handle<Blocks<RpcMethodBlock>>>)]
        }
        fn generate(&self, cx: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
            let blocks = cx.inputs.get::<Blocks<RpcMethodBlock>>()?;
            assert_eq!(blocks.items.len(), 4);
            assert_eq!(blocks.with_capability("rpc.streaming").count(), 3);
            assert_eq!(
                blocks
                    .with_capability("rpc.bidirectional-streaming")
                    .count(),
                1
            );
            for block in &blocks.items {
                assert_eq!(block.value.service, "demo.Store");
                assert_eq!(block.value.file, "service.proto");
                assert_eq!(block.metadata.id.source, "store.schema");
                assert_eq!(block.metadata.id.local, block.value.method.full_name);
                assert!(
                    block
                        .metadata
                        .references
                        .iter()
                        .all(|r| r.contract == RpcContract::NAME)
                );
            }
            cx.files.emit(GeneratedFile::new(
                "streaming.txt",
                blocks.with_capability("rpc.streaming").count().to_string(),
            )?)
        }
    }
    let mut registry = InputRegistry::new();
    registry.register(ProtobufInput).unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let input = poolster_core::input::InputProvider::<RpcContract>::new(
        Arc::new(registry),
        "protobuf",
        path,
    );
    let tree = Packages::new()
        .package(
            Package::<Language>::new("sdk")
                .with(Output(Meta::new()))
                .with(Extract(Meta::new()))
                .with(input),
        )
        .generate_native()
        .unwrap();
    assert_eq!(tree.get("sdk/streaming.txt"), Some("3"));
}

#[test]
fn relative_import_roots_match_canonical_input_paths() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let cwd = std::env::current_dir().unwrap();
    let relative = Path::new("tests/fixtures/rpc");
    let root = if cwd.join(relative).is_dir() {
        relative.to_path_buf()
    } else {
        Path::new("crates/inputs/protobuf/tests/fixtures/rpc").to_path_buf()
    };
    let doc = poolster_input_protobuf::load_with_includes(&path, &[root]).unwrap();
    assert_eq!(doc.root_file, "service.proto");
}

#[test]
fn rpc_block_identity_context_and_edits_do_not_mutate_wire_contract() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let contract = poolster_input_protobuf::load(&path).unwrap().rpc_contract();
    let descriptors = contract.descriptor_set.clone();
    let mut blocks = contract.method_blocks("stable.source");
    let other = contract.method_blocks("another.source");
    assert_ne!(blocks.items[0].metadata.id, other.items[0].metadata.id);
    assert_eq!(
        blocks.items[0].metadata.id.local,
        other.items[0].metadata.id.local
    );
    let native = protox::prost_reflect::DescriptorPool::decode(descriptors.as_slice()).unwrap();
    for item in &blocks.items {
        for reference in &item.metadata.references {
            assert_eq!(reference.id.source, "stable.source");
            assert!(native.get_message_by_name(&reference.id.local).is_some());
        }
    }
    blocks.items[0].value.method.name = "Replacement".into();
    blocks.items[0]
        .metadata
        .capabilities
        .insert("example.custom-capability".into());
    assert_eq!(
        blocks.with_capability("example.custom-capability").count(),
        1
    );
    assert_ne!(
        blocks.items[0].value.method,
        contract.services[0].methods[0]
    );
    assert_eq!(contract.descriptor_set, descriptors);
}

#[test]
fn provider_directly_publishes_native_rpc_and_method_blocks() {
    use poolster_core::{blocks::Blocks, native::rpc::RpcMethodBlock};
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let mut registry = InputRegistry::new();
    registry.register(ProtobufInput).unwrap();
    let loaded = registry.load("protobuf", None, &path).unwrap().contract;
    let options = ProtobufInput
        .load_with_options(&path, &InputOptions::default())
        .unwrap();
    assert!(loaded.get::<ProtobufDocument>().is_ok());
    assert!(loaded.get::<RpcContract>().is_ok());
    let blocks = loaded.get::<Blocks<RpcMethodBlock>>().unwrap();
    assert_eq!(blocks.items.len(), 4);
    assert_eq!(blocks, options.get::<Blocks<RpcMethodBlock>>().unwrap());
    let source = path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    assert!(
        blocks
            .items
            .iter()
            .all(|item| item.metadata.id.source == source)
    );
    assert_eq!(blocks.with_capability("rpc.streaming").count(), 3);
}

#[test]
fn package_public_contract_and_block_apis_preserve_shared_identities() {
    use poolster_core::engine::Contract;
    use poolster_input_protobuf::{blocks, contracts};
    assert_eq!(contracts::RpcContract::NAME, RpcContract::NAME);
    assert_eq!(
        blocks::RpcMethodBlocks::NAME,
        "poolster.rpc-method-blocks.v1"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.proto");
    let loaded = ProtobufInput.load(&path).unwrap();
    let contract: &contracts::RpcContract = loaded.get().unwrap();
    let published: &blocks::RpcMethodBlocks = loaded.get().unwrap();
    let projected = blocks::method_blocks(contract, "application.document");
    assert_eq!(projected.items.len(), published.items.len());
    assert_eq!(
        projected.items[0].metadata.id.source,
        "application.document"
    );
    let _: &blocks::BuildingBlock<blocks::RpcMethodBlock> = &projected.items[0];
}
