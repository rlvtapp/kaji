use anyhow::Result;
use poolster_core::{
    AdaptedApi, Api, Operation, Schema, SchemaKind, SchemaValue,
    blocks::{Blocks, CollectionState, ContractReference},
    engine::*,
};
use poolster_plugin_java as output;
struct Source {
    meta: Meta,
    partial: bool,
    wrong_parent: bool,
}
impl Source {
    fn new() -> Self {
        Self {
            meta: Meta::new(),
            partial: false,
            wrong_parent: false,
        }
    }
}
impl<L: Language> Plugin<L> for Source {
    fn kind(&self) -> &'static str {
        "selected-http-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<AdaptedApi>(),
            Provision::of::<Blocks<Schema>>(),
            Provision::of::<Blocks<Operation>>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let whole = AdaptedApi {
            api: Api {
                name: "Selected".into(),
                version: "1.0.0".into(),
                schemas: vec![
                    Schema::new("Kept", SchemaValue::new(SchemaKind::String)),
                    Schema::new("DisappearedWireModel", SchemaValue::new(SchemaKind::String)),
                ],
                operations: vec![
                    Operation {
                        id: "changed".into(),
                        path: "/old".into(),
                        ..Default::default()
                    },
                    Operation {
                        id: "omitted".into(),
                        path: "/omitted".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let parent = ContractReference::from_bytes(AdaptedApi::NAME, "selected", b"fixture-v1");
        let mut models = whole.schema_blocks("selected").with_parent(parent.clone());
        models.items.retain(|block| block.value.name == "Kept");
        let mut endpoints = whole
            .endpoint_blocks("selected")
            .with_parent(parent.clone());
        endpoints.items.retain(|block| block.value.id == "changed");
        endpoints.items[0].value.path = "/transformed".into();
        if self.partial {
            endpoints.state = CollectionState::Partial {
                diagnostics: vec!["unsupported operation".into()],
            };
        }
        if self.wrong_parent {
            endpoints = endpoints.with_parent(ContractReference::from_bytes(
                AdaptedApi::NAME,
                "other",
                b"fixture-v1",
            ));
        }
        cx.publish_with_reference(whole, parent)?;
        cx.publish(models)?;
        cx.publish(endpoints)
    }
}
fn selected(partial: bool, wrong_parent: bool) -> Result<poolster_core::GeneratedTree> {
    let mut source = Source::new();
    source.partial = partial;
    source.wrong_parent = wrong_parent;
    let sdk = output::sdk()
        .input(source.meta.handle())
        .input_models(source.meta.handle())
        .input_endpoints(source.meta.handle());
    Packages::new()
        .package(output::package("sdk").with(source).with(sdk))
        .generate_native()
}
#[test]
fn selected_transformed_http_blocks_drive_generated_files() {
    let tree = selected(false, false).unwrap();
    let source = tree
        .iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(source.contains("/transformed"));
    assert!(!source.contains("/old"));
    assert!(!source.contains("/omitted"));
    assert!(source.contains("Kept"));
    assert!(!source.contains("DisappearedWireModel"));
}
#[test]
fn partial_and_foreign_parent_fail_before_generation() {
    let error = selected(true, false).unwrap_err();
    assert!(format!("{error:#}").contains("complete"));
    let error = selected(false, true).unwrap_err();
    assert!(format!("{error:#}").contains("parent"));
}

#[test]
fn typed_selection_preserves_legacy_renderer_bytes() {
    let api = Api {
        name: "Selected".into(),
        version: "1.0.0".into(),
        schemas: vec![Schema::new("Kept", SchemaValue::new(SchemaKind::String))],
        operations: vec![Operation {
            id: "changed".into(),
            path: "/transformed".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let legacy = Packages::new()
        .package(output::package("sdk").with(output::sdk()))
        .generate(&api, None)
        .unwrap();
    let typed = selected(false, false).unwrap();
    assert_eq!(
        legacy.iter().collect::<Vec<_>>(),
        typed.iter().collect::<Vec<_>>()
    );
}
