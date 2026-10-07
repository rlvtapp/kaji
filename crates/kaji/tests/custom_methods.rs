use kaji::{ProfileSet, generate};
use kaji_core::{Api, HttpMethod, Operation};
fn profiles() -> ProfileSet {
    ProfileSet::new("sdk")
        .package(kaji::rust::package("rust").with(kaji::rust::sdk()))
        .package(kaji::ts::package("typescript-fetch").with(kaji::ts::sdk().fetch()))
        .package(kaji::ts::package("typescript-axios").with(kaji::ts::sdk().axios()))
        .package(kaji::go::package("go").with(kaji::go::sdk()))
        .package(kaji::python::package("python").with(kaji::python::sdk()))
        .package(kaji::php::package("php").with(kaji::php::sdk()))
        .package(kaji::ruby::package("ruby").with(kaji::ruby::sdk()))
        .package(kaji::java::package("java").with(kaji::java::sdk()))
        .package(kaji::dotnet::package("dotnet").with(kaji::dotnet::sdk()))
        .package(kaji::elixir::package("elixir").with(kaji::elixir::sdk()))
        .package(kaji::swift::package("swift").with(kaji::swift::sdk()))
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
