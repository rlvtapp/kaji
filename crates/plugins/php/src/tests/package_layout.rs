use super::*;

#[test]
fn emits_a_psr18_php_package_with_typed_models_and_operations() {
    let tree = render_test_sdk(&api(), "sdks/php", Some("acme/pet-sdk")).unwrap();
    let composer = tree.get("sdks/php/composer.json").unwrap();
    let model = tree.get("sdks/php/src/Models/Pet.php").unwrap();
    let client = tree.get("sdks/php/src/Client.php").unwrap();
    let operations = tree.get("sdks/php/src/ClientOperations000.php").unwrap();
    assert!(composer.contains("\"php\": \">=8.2\""));
    assert!(composer.contains("\"Acme\\\\Pet\\\\Sdk\\\\\": \"src/\""));
    assert!(model.contains("public readonly int $id"));
    assert!(model.contains("public readonly ?string $displayName = null"));
    assert!(operations.contains("public function getPet(int $id, ?string $include = null): Pet"));
    assert!(operations.contains("rawurlencode((string) $id)"));
    assert!(client.contains("private readonly ?string $apiKey"));
}

#[test]
#[ignore = "requires PHP 8.2 or newer"]
fn generated_package_passes_php_syntax_checks() {
    use std::process::Command;

    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &api(),
        "sdk",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    tree.write_to(root.path()).unwrap();
    for (path, _) in tree
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|extension| extension == "php"))
    {
        let output = Command::new("php")
            .args(["-l"])
            .arg(root.path().join(path))
            .output()
            .expect("PHP must be available when this test is selected");
        assert!(
            output.status.success(),
            "generated PHP file {} failed syntax validation:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn output_is_deterministic_and_uses_a_safe_default_composer_name() {
    let first = render_test_sdk(&api(), "php", None).unwrap();
    let second = render_test_sdk(&api(), "php", None).unwrap();
    assert_eq!(first, second);
    assert!(
        first
            .get("php/composer.json")
            .unwrap()
            .contains("\"name\": \"poolster/pet-store-api-sdk\"")
    );
}

#[test]
fn namespaced_style_exports_typed_resource_facades_and_a_style_guide() {
    let tree = render_sdk(
        &api(),
        "php",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let resource = tree
        .get("php/src/Resources/PetsResourceOperations000.php")
        .unwrap();
    assert!(client.contains("function pets(): \\Acme\\Pet\\Sdk\\Resources\\PetsResource"));
    assert!(resource.contains("public function get(int $id, ?string $include = null): Pet"));
    assert!(resource.contains("return $this->client->getPet($id, $include);"));
    assert!(
        tree.get("php/STYLE_GUIDE.md")
            .unwrap()
            .contains("$client->contacts()->get($id)")
    );
}

#[test]
fn splits_large_resource_facades_into_composed_traits() {
    let mut source = api();
    source.operations = (0..51)
        .map(|index| Operation {
            id: format!("listPets{index}"),
            method: HttpMethod::Get,
            path: format!("/pets/{index}"),
            ..Operation::default()
        })
        .collect();
    let tree = render_sdk(
        &source,
        "php",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let facade = tree.get("php/src/Resources/PetsResource.php").unwrap();
    assert!(facade.contains("use PetsResourceOperations000;"));
    assert!(facade.contains("use PetsResourceOperations002;"));
    assert!(
        tree.get("php/src/Resources/PetsResourceOperations002.php")
            .unwrap()
            .contains("trait PetsResourceOperations002")
    );
}

#[test]
fn namespaced_accessor_avoids_a_direct_auth_check_operation_collision() {
    let mut source = api();
    let mut auth_check = source.operations[0].clone();
    auth_check.id = "authCheck".into();
    auth_check.path = "/v1/auth-check".into();
    source.operations.push(auth_check);
    let tree = render_sdk(
        &source,
        "php",
        Some("acme/pet-sdk"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(client.contains("public function pets():"));
    assert!(client.contains("public function authCheckResource():"));
    assert!(operations.contains("public function authCheck("));
    assert!(!operations.contains("public function authCheck():"));
}

#[test]
fn emits_retry_hooks_idempotency_safety_and_binary_downloads() {
    let mut source = api();
    source.operations[0].method = HttpMethod::Post;
    source.operations[0].parameters.push(OperationParameter {
        name: "Idempotency-Key".into(),
        location: "header".into(),
        required: true,
        schema: Some(SchemaValue::new(SchemaKind::String)),
        description: None,
        annotations: BTreeMap::new(),
    });
    source.operations[0].responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/octet-stream".into(),
            schema: None,
        }],
    }];
    let tree = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(client.contains("private readonly int $maxRetries = 2"));
    assert!(client.contains("private readonly mixed $beforeRequest = null"));
    assert!(client.contains("[408, 429, 500, 502, 503, 504]"));
    assert!(client.contains("strtolower($name) === 'idempotency-key' || ($idempotencyHeader !== null && strcasecmp($name, $idempotencyHeader) === 0)"));
    assert!(operations.contains(
        "public function getPet(int $id, string $idempotencyKey, ?string $include = null): string"
    ));
    assert!(operations.contains("return $contents;"));
}

#[test]
fn emits_an_honest_psr7_sse_stream_surface() {
    let mut source = api();
    source.operations[0].id = "watchPets".into();
    source.operations[0].path = "/pets/events".into();
    source.operations[0].parameters.clear();
    source.operations[0].responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "text/event-stream".into(),
            schema: None,
        }],
    }];
    let tree = render_test_sdk(&source, "php", Some("acme/pet-sdk")).unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(operations.contains("use Psr\\Http\\Message\\StreamInterface;"));
    assert!(operations.contains("public function watchPets(): StreamInterface"));
    assert!(
        operations.contains("return $this->eventStream('GET', $path, [], [], null, \"json\");")
    );
    assert!(client.contains("private function eventStream("));
    assert!(client.contains("PSR-18 only promises a PSR-7 response, not a live socket"));
    assert!(client.contains("['Accept' => 'text/event-stream']"));
}
