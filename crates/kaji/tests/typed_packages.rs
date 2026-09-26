mod support;
use kaji::{dotnet, elixir, go, java, php, prelude::*, python, rust, ts};

struct CommunityConsumer {
    meta: Meta,
    types: Handle<ts::TsTypes>,
}

impl Plugin<ts::TypeScript> for CommunityConsumer {
    fn kind(&self) -> &'static str {
        "community-consumer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.types))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
        let symbol = &cx.inputs.get::<ts::TsTypes>()?.schemas["Contact"];
        let module = symbol.import_from("custom/consumer.ts")?;
        cx.workspace.dependency("example-runtime", "^1.0.0")?;
        cx.files.emit(GeneratedFile::new(
            "custom/consumer.ts",
            format!("import type {{ {} }} from \"{module}\";\n", symbol.name),
        )?)
    }
}

#[test]
fn community_consumer_uses_real_types_and_shared_package_dependencies() {
    let types = ts::types().output("generated/models");
    let consumer = CommunityConsumer {
        meta: Meta::new(),
        types: types.handle(),
    };
    let tree = kaji::generate(
        &support::sdk_contract_api(),
        ProfileSet::new("sdk").package(
            ts::package("models")
                .name("@acme/models")
                .with(consumer)
                .with(types),
        ),
    )
    .unwrap();
    assert_eq!(
        tree.get("sdk/models/custom/consumer.ts"),
        Some("import type { Contact } from \"../generated/models\";\n")
    );
    assert!(tree.get("sdk/models/generated/models.ts").is_some());
    let manifest: serde_json::Value =
        serde_json::from_str(tree.get("sdk/models/package.json").unwrap()).unwrap();
    assert_eq!(manifest["name"], "@acme/models");
    assert_eq!(manifest["dependencies"]["example-runtime"], "^1.0.0");
    assert!(
        tree.get("sdk/models/index.ts")
            .unwrap()
            .contains("./generated/models")
    );
}

#[test]
fn typed_packages_match_all_existing_language_outputs_byte_for_byte() {
    let api = support::sdk_contract_api();
    let old = kaji::generate(
        &api,
        ProfileSet::new("sdk")
            .rust()
            .typescript_fetch()
            .typescript_axios()
            .go()
            .python()
            .php()
            .java()
            .dotnet()
            .elixir(),
    )
    .unwrap();
    let new = kaji::generate(
        &api,
        ProfileSet::new("sdk")
            .package(rust::package("rust").with(rust::sdk()))
            .package(ts::package("typescript-fetch").with(ts::sdk().fetch()))
            .package(ts::package("typescript-axios").with(ts::sdk().axios()))
            .package(go::package("go").with(go::sdk()))
            .package(python::package("python").with(python::sdk()))
            .package(php::package("php").with(php::sdk()))
            .package(java::package("java").with(java::sdk()))
            .package(dotnet::package("dotnet").with(dotnet::sdk()))
            .package(elixir::package("elixir").with(elixir::sdk())),
    )
    .unwrap();
    for (path, contents) in old.iter() {
        assert_eq!(
            Some(contents),
            new.get(path),
            "different file: {}",
            path.display()
        );
        assert_eq!(
            old.preserves_existing(path),
            new.preserves_existing(path),
            "preservation: {}",
            path.display()
        );
    }
    assert_eq!(old.iter().count(), new.iter().count());
}

#[test]
fn typescript_variants_keep_raw_flat_namespaced_and_grouping_output() {
    for axios in [false, true] {
        for raw in [false, true] {
            for flat in [false, true] {
                for group in [false, true] {
                    let mut old = ProfileSet::new("sdk");
                    old = if axios {
                        old.typescript_axios()
                    } else {
                        old.typescript_fetch()
                    };
                    old = old.typescript_options(ts::TypeScriptOptions {
                        client_name: Some("Acme".into()),
                        client_style: if flat {
                            SdkClientStyle::Flat
                        } else {
                            SdkClientStyle::Namespaced
                        },
                        surface: if raw {
                            ts::SdkSurface::Raw
                        } else {
                            ts::SdkSurface::Client
                        },
                        group_by_tag: group,
                    });
                    let mut sdk = ts::sdk().client_name("Acme").group_by_tag(group);
                    if axios {
                        sdk = sdk.axios();
                    }
                    if raw {
                        sdk = sdk.raw();
                    }
                    if flat {
                        sdk = sdk.flat();
                    }
                    let dir = if axios {
                        "typescript-axios"
                    } else {
                        "typescript-fetch"
                    };
                    let new = ProfileSet::new("sdk").package(ts::package(dir).with(sdk));
                    let api = support::sdk_contract_api();
                    assert_eq!(
                        kaji::generate(&api, old).unwrap(),
                        kaji::generate(&api, new).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn shared_package_and_local_settings_resolve_per_instance() {
    let tree = kaji::generate(
        &support::sdk_contract_api(),
        ProfileSet::new("sdk")
            .common(
                Common::default()
                    .client_name("Shared")
                    .client_style(SdkClientStyle::Flat),
            )
            .package(ts::package("a").with(ts::sdk()))
            .package(
                ts::package("b")
                    .common(Common::default().client_name("Package"))
                    .with(ts::sdk()),
            )
            .package(
                ts::package("c")
                    .common(Common::default().client_name("Package"))
                    .with(ts::sdk().client_name("Local").namespaced()),
            ),
    )
    .unwrap();
    assert!(
        tree.get("sdk/a/client.ts")
            .unwrap()
            .contains("class Shared")
    );
    assert!(
        tree.get("sdk/b/client.ts")
            .unwrap()
            .contains("class Package")
    );
    assert!(tree.get("sdk/c/client.ts").unwrap().contains("class Local"));
    assert!(
        tree.get("sdk/a/STYLE_GUIDE.md")
            .unwrap()
            .contains("flat instantiated client")
    );
    assert!(
        tree.get("sdk/c/STYLE_GUIDE.md")
            .unwrap()
            .contains("namespaced instantiated client")
    );
}
