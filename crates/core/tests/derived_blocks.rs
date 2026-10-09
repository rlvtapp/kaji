use anyhow::Result;
use poolster_core::{GeneratedFile, blocks::*, engine::*};
struct Lang;
impl Language for Lang {
    const NAME: &'static str = "test";
    type Settings = ();
    type Workspace = ();
}
struct Data(&'static str);
impl Contract for Data {
    const NAME: &'static str = "test.data.v1";
}
struct Item(&'static str);
impl Block for Item {
    const CONTRACT_NAME: &'static str = "test.items.v1";
}
struct Original {
    meta: Meta,
}
impl Plugin<Lang> for Original {
    fn kind(&self) -> &'static str {
        "original"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Data>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> Result<()> {
        cx.publish_with_reference(
            Data("old"),
            ContractReference::from_bytes(Data::NAME, "document", b"old"),
        )
    }
}
fn project(data: &Data) -> Result<Blocks<Item>> {
    Ok(Blocks {
        parent: None,
        state: CollectionState::Complete,
        items: vec![BuildingBlock {
            metadata: BlockMetadata {
                id: BlockId {
                    source: "document".into(),
                    local: "stable-model".into(),
                },
                parent: None,
                capabilities: Default::default(),
                references: vec![],
                location: None,
            },
            value: Item(data.0),
        }],
    })
}
fn run(mismatch: bool) -> Result<poolster_core::GeneratedTree> {
    let original = Original { meta: Meta::new() };
    let transform = hooks::<Lang>().provides::<Data>().on_contract(
        Some(original.meta.handle::<Data>()),
        |_, cx| {
            cx.publish_with_reference(
                Data("new"),
                ContractReference::from_bytes(Data::NAME, "document", b"new"),
            )
        },
    );
    let extractor = hooks::<Lang>().extract_blocks(Some(transform.handle::<Data>()), project);
    let chosen = if mismatch {
        original.meta.handle::<Data>()
    } else {
        transform.handle::<Data>()
    };
    let consumer = hooks::<Lang>().on_derived_block(
        Some(chosen),
        Some(extractor.handle::<Blocks<Item>>()),
        |block, cx| {
            assert_eq!(block.metadata.id.local, "stable-model");
            cx.files
                .emit(GeneratedFile::new("value.txt", block.value.0)?)
        },
    );
    Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .with(consumer)
                .with(extractor)
                .with(transform)
                .with(original),
        )
        .generate_native()
}
#[test]
fn selected_transform_derives_new_blocks_with_stable_entity_id() {
    assert_eq!(run(false).unwrap().get("sdk/value.txt"), Some("new"));
}
#[test]
fn pairing_original_with_transformed_blocks_fails_before_emission() {
    assert!(format!("{:#}", run(true).unwrap_err()).contains("revision"));
}
#[test]
fn completeness_is_independent_from_cardinality() {
    let mut blocks = project(&Data("old")).unwrap();
    blocks.state = CollectionState::Partial {
        diagnostics: vec!["unsupported second model".into()],
    };
    assert!(blocks.require_complete().is_err());
    blocks.items.clear();
    blocks.state = CollectionState::Complete;
    assert!(blocks.require_complete().is_ok());
    let parent = ContractReference::from_bytes(Data::NAME, "document", b"new");
    assert!(blocks.require_parent(&parent).is_err());
    blocks = blocks.with_parent(parent.clone());
    assert!(blocks.require_parent(&parent).is_ok());
}
#[test]
fn merge_does_not_promote_partial_data_to_complete() {
    let mut complete = project(&Data("old")).unwrap();
    let partial = Blocks::<Item> {
        parent: None,
        state: CollectionState::Partial {
            diagnostics: vec!["second source incomplete".into()],
        },
        items: vec![],
    };
    complete.merge(partial).unwrap();
    assert!(matches!(complete.state, CollectionState::Partial { .. }));
    assert!(complete.require_complete().is_err());
}
#[test]
fn malformed_revision_metadata_is_not_publishable() {
    let invalid = ContractReference {
        contract: Data::NAME.into(),
        instance: "document".into(),
        revision: String::new(),
    };
    assert!(invalid.validate().is_err());
}
