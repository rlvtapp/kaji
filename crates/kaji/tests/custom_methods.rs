use poolster::{ProfileSet, generate};
use poolster_core::{Api, HttpMethod, Operation};
fn profiles() -> ProfileSet {
    ProfileSet::new("sdk")
        .package(poolster::rust::package("rust").with(poolster::rust::sdk()))
        .package(poolster::ts::package("typescript-fetch").with(poolster::ts::sdk().fetch()))
        .package(poolster::ts::package("typescript-axios").with(poolster::ts::sdk().axios()))
        .package(poolster::go::package("go").with(poolster::go::sdk()))
        .package(poolster::python::package("python").with(poolster::python::sdk()))
        .package(poolster::php::package("php").with(poolster::php::sdk()))
        .package(poolster::ruby::package("ruby").with(poolster::ruby::sdk()))
        .package(poolster::java::package("java").with(poolster::java::sdk()))
        .package(poolster::dotnet::package("dotnet").with(poolster::dotnet::sdk()))
        .package(poolster::elixir::package("elixir").with(poolster::elixir::sdk()))
        .package(poolster::swift::package("swift").with(poolster::swift::sdk()))
}
#[test]
fn all_sdk_targets_emit_validated_custom_tokens() {
    let api = Api {
        name: "Custom".into(),
        operations: vec![
            Operation {
                id: "copyThing".into(),
                path: "/copy".into(),
                method: HttpMethod::Custom("COPY".into()),
                ..Default::default()
            },
            Operation {
                id: "quoteThing".into(),
                path: "/quote".into(),
                method: HttpMethod::Custom("X'CHECK`TEST".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let tree = generate(&api, profiles()).unwrap();
    for target in [
        "rust",
        "typescript-fetch",
        "typescript-axios",
        "go",
        "python",
        "php",
        "ruby",
        "java",
        "dotnet",
        "elixir",
        "swift",
    ] {
        let contents = tree
            .iter()
            .filter(|(path, _)| path.starts_with(format!("sdk/{target}")))
            .map(|(_, source)| source)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(contents.contains("COPY"), "{target} lost COPY");
        assert!(
            contents.contains("X'CHECK`TEST") || contents.contains("X\\'CHECK`TEST"),
            "{target} lost custom punctuation"
        );
    }
}
#[test]
fn invalid_custom_constructor_is_rejected_before_generators() {
    let api = Api {
        operations: vec![Operation {
            method: HttpMethod::Custom("X\"; injected".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(generate(&api, profiles()).is_err());
}
