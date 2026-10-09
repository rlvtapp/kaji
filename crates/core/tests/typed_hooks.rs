use anyhow::Result;
use poolster_core::{GeneratedFile, Schema, SchemaKind, SchemaValue, blocks::*, engine::*};
struct Lang;
impl Language for Lang {
    const NAME: &'static str = "test";
    type Settings = ();
    type Workspace = ();
}
// Deliberately non-Clone opaque contract.
struct Opaque(String);
impl Contract for Opaque {
    const NAME: &'static str = "example.opaque.v1";
}
struct Producer {
    meta: Meta,
}
impl Plugin<Lang> for Producer {
    fn kind(&self) -> &'static str {
        "producer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Opaque>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> Result<()> {
        cx.publish(Opaque("native".into()))
    }
}
fn blocks() -> Blocks<Schema> {
    Blocks {
        parent: None,
        state: CollectionState::Complete,
        items: vec![BuildingBlock {
            metadata: BlockMetadata {
                parent: None,
                id: BlockId {
                    source: "spec".into(),
                    local: "Name".into(),
                },
                capabilities: ["model".into()].into(),
                references: vec![],
                location: None,
            },
            value: Schema::new("Name", SchemaValue::new(SchemaKind::String)),
        }],
    }
}
#[test]
fn opaque_transform_blocks_and_post_handlers_follow_dependencies() {
    let producer = Producer { meta: Meta::new() };
    let transform = hooks::<Lang>().provides::<Blocks<Schema>>().on_contract(
        Some(producer.meta.handle::<Opaque>()),
        |whole, cx| {
            assert_eq!(whole.0, "native");
            cx.publish(blocks())
        },
    );
    let consumer = hooks::<Lang>()
        .on_contract(Some(transform.handle::<Blocks<Schema>>()), |models, _| {
            assert_eq!(models.items.len(), 1);
            Ok(())
        })
        .on_model(Some(transform.handle::<Blocks<Schema>>()), |model, cx| {
            cx.files
                .emit(GeneratedFile::new("models.txt", &model.value.name)?)
        })
        .on_contract(Some(producer.meta.handle::<Opaque>()), |whole, cx| {
            cx.files.emit(GeneratedFile::new("whole.txt", &whole.0)?)
        });
    let post = hooks::<Lang>().phase(PluginPhase::Post).on_contract(
        Some(transform.handle::<Blocks<Schema>>()),
        |models, cx| {
            cx.files.emit(GeneratedFile::new(
                "summary.txt",
                models.items.len().to_string(),
            )?)
        },
    );
    let tree = Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .with(post)
                .with(consumer)
                .with(transform)
                .with(producer),
        )
        .generate_native()
        .unwrap();
    assert_eq!(tree.get("sdk/models.txt"), Some("Name"));
    assert_eq!(tree.get("sdk/whole.txt"), Some("native"));
    assert_eq!(tree.get("sdk/summary.txt"), Some("1"));
}
#[test]
fn optional_absent_whole_contract_does_not_require_blocks() {
    let hook = hooks::<Lang>().on_optional_contract(None::<Handle<Opaque>>, |_, _| {
        panic!("absent optional contract must not run")
    });
    assert!(
        Packages::new()
            .package(Package::<Lang>::new("sdk").with(hook))
            .generate_native()
            .is_ok()
    );
}
#[test]
fn required_missing_whole_contract_fails_before_callback() {
    let hook = hooks::<Lang>().on_contract(None::<Handle<Opaque>>, |_, _| panic!("must not run"));
    assert!(
        Packages::new()
            .package(Package::<Lang>::new("sdk").with(hook))
            .generate_native()
            .is_err()
    );
}
#[test]
fn duplicate_block_identity_is_rejected_transactionally() {
    let mut left = blocks();
    assert!(left.merge(blocks()).is_err());
    assert_eq!(left.items.len(), 1);
    let mut right = blocks();
    right.items[0].metadata.id.source = "other".into();
    left.merge(right).unwrap();
    assert_eq!(left.items.len(), 2);
}
