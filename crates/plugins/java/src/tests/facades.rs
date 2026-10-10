use super::*;

#[test]
fn uses_safe_output_paths_and_deterministic_defaults() {
    assert!(render_test_sdk(&contact_api(), "../escape", None).is_err());
    let tree = render_test_sdk(&contact_api(), "java", None).unwrap();
    assert!(
        tree.get("java/src/main/java/io/poolster/poolsteremailapi/Client.java")
            .is_some()
    );
    assert!(
        tree.get("java/pom.xml")
            .unwrap()
            .contains("<version>2026.09.19</version>")
    );
}

#[test]
fn generates_void_operations_without_decode() {
    let mut api = contact_api();
    api.operations[0].id = "deleteContact".into();
    api.operations[0].responses.clear();
    api.operations[0].request_body = None;
    let tree = render_test_sdk(&api, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("public void deleteContact(DeleteContactRequest input)"));
    assert!(!client.contains("return decode(response, Void.class)"));
}

#[test]
fn emits_binary_upload_download_and_sse_surfaces() {
    let mut source = contact_api();
    source.operations[0].id = "downloadContact".into();
    source.operations[0].request_body = Some(OperationRequestBody {
        required: true,
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/octet-stream".into(),
            schema: Some(string()),
        }],
    });
    source.operations[0].responses = vec![OperationResponse {
        status: "200".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/pdf".into(),
            schema: None,
        }],
    }];
    let mut stream = source.operations[0].clone();
    stream.id = "watchContacts".into();
    stream.request_body = None;
    stream.responses[0].media_types[0].content_type = "text/event-stream".into();
    source.operations.push(stream);
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("public byte[] downloadContact(DownloadContactRequest input)"));
    assert!(client.contains("byte[] body"));
    assert!(client.contains("requestBinary"));
    assert!(client.contains("public java.util.stream.Stream<String> watchContacts"));
    assert!(client.contains("requestEventStream"));
    source.operations[0]
        .request_body
        .as_mut()
        .unwrap()
        .media_types[0]
        .content_type = "multipart/form-data".into();
    let multipart = render_test_sdk(&source, "java", None).unwrap();
    assert!(
        multipart
            .iter()
            .any(|(_, source)| source.contains("java.util.List<OrderedMultipart.Part> parts"))
    );
}

#[test]
fn emits_operation_status_errors_with_declared_bodies() {
    let mut source = contact_api();
    source.operations[0].responses.push(OperationResponse {
        status: "404".into(),
        description: None,
        media_types: vec![OperationMediaType {
            content_type: "application/json".into(),
            schema: Some(SchemaValue::reference("#/components/schemas/Contact")),
        }],
    });
    let tree = render_test_sdk(&source, "java", None).unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("class GetContactStatus404Exception extends ApiException"));
    assert!(client.contains("mapGetContactError(error)"));
    assert!(client.contains("decodeDeclaredError(error.responseBody(), Contact.class)"));
}

#[test]
fn namespaced_style_exports_resource_facades_and_a_style_guide() {
    let tree = render_sdk(
        &contact_api(),
        "java",
        Some("com.poolster.email"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_java(&tree);
    let resource = tree
        .get("java/src/main/java/com/poolster/email/ContactsResource.java")
        .unwrap();
    assert!(
        client
            .split_whitespace()
            .collect::<String>()
            .contains("publicContactsResourcecontacts(){returncontactsResource;}")
    );
    assert!(resource.contains("extends ContactsResourcePart000"));
    assert!(client.contains("public Contact get(Client.GetContactRequest input)"));
    assert!(client.contains("return client.getContact(input);"));
    assert!(
        tree.get("java/STYLE_GUIDE.md")
            .unwrap()
            .contains("client.contacts().get(input)")
    );
}

#[test]
fn partitions_large_resource_facades_without_changing_the_namespace() {
    let mut source = contact_api();
    let template = source.operations[0].clone();
    source.operations = (0..101)
        .map(|index| {
            let mut operation = template.clone();
            operation.id = format!("getContact{index}");
            operation
        })
        .collect();
    let tree = render_sdk(
        &source,
        "java",
        Some("com.poolster.email"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    assert!(tree.iter().any(|(path, _)| {
        path.to_string_lossy()
            .ends_with("ContactsResourcePart001.java")
    }));
    assert!(
        tree.get("java/src/main/java/com/poolster/email/ContactsResource.java")
            .unwrap()
            .contains("extends ContactsResourcePart001")
    );
}

#[test]
fn partitions_verbose_operations_at_sixty_four_methods() {
    let mut source = contact_api();
    let template = source.operations[0].clone();
    source.operations = (0..65)
        .map(|index| {
            let mut operation = template.clone();
            operation.id = format!("getContact{index}");
            operation
        })
        .collect();
    let tree = render_test_sdk(&source, "java", Some("com.poolster.email")).unwrap();
    assert!(
        tree.get("java/src/main/java/com/poolster/email/internal/Operations001.java")
            .is_some()
    );
    assert!(
        tree.get("java/src/main/java/com/poolster/email/Client.java")
            .unwrap()
            .contains("extends com.poolster.email.internal.Operations001")
    );
}

#[test]
fn namespaced_accessor_avoids_a_direct_auth_check_operation_collision() {
    let mut source = contact_api();
    let mut auth_check = source.operations[0].clone();
    auth_check.id = "authCheck".into();
    auth_check.path = "/v1/auth-check".into();
    source.operations.push(auth_check);
    let tree = render_sdk(
        &source,
        "java",
        Some("com.poolster.email"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let client = rendered_java(&tree);
    assert!(client.contains("public ContactsResource contacts()"));
    assert!(client.contains("public AuthCheckResource authCheckResource()"));
    assert!(client.contains("public Contact authCheck(AuthCheckRequest input)"));
    assert!(!client.contains("public AuthCheckResource authCheck()"));
}
