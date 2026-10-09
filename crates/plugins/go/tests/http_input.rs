use anyhow::Result;
use poolster_core::{
    AdaptedApi, Api, Operation, Schema, SchemaKind, SchemaValue,
    blocks::{Blocks, CollectionState, ContractReference},
    engine::*,
};
use poolster_plugin_go as output;
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

struct Consumer {
    meta: Meta,
}
impl Plugin<output::Go> for Consumer {
    fn kind(&self) -> &'static str {
        "selected-symbol-consumer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on::<output::providers::Models>(None),
            Requirement::on::<output::providers::Operations>(None),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, output::Go>) -> Result<()> {
        let models = cx.inputs.get::<output::providers::Models>()?;
        let operations = cx.inputs.get::<output::providers::Operations>()?;
        assert_eq!(models.symbols.len(), 1);
        assert_eq!(operations.methods.len(), 1);
        let symbol = models.symbols.get("Kept").unwrap();
        let method = operations.methods.get("changed").unwrap();
        cx.files.emit(poolster_core::GeneratedFile::new(
            "selected-symbols.txt",
            format!("{symbol}\n{method}\n"),
        )?)
    }
}
#[test]
fn downstream_consumers_read_actual_selected_symbols() {
    let source = Source::new();
    let sdk = output::sdk()
        .input(source.meta.handle())
        .input_models(source.meta.handle())
        .input_endpoints(source.meta.handle());
    let tree = Packages::new()
        .package(
            output::package("sdk")
                .with(source)
                .with(sdk)
                .with(Consumer { meta: Meta::new() }),
        )
        .generate_native()
        .unwrap();
    let recorded = tree.get("sdk/selected-symbols.txt").unwrap();
    for symbol in recorded.lines() {
        let symbol = symbol.rsplit("::").next().unwrap();
        assert!(
            tree.iter()
                .filter(|(path, _)| !path.ends_with("selected-symbols.txt"))
                .any(|(_, source)| source.contains(symbol)),
            "unrendered symbol {symbol}"
        );
    }
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

#[test]
fn independently_selected_parts_keep_runtime_and_symbols_in_sync() {
    let source = Source::new();
    let models = output::models()
        .input(source.meta.handle())
        .input_models(source.meta.handle())
        .input_endpoints(source.meta.handle());
    let transport = output::transport().input(source.meta.handle());
    let operations = output::operations()
        .input(source.meta.handle())
        .input_models(source.meta.handle())
        .input_endpoints(source.meta.handle())
        .using_models(models.models_handle())
        .using_transport(transport.transport_handle());
    let client = output::client()
        .input(source.meta.handle())
        .input_models(source.meta.handle())
        .input_endpoints(source.meta.handle())
        .using_operations(operations.operations_handle());
    let tree = Packages::new()
        .package(
            output::package("sdk")
                .with(source)
                .with(models)
                .with(transport)
                .with(operations)
                .with(client)
                .with(Consumer { meta: Meta::new() }),
        )
        .generate_native()
        .unwrap();
    let rendered = tree
        .iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("/transformed"));
    assert!(!rendered.contains("/old"));
    assert!(!rendered.contains("DisappearedWireModel"));
}
