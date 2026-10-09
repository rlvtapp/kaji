use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::RpcContract,
};
use poolster_plugin_go::{Go, GrpcToolchain, grpc};
use std::{path::Path, process::Command, sync::Arc};
fn generate(tools: &Path) -> anyhow::Result<poolster_core::GeneratedTree> {
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_protobuf::ProtobufInput)?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let output = grpc("example.com/poolster/grpc")
        .input(input.handle())
        .toolchain(GrpcToolchain {
            protoc: tools.join("protoc"),
            protoc_gen_go: tools.join("protoc-gen-go"),
            protoc_gen_go_grpc: tools.join("protoc-gen-go-grpc"),
        });
    Packages::new()
        .package(Package::<Go>::new("sdk").with(output).with(input))
        .generate_native()
}
#[test]
#[ignore = "requires Go and pinned protoc/protoc-gen-go/protoc-gen-go-grpc in POOLSTER_GRPC_TOOLS; fetches pinned Go module dependencies"]
fn official_grpc_package_compiles_and_runs_all_streams_against_local_server() {
    let tools = std::env::var("POOLSTER_GRPC_TOOLS").expect("POOLSTER_GRPC_TOOLS");
    let tree = generate(Path::new(&tools)).unwrap();
    assert!(
        tree.get("sdk/storev1/service_grpc.pb.go")
            .unwrap()
            .contains("StoreClient")
    );
    assert!(
        tree.get("sdk/storev1/common.pb.go")
            .unwrap()
            .contains("Note")
    );
    assert!(tree.get("sdk/proto/service.proto").is_some());
    let again = generate(Path::new(&tools)).unwrap();
    assert_eq!(tree, again);
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    std::fs::write(
        root.path().join("sdk/storev1/runtime_test.go"),
        include_str!("fixtures/grpc/runtime_test.go"),
    )
    .unwrap();
    // A handwritten implementation remains untouched across regeneration.
    let server = root.path().join("sdk/server.go");
    std::fs::write(&server, "package support\n// handwritten server\n").unwrap();
    again.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(server).unwrap(),
        "package support\n// handwritten server\n"
    );
    {
        let args = ["test", "-mod=readonly", "-race", "./..."];
        let result = Command::new("go")
            .args(args)
            .current_dir(root.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    // Compilation does not drift generated dependency files or block regeneration.
    again.write_to(root.path()).unwrap();
    let mut wrong = Path::new(&tools).to_path_buf();
    wrong.push("missing");
    assert!(generate(&wrong).is_err());
}

#[test]
#[ignore = "requires Go and pinned official gRPC generators in POOLSTER_GRPC_TOOLS; fetches Go module dependencies"]
fn pinned_upstream_proto2_package_compiles_with_official_generators() {
    use poolster_core::input::InputOptions;
    let tools = std::env::var("POOLSTER_GRPC_TOOLS").expect("POOLSTER_GRPC_TOOLS");
    let tools = Path::new(&tools);
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../inputs/protobuf/tests/fixtures/upstream");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let source = tempfile::tempdir().unwrap();
    let schema = source.path().join("upstream.proto");
    std::fs::write(&schema, r#"syntax="proto2"; package test; import "google/protobuf/unittest_import.proto"; option go_package="example.com/poolster/upstream/service;service"; service Imported { rpc Get(protobuf_unittest_import.ImportMessage) returns (protobuf_unittest_import.ImportMessage); }"#).unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", schema)
        .with_options(InputOptions {
            import_roots: vec![corpus],
            ..Default::default()
        });
    let output = grpc("example.com/poolster/upstream")
        .input(input.handle())
        .toolchain(GrpcToolchain {
            protoc: tools.join("protoc"),
            protoc_gen_go: tools.join("protoc-gen-go"),
            protoc_gen_go_grpc: tools.join("protoc-gen-go-grpc"),
        })
        .go_package(
            "google/protobuf/unittest_import.proto",
            "example.com/poolster/upstream/imports;imports",
        )
        .go_package(
            "google/protobuf/unittest_import_public.proto",
            "example.com/poolster/upstream/public;public",
        );
    let tree = Packages::new()
        .package(Package::<Go>::new("sdk").with(output).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    {
        let args = ["test", "-mod=readonly", "./..."];
        let result = Command::new("go")
            .args(args)
            .current_dir(root.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
