use anyhow::Result;
use poolster_core::{
    AdaptedApi, Api, HttpMethod, Operation, Schema, SchemaKind, SchemaValue,
    blocks::{Blocks, CollectionState, ContractReference},
    engine::{Contract, Meta, Packages, Plugin, PluginContext, Provision, hooks},
};
use poolster_plugin_typescript as ts;

fn api(name: &str) -> Api {
    Api {
        name: name.into(),
        version: "1.2.3".into(),
        schemas: vec![Schema::new("Thing", SchemaValue::new(SchemaKind::String))],
        operations: vec![Operation {
            id: "getThing".into(),
            path: format!("/{name}"),
            method: HttpMethod::Get,
            ..Default::default()
        }],
        ..Default::default()
    }
}
struct Source {
    meta: Meta,
}
impl Source {
    fn new() -> Self {
        Self { meta: Meta::new() }
    }
}
impl Plugin<ts::TypeScript> for Source {
    fn kind(&self) -> &'static str {
        "test-http-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<AdaptedApi>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> Result<()> {
        cx.publish_with_reference(
            AdaptedApi::new(api("original"), Default::default()),
            ContractReference::from_bytes(AdaptedApi::NAME, "source", b"original"),
        )
    }
}
#[test]
fn whole_input_substitution_preserves_legacy_bytes_and_registration_order() {
    let legacy = Packages::new()
        .package(ts::package("sdk").with(ts::sdk()))
        .generate(&api("original"), None)
        .unwrap();
    let source = Source::new();
    let sdk = ts::sdk().input(source.meta.handle());
    let native = Packages::new()
        .package(ts::package("sdk").with(sdk).with(source))
        .generate_native()
        .unwrap();
    assert_eq!(
        legacy.iter().collect::<Vec<_>>(),
        native.iter().collect::<Vec<_>>()
    );
}
fn transformed(mismatch: bool, partial: bool) -> Result<poolster_core::GeneratedTree> {
    let source = Source::new();
    let original = source.meta.handle::<AdaptedApi>();
    let transform = hooks::<ts::TypeScript>()
        .provides::<AdaptedApi>()
        .on_contract(Some(original), |_, cx| {
            cx.publish_with_reference(
                AdaptedApi::new(api("transformed"), Default::default()),
                ContractReference::from_bytes(AdaptedApi::NAME, "source", b"transformed"),
            )
        });
    let whole = transform.handle::<AdaptedApi>();
    let models = hooks::<ts::TypeScript>().extract_blocks(Some(whole), move |input| {
        let mut models = input.schema_blocks("source");
        models.items[0].value.name = "SelectedThing".into();
        if partial {
            models.state = CollectionState::Partial {
                diagnostics: vec!["missing second model".into()],
            };
        }
        Ok(models)
    });
    let endpoints = hooks::<ts::TypeScript>().extract_blocks(Some(whole), |input| {
        let mut endpoints = input.endpoint_blocks("source");
        endpoints.items[0].value.id = "selectedThing".into();
        Ok(endpoints)
    });
    let chosen = if mismatch { original } else { whole };
    let schema_handle = models.handle::<Blocks<Schema>>();
    let endpoint_handle = endpoints.handle::<Blocks<Operation>>();
    let provider = ts::composition::models()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle);
    let model_symbols = provider.models_handle();
    let transport = ts::composition::transport().input(chosen);
    let transport_symbols = transport.transport_handle();
    let operations = ts::composition::operations()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols)
        .using_transport(transport_symbols);
    let operation_symbols = operations.operations_handle();
    let client = ts::composition::client()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_operations(operation_symbols)
        .using_transport(transport_symbols);
    let query = ts::composition::react_query()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_operations(operation_symbols);
    let zod = ts::composition::zod()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols);
    let faker = ts::composition::faker()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols);
    let msw = ts::composition::msw()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols)
        .using_operations(operation_symbols);
    let cypress = ts::composition::cypress()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols)
        .using_operations(operation_symbols);
    let vue_query = ts::composition::vue_query()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_operations(operation_symbols);
    let swr = ts::composition::swr()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_operations(operation_symbols);
    let tests = ts::operation_tests()
        .input(chosen)
        .input_models(schema_handle)
        .input_endpoints(endpoint_handle)
        .using_models(model_symbols)
        .using_operations(operation_symbols)
        .using_transport(transport_symbols);
    Packages::new()
        .package(
            ts::package("sdk")
                .with(query)
                .with(faker)
                .with(msw)
                .with(cypress)
                .with(vue_query)
                .with(swr)
                .with(tests)
                .with(zod)
                .with(client)
                .with(operations)
                .with(transport)
                .with(provider)
                .with(endpoints)
                .with(models)
                .with(transform)
                .with(source),
        )
        .generate_native()
}
#[test]
fn selected_transformed_blocks_drive_models_operations_and_helpers_and_regenerate() {
    let tree = transformed(false, false).unwrap();
    let combined = tree
        .iter()
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("SelectedThing"));
    assert!(combined.contains("selectedThing"));
    assert!(combined.contains("/transformed"));
    assert!(!combined.contains("/original"));
    assert_eq!(
        tree.iter().collect::<Vec<_>>(),
        transformed(false, false)
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
    );
}
#[test]
fn stale_or_incomplete_selected_blocks_fail_before_generation() {
    assert!(format!("{:#}", transformed(true, false).unwrap_err()).contains("revision"));
    assert!(
        format!("{:#}", transformed(false, true).unwrap_err()).contains("missing second model")
    );
}
#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS (TypeScript5.9.3)"]
fn selected_whole_sdk_compiles() {
    let source = Source::new();
    let sdk = ts::sdk().input(source.meta.handle());
    let tree = Packages::new()
        .package(ts::package("sdk").with(sdk).with(source))
        .generate_native()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let output = std::process::Command::new("node")
        .args([
            &std::env::var("POOLSTER_TSC_JS").unwrap(),
            "-p",
            "tsconfig.json",
        ])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Node, POOLSTER_TSC_JS and POOLSTER_TS_NODE_MODULES with auxiliary/framework dependencies"]
fn transformed_block_models_and_all_helpers_compile_together() {
    let tree = transformed(false, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let cwd = dir.path().join("sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        std::env::var_os("POOLSTER_TS_NODE_MODULES").unwrap(),
        cwd.join("node_modules"),
    )
    .unwrap();
    let output = std::process::Command::new("node")
        .args([
            &std::env::var("POOLSTER_TSC_JS").unwrap(),
            "-p",
            "tsconfig.json",
        ])
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn standalone_types_select_transformed_model_blocks_and_publish_actual_symbols() {
    let source = Source::new();
    let whole = source.meta.handle::<AdaptedApi>();
    let blocks = hooks::<ts::TypeScript>().extract_blocks(Some(whole), |input| {
        let mut models = input.schema_blocks("source");
        models.items[0].value.name = "ChosenModel".into();
        Ok(models)
    });
    let types = ts::types().input(whole).input_models(blocks.handle());
    let consumer = hooks::<ts::TypeScript>().on_contract(Some(types.handle()), |types, cx| {
        let chosen = &types.schemas["ChosenModel"];
        assert!(!types.schemas.contains_key("Thing"));
        cx.files.emit(poolster_core::GeneratedFile::new(
            "selected.txt",
            chosen.name.clone(),
        )?)
    });
    let tree = Packages::new()
        .package(
            ts::package("sdk")
                .with(consumer)
                .with(types)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
    assert_eq!(tree.get("sdk/selected.txt"), Some("ChosenModel"));
    assert!(tree.get("sdk/models.ts").unwrap().contains("ChosenModel"));
}
