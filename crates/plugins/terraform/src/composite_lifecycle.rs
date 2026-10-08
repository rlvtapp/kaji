use super::*;
use crate::plan::{AttributePlan, IdentityBinding};
fn fixture() -> EntityCatalog {
    let mut catalog = super::native_lifecycle::fixture();
    catalog.authentication = AuthenticationPlan::None;
    let plan = &mut catalog.resources[0];
    plan.requires_auth = false;
    plan.identity = vec![
        IdentityBinding {
            parameter: "org".into(),
            field: "organizationId".into(),
        },
        IdentityBinding {
            parameter: "thingId".into(),
            field: "id".into(),
        },
    ];
    plan.create.path = "/organizations/{org}/things".into();
    plan.create.parameters = vec![poolster_core::OperationParameter {
        name: "org".into(),
        location: "path".into(),
        required: true,
        schema: Some(poolster_core::SchemaValue::new(
            poolster_core::SchemaKind::String,
        )),
        description: None,
        annotations: Default::default(),
    }];
    plan.read.path = "/organizations/{org}/things/{thingId}".into();
    plan.delete.path = plan.read.path.clone();
    plan.update.as_mut().unwrap().path = plan.read.path.clone();
    plan.attributes = vec![AttributePlan {
        name: "organization_id".into(),
        wire_name: "organizationId".into(),
        ty: ScalarType::String,
        shape: None,
        required: true,
        optional: false,
        computed: false,
        sensitive: false,
        replace_on_change: true,
        update_input: false,
        update_required: false,
        response_required: true,
    }];
    catalog
}
#[test]
fn composite_sources_substitute_every_path_and_configured_parent() {
    let tree = render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap();
    let source = tree.get("internal/provider/resource_thing.go").unwrap();
    assert!(source.contains("createRoute=strings.Replace"));
    assert!(source.contains("ThingResourceParseIdentity"));
    assert!(!source.contains("route:=strings.Replace"));
    assert!(!source.contains("readRoute:=strings.Replace"));
    assert!(!source.contains("body[\"organizationId\"]=data.OrganizationId"));
}
#[test]
#[ignore = "requires Go and cached Framework modules"]
fn generated_composite_provider_executes_native_framework_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = fixture();
    let mut tree = render(&Api::default(), &catalog, "example.com/provider", "example").unwrap();
    add_data_sources(&mut tree, &catalog, "example").unwrap();
    tree.insert(
        GeneratedFile::new(
            "internal/provider/composite_lifecycle_test.go",
            include_str!("composite_lifecycle_test.go.txt"),
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(directory.path()).unwrap();
    let mut command = std::process::Command::new("go");
    command
        .args(["test", "-mod=readonly", "./..."])
        .current_dir(directory.path())
        .env("GOCACHE", "/tmp/kaji-tf-go-cache")
        .env(
            "GOMODCACHE",
            std::env::var("KAJI_TERRAFORM_GOMODCACHE")
                .unwrap_or_else(|_| "/tmp/kaji-tf-mod-cache".into()),
        );
    if std::env::var_os("KAJI_TERRAFORM_OFFLINE").is_some() {
        command.env("GOPROXY", "off");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
