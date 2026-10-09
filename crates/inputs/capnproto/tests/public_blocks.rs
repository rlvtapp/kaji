use poolster_core::input::InputPlugin;
use poolster_input_capnproto::{
    CapnProtoInput,
    blocks::{CapnpNodeKind, NodeBlocks},
    contracts::CapnProtoDocument,
};
#[test]
#[ignore = "requires official capnp compiler on PATH"]
fn compiler_provider_publishes_owned_nodes_and_native_wire_contract() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpc/service.capnp");
    let input = CapnProtoInput.load(&path).unwrap();
    assert!(
        !input
            .get::<CapnProtoDocument>()
            .unwrap()
            .schema_request
            .is_empty()
    );
    let nodes = input.get::<NodeBlocks>().unwrap();
    nodes.validate().unwrap();
    let interface = nodes
        .items
        .iter()
        .find(|node| node.value.kind == CapnpNodeKind::Interface)
        .unwrap();
    assert_eq!(interface.value.methods[0].name, "get");
    assert!(interface.metadata.capabilities.contains("rpc.capability"));
    for reference in &interface.metadata.references {
        assert!(nodes.resolve(reference).is_some());
    }
}
