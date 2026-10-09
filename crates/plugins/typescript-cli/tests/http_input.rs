use anyhow::Result;
use poolster_core::{
    AdaptedApi, Api, HttpMethod, Operation,
    blocks::ContractReference,
    engine::{Contract, Meta, Packages, Plugin, PluginContext, Provision},
};
use poolster_plugin_typescript_cli as cli;
struct Source {
    meta: Meta,
}
impl Plugin<cli::TypeScriptCli> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, cli::TypeScriptCli>) -> Result<()> {
        cx.publish_with_reference(
            AdaptedApi::new(api(), Default::default()),
            ContractReference::from_bytes(AdaptedApi::NAME, "cli-source", b"selected"),
        )
    }
}
fn api() -> Api {
    Api {
        name: "Selected".into(),
        version: "1.2.3".into(),
        operations: vec![Operation {
            id: "getThing".into(),
            path: "/selected".into(),
            method: HttpMethod::Get,
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn explicit_whole_source_preserves_cli_bytes_and_sorts_dependency_before_consumer() {
    let legacy = Packages::new()
        .package(cli::package("sdk").with(cli::cli()))
        .generate(&api(), None)
        .unwrap();
    let source = Source { meta: Meta::new() };
    let generator = cli::cli().input(source.meta.handle());
    let selected = Packages::new()
        .package(cli::package("sdk").with(generator).with(source))
        .generate_native()
        .unwrap();
    assert_eq!(
        legacy.iter().collect::<Vec<_>>(),
        selected.iter().collect::<Vec<_>>()
    );
}

#[test]
fn selected_endpoint_blocks_replace_cli_routes_and_regenerate() {
    let run = || {
        let source = Source { meta: Meta::new() };
        let whole = source.meta.handle::<AdaptedApi>();
        let endpoints = poolster_core::engine::hooks::<cli::TypeScriptCli>().extract_blocks(
            Some(whole),
            |input| {
                let mut blocks = input.endpoint_blocks("cli-source");
                blocks.items[0].value.path = "/filtered".into();
                Ok(blocks)
            },
        );
        let generator = cli::cli().input(whole).input_endpoints(endpoints.handle());
        Packages::new()
            .package(
                cli::package("sdk")
                    .with(generator)
                    .with(endpoints)
                    .with(source),
            )
            .generate_native()
            .unwrap()
    };
    let tree = run();
    let text = tree
        .iter()
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("/filtered"));
    assert!(!text.contains("/selected"));
    assert_eq!(
        tree.iter().collect::<Vec<_>>(),
        run().iter().collect::<Vec<_>>()
    );
}
