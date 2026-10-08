use kaji::{ProfileSet, generate, mock};
use kaji_core::Api;

#[test]
fn embedding_works_without_a_language_plugin_dependency() {
    let api = Api {
        name: "Example".into(),
        version: "1.0.0".into(),
        ..Api::default()
    };
    let profiles = ProfileSet::new("generated").package(mock::package("mock").with(mock::server()));
    let files = generate(&api, profiles).unwrap();
    assert!(
        files.get("generated/mock/Dockerfile").is_some(),
        "generated files: {:?}",
        files.iter().map(|(file, _)| file).collect::<Vec<_>>()
    );
}
