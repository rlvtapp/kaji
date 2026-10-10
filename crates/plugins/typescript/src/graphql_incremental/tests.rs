use super::*;
use poolster_core::engine::Packages;
struct Source {
    meta: Meta,
    value: GraphqlIncrementalOperations,
}
impl Plugin<TypeScript> for Source {
    fn kind(&self) -> &'static str {
        "incremental-test-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlIncrementalOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        cx.publish(self.value.clone())
    }
}
fn generate() -> Result<poolster_core::GeneratedTree> {
    let sdl = "directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT\ndirective @stream(if:Boolean! = true,label:String,initialCount:Int! = 0) on FIELD\ntype Query { users:[User!]! } type User { id:ID! name:String nickname:String }";
    let schema = poolster_input_graphql::parse(sdl)?;
    let value = poolster_input_graphql::lower_incremental_operations(
        &schema.schema,
        sdl,
        "query Users { users @stream(initialCount:1,label:\"list\") { id ... @defer(label:\"extra\") { name nickname } } }",
    )?;
    let source = Source {
        meta: Meta::new(),
        value,
    };
    let output = graphql_incremental(Some(source.meta.handle()));
    Packages::new()
        .package(crate::package("sdk").with(source).with(output))
        .generate_native()
}
#[test]
fn native_incremental_generation_and_regeneration() {
    let first = generate().unwrap();
    let second = generate().unwrap();
    assert_eq!(
        first.get("sdk/graphql-incremental.ts"),
        second.get("sdk/graphql-incremental.ts")
    );
    assert!(
        first
            .get("sdk/graphql-incremental/operations/operation_0.ts")
            .unwrap()
            .contains("AsyncGenerator<IncrementalSnapshot<UsersResult>>")
    );
    assert!(
        first
            .get("sdk/graphql-incremental-runtime.ts")
            .unwrap()
            .contains("deferSpec=20220824")
    );
}
#[test]
#[ignore = "requires pinned TypeScript5.9.3, Node24 and local multipart HTTP server"]
fn generated_incremental_compiles_and_runs() {
    let tree = generate().unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let tsc = std::env::var("POOLSTER_TEST_TSC").unwrap_or("tsc".into());
    let compiled = std::process::Command::new(tsc)
        .args(["-p", "tsconfig.json"])
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{} {}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run =
        std::process::Command::new(std::env::var("POOLSTER_TEST_NODE").unwrap_or("node".into()))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/graphql_incremental/runtime-test.cjs"
            ))
            .arg(dir.path().join("sdk/dist/graphql-incremental.js"))
            .output()
            .unwrap();
    assert!(
        run.status.success(),
        "{} {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
