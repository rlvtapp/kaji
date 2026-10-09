use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::GraphqlOperations,
};
use poolster_plugin_typescript::{self as ts, PackageExt};
use std::collections::BTreeMap;
struct ManifestProvider {
    meta: Meta,
}
impl ManifestProvider {
    fn new() -> Self {
        Self { meta: Meta::new() }
    }
}
impl Plugin<ts::TypeScript> for ManifestProvider {
    fn kind(&self) -> &'static str {
        "community-npm-manifest"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> Result<()> {
        cx.workspace.package_file(GeneratedFile::new(
            "package.json",
            "{\"name\":\"community-helper-test\",\"version\":\"1.0.0\",\"type\":\"module\"}",
        )?)
    }
}
struct CommunityClient {
    meta: Meta,
    broken: bool,
}
impl CommunityClient {
    fn new(broken: bool) -> Self {
        Self {
            meta: Meta::new(),
            broken,
        }
    }
    fn handle(&self) -> Handle<ts::GraphqlClient> {
        self.meta.handle()
    }
}
impl Plugin<ts::TypeScript> for CommunityClient {
    fn kind(&self) -> &'static str {
        "community-graphql-test-client"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<ts::GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> Result<()> {
        let schema = "type User {id:ID! nickname:String} type Query{user(id:ID!):User!}";
        let doc = poolster_input_graphql::parse(schema)?;
        let definition: GraphqlOperations = poolster_input_graphql::lower_operations(
            &doc.schema,
            schema,
            "query ReadUser($id:ID!){user(id:$id){id nickname}}",
        )?;
        cx.files.emit(GeneratedFile::new("community/types.ts","export type ProvidedVariables={id:string}; export type ProvidedResult={user:{id:string;nickname:string|null}};" )?)?;
        let symbol = |name: &str| ts::Symbol {
            module: "community/types".into(),
            name: name.into(),
        };
        let operations = if self.broken {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                "ReadUser".into(),
                ts::GraphqlOperationSymbols {
                    function: symbol("providedLegacy"),
                    raw_function: symbol("providedRead"),
                    variables: symbol("ProvidedVariables"),
                    result: symbol("ProvidedResult"),
                    kind: poolster_core::native::GraphqlOperationKind::Query,
                },
            )])
        };
        cx.publish(ts::GraphqlClient {
            operations,
            runtime_module: "community/runtime".into(),
            style: ts::GraphqlStyle::Flat,
            factory: None,
            methods: BTreeMap::from([("ReadUser".into(), "readUser".into())]),
            definition,
            scalars: BTreeMap::new(),
        })
    }
}
#[test]
fn helper_uses_community_symbols_and_owned_definition() -> Result<()> {
    let provider = CommunityClient::new(false);
    let handle = provider.handle();
    let tree = Packages::new()
        .package(
            ts::package("sdk")
                .name("community-helper-test")
                .with(ts::faker().using_graphql(Some(handle)))
                .with(ts::zod().using_graphql(Some(handle)))
                .with(provider)
                .with(ManifestProvider::new()),
        )
        .generate_native()?;
    let schema = tree.get("sdk/zod.ts").unwrap();
    ensure!(schema.contains("import type { ProvidedResult } from \"./community/types.js\""));
    ensure!(schema.contains("z.ZodType<ProvidedResult>"));
    ensure!(
        tree.get("sdk/faker.ts")
            .unwrap()
            .contains("fakeReadUserResult(): ProvidedResult")
    );
    Ok(())
}
#[test]
fn incomplete_community_operation_metadata_is_rejected() -> Result<()> {
    let provider = CommunityClient::new(true);
    let handle = provider.handle();
    let error = Packages::new()
        .package(
            ts::package("sdk")
                .name("community-helper-test")
                .with(ts::zod().using_graphql(Some(handle)))
                .with(provider)
                .with(ManifestProvider::new()),
        )
        .generate_native()
        .expect_err("missing symbols must fail before emission");
    ensure!(format!("{error:#}").contains("omitted operation symbols"));
    Ok(())
}

#[test]
fn conflicting_community_package_assembly_is_rejected() -> Result<()> {
    struct Conflict {
        meta: Meta,
    }
    impl Plugin<ts::TypeScript> for Conflict {
        fn kind(&self) -> &'static str {
            "conflicting-manifest"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> Result<()> {
            cx.workspace
                .package_file(GeneratedFile::new("package.json", "{}")?)
        }
    }
    let error = Packages::new()
        .package(
            ts::package("sdk")
                .with(ManifestProvider::new())
                .with(Conflict { meta: Meta::new() }),
        )
        .generate_native()
        .expect_err("package ownership conflict must fail");
    ensure!(format!("{error:#}").contains("multiple plugins own package.json"));
    Ok(())
}
