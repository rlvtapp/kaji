use super::*;
use crate::plan::{AttributePlan, NestedFieldPlan, ShapePlan};

fn fixture() -> EntityCatalog {
    let scalar = |ty| ShapePlan::Scalar { ty };
    let object = ShapePlan::Object {
        fields: vec![
            NestedFieldPlan {
                name: "display_name".into(),
                wire_name: "displayName".into(),
                required: true,
                shape: scalar(ScalarType::String),
            },
            NestedFieldPlan {
                name: "enabled".into(),
                wire_name: "enabled".into(),
                required: true,
                shape: scalar(ScalarType::Bool),
            },
            NestedFieldPlan {
                name: "note".into(),
                wire_name: "note".into(),
                required: false,
                shape: scalar(ScalarType::String),
            },
        ],
    };
    let mut catalog = super::native_lifecycle::fixture();
    catalog.authentication = AuthenticationPlan::None;
    catalog.resources[0].requires_auth = false;
    catalog.resources[0].schema_version = 1;
    catalog.resources[0].state_upgrades = vec![crate::plan::StateUpgradeBinding {
        version: 0,
        rename_fields: std::collections::BTreeMap::from([("old_config".into(), "config".into())]),
    }];
    catalog.resources[0].attributes = [
        ("config", object.clone()),
        (
            "rules",
            ShapePlan::List {
                element: Box::new(object.clone()),
            },
        ),
        (
            "labels",
            ShapePlan::Map {
                element: Box::new(object),
            },
        ),
        (
            "tags",
            ShapePlan::List {
                element: Box::new(scalar(ScalarType::String)),
            },
        ),
        (
            "properties",
            ShapePlan::Map {
                element: Box::new(scalar(ScalarType::String)),
            },
        ),
    ]
    .into_iter()
    .map(|(name, shape)| AttributePlan {
        name: name.into(),
        wire_name: name.into(),
        ty: ScalarType::String,
        shape: Some(shape),
        required: true,
        optional: false,
        computed: false,
        sensitive: false,
        replace_on_change: false,
        update_input: true,
        update_required: true,
        response_required: true,
    })
    .collect();
    catalog
}

#[test]
fn nested_sources_use_native_framework_types_and_recursive_wire_codecs() {
    let tree = render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap();
    let source = tree.get("internal/provider/resource_thing.go").unwrap();
    for expected in [
        "SingleNestedAttribute",
        "ListNestedAttribute",
        "MapNestedAttribute",
        "types.Object",
        "types.List",
        "types.Map",
        "mergeNested",
        "displayName",
    ] {
        assert!(source.contains(expected), "missing {expected}");
    }
}

#[test]
#[ignore = "requires Go and cached Framework modules"]
fn generated_nested_provider_executes_native_framework_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = fixture();
    let mut tree = render(&Api::default(), &catalog, "example.com/provider", "example").unwrap();
    add_data_sources(&mut tree, &catalog, "example").unwrap();
    tree.insert(
        GeneratedFile::new(
            "internal/provider/nested_lifecycle_test.go",
            include_str!("nested_lifecycle_test.go.txt"),
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
