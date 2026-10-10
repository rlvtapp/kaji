use super::*;
fn fixture() -> GraphqlOperations {
    let document = poolster_input_graphql::parse("scalar DateTime\ntype User { id: ID!, name: String!, when: DateTime! }\ntype Query { user(id: ID!): User! }\ntype Mutation { rename(id: ID!, name: String!): User! }").unwrap();
    poolster_input_graphql::lower_operations(&document.schema, "", "query ReadUser($id: ID!) { user(id: $id) { id name when } }\nmutation Rename($id: ID!, $name: String!) { rename(id: $id, name: $name) { id name when } }").unwrap()
}
#[test]
fn bundle_reuses_portable_models_and_configures_graphql_constructor() {
    for style in [
        GraphqlStyle::Raw,
        GraphqlStyle::Flat,
        GraphqlStyle::Idiomatic,
    ] {
        let tree = render(&fixture(), "acme/graphql", style, &BTreeMap::new()).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(tree.get("composer.json").unwrap()).unwrap();
        assert_eq!(manifest["require"]["symfony/http-client"], "^6.4 || ^7.0");
        assert_eq!(
            manifest["autoload"]["psr-4"]["Acme\\Graphql\\Symfony\\"],
            "src/Symfony/"
        );
        assert!(
            tree.get("src/Symfony/DependencyInjection/AcmeGraphqlExtension.php")
                .unwrap()
                .contains("$config['endpoint'], $config['headers']")
        );
        let portable =
            render_graphql_package(&fixture(), "acme/graphql", style, &BTreeMap::new()).unwrap();
        for (path, contents) in portable.iter() {
            if path.starts_with("src") {
                assert_eq!(tree.get(path), Some(contents));
            }
        }
        assert_eq!(
            tree,
            render(&fixture(), "acme/graphql", style, &BTreeMap::new()).unwrap()
        );
    }
    assert!(
        render(
            &fixture(),
            "../invalid",
            GraphqlStyle::Flat,
            &BTreeMap::new()
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires PHP 8.2 with installed Symfony 6.4 dependencies; POOLSTER_SYMFONY_RUNNER"]
fn symfony_container_and_http_client_execute_generated_package() {
    use std::{
        fs,
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let modules = std::env::var("POOLSTER_GRAPHQL_JS_ROOT").expect("set pinned GraphQL.js root");
    let mut server = Server(
        Command::new("node")
            .args(["-e", include_str!("server.cjs")])
            .env("POOLSTER_GRAPHQL_JS_ROOT", modules)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    let host = std::env::var("POOLSTER_GRAPHQL_SERVER_HOST").unwrap_or("127.0.0.1".into());
    let endpoint = format!("http://{host}:{}", port.trim());
    let root = tempfile::tempdir().unwrap();
    for (name, style) in [
        ("raw", GraphqlStyle::Raw),
        ("flat", GraphqlStyle::Flat),
        ("grouped", GraphqlStyle::Idiomatic),
        ("custom", GraphqlStyle::Idiomatic),
    ] {
        let path = root.path().join(name);
        fs::create_dir(&path).unwrap();
        let groups = if name == "custom" {
            BTreeMap::from([(
                "users".into(),
                BTreeMap::from([
                    ("read".into(), "ReadUser".into()),
                    ("rename".into(), "Rename".into()),
                ]),
            )])
        } else {
            BTreeMap::new()
        };
        let tree = render(&fixture(), "acme/graphql", style, &groups).unwrap();
        for (relative, contents) in tree.iter() {
            let target = path.join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, contents).unwrap();
        }
        fs::write(path.join("probe.php"), include_str!("probe.php")).unwrap();
        let output = Command::new(
            std::env::var("POOLSTER_SYMFONY_RUNNER").expect("set POOLSTER_SYMFONY_RUNNER"),
        )
        .arg(&path)
        .arg(name)
        .env("POOLSTER_SYMFONY_ENDPOINT", &endpoint)
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn selected_provider_and_package_settings_drive_bundle_generation() {
    use crate::PackageExt as _;
    use poolster_core::engine::{Packages, Provision};
    struct Source {
        meta: Meta,
        operation: &'static str,
    }
    impl Plugin<crate::Symfony> for Source {
        fn kind(&self) -> &'static str {
            "test-graphql-input"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<GraphqlOperations>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, crate::Symfony>) -> Result<()> {
            let mut contract = fixture();
            contract.operations.truncate(1);
            contract.operations[0].name = self.operation.into();
            cx.publish(contract)
        }
    }
    for operation in ["ReadUser", "Renamed"] {
        let source = Source {
            meta: Meta::new(),
            operation,
        };
        let ignored = Source {
            meta: Meta::new(),
            operation: "Ignored",
        };
        let generator = graphql(Some(source.meta.handle())).group("users", "read", operation);
        let package = crate::package("bundle")
            .name("acme/graphql")
            .with(generator)
            .with(ignored)
            .with(source);
        let tree = Packages::new().package(package).generate_native().unwrap();
        assert!(
            tree.get(format!("bundle/src/Operations/{operation}.php"))
                .is_some()
        );
        assert!(tree.get("bundle/src/Operations/Ignored.php").is_none());
        assert!(
            tree.get("bundle/src/Symfony/AcmeGraphqlBundle.php")
                .is_some()
        );
    }
    let source = Source {
        meta: Meta::new(),
        operation: "ReadUser",
    };
    let generator = graphql(Some(source.meta.handle()));
    let package = crate::package("bundle")
        .sdk_package("acme/external")
        .with(generator)
        .with(source);
    let error = Packages::new()
        .package(package)
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("sdk_package"));
}
