use super::*;
use crate::blocks::BlockId;
use crate::{
    GeneratedFile,
    engine::{Handle, Language, Meta, Package, Packages, Plugin, PluginContext, Requirement},
};
use anyhow::Result;
struct Example;
impl Language for Example {
    const NAME: &'static str = "symbols-example";
    type Settings = ();
    type Workspace = ();
}
struct Emit {
    meta: Meta,
    provider: Handle<ResolvedSymbols>,
}
impl Plugin<Example> for Emit {
    fn kind(&self) -> &'static str {
        "symbol-example-emission"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.provider))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Example>) -> Result<()> {
        let names = cx.inputs.get::<ResolvedSymbols>()?;
        let code = names
            .iter()
            .map(|symbol| {
                format!(
                    "export type {} = {{ source: {:?} }};\n",
                    symbol.name, symbol.key.entity.source
                )
            })
            .collect::<String>();
        cx.files.emit(GeneratedFile::new("models.ts", code)?)?;
        cx.files.emit(GeneratedFile::new(
            "symbols.json",
            serde_json::to_string(names)?,
        )?)
    }
}
#[test]
fn real_plugin_graph_runs_reservation_resolution_then_emission() {
    let requests = [
        SymbolRequest {
            entity: BlockId {
                source: "z-source".into(),
                local: "#/Order".into(),
            },
            target: "typescript".into(),
            module: "models".into(),
            preferred: "Order".into(),
        },
        SymbolRequest {
            entity: BlockId {
                source: "a-source".into(),
                local: "#/Order".into(),
            },
            target: "typescript".into(),
            module: "models".into(),
            preferred: "Order".into(),
        },
    ];
    let reserve = reserve_symbol_requests(requests).reserve_name("typescript", "models", "Array");
    let resolve = resolve_symbol_requests(Some(reserve.handle()));
    let emit = Emit {
        meta: Meta::new(),
        provider: resolve.handle(),
    };
    // Reverse registration deliberately; typed requirements enforce the lifecycle.
    let files = Packages::new()
        .package(
            Package::<Example>::new("example")
                .with(emit)
                .with(resolve)
                .with(reserve),
        )
        .generate_native()
        .unwrap();
    let source = files.get("example/models.ts").unwrap();
    assert!(source.contains("export type Order = { source: \"a-source\" }"));
    assert!(source.contains("export type Order_2 = { source: \"z-source\" }"));
    let manifest: serde_json::Value =
        serde_json::from_str(files.get("example/symbols.json").unwrap()).unwrap();
    assert_eq!(manifest[0]["name"], "Order");
}
