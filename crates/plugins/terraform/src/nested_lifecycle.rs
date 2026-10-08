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

#[test]
#[ignore = "requires Terraform CLI (KAJI_TERRAFORM_BIN), Go and official Framework cache"]
fn terraform_cli_nested_local_mock_lifecycle() {
    use std::{fs, process::Command, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("bin");
    fs::create_dir(&binary).unwrap();
    let mut tree = render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap();
    add_data_sources(&mut tree, &fixture(), "example").unwrap();
    tree.write_to(root.path()).unwrap();
    let output = Command::new("go")
        .args(["build", "-mod=readonly", "-o"])
        .arg(binary.join("terraform-provider-example"))
        .arg(".")
        .current_dir(root.path())
        .env("GOCACHE", "/tmp/kaji-tf-go-cache")
        .env(
            "GOMODCACHE",
            std::env::var("KAJI_TERRAFORM_GOMODCACHE")
                .unwrap_or_else(|_| "/tmp/kaji-tf-mod-cache".into()),
        )
        .env("GOPROXY", "off")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let server_source = r#"import http.server,json,sys,pathlib
state={'id':'a/b','config':{'displayName':'planned','enabled':True},'rules':[{'displayName':'rule','enabled':False}],'labels':{'primary':{'displayName':'label','enabled':True}},'tags':['a','b'],'properties':{'color':'blue'}}
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def respond(self,status,body=None):
  self.send_response(status);self.send_header('Content-Type','application/json');self.end_headers()
  if body is not None:self.wfile.write(json.dumps(body).encode())
 def do_GET(self):
  mode=pathlib.Path(sys.argv[2]).read_text()
  if mode=='denied':return self.respond(401)
  if mode=='missing':return self.respond(404)
  if mode=='drift':state['config']['displayName']='drifted'
  if self.path!='/things/a%2Fb':return self.respond(404)
  self.respond(200,state)
 def do_POST(self):
  state.update(json.loads(self.rfile.read(int(self.headers.get('Content-Length','0')))));self.respond(201,state)
 def do_PATCH(self):
  if pathlib.Path(sys.argv[2]).read_text()=='update-failure':return self.respond(500)
  state.update(json.loads(self.rfile.read(int(self.headers.get('Content-Length','0')))));self.respond(204)
 def do_DELETE(self):self.respond(204)
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
open(sys.argv[1],'w').write(str(server.server_port))
server.serve_forever()
"#;
    fs::write(root.path().join("mock.py"), server_source).unwrap();
    let python = std::env::var("KAJI_TEST_PYTHON").unwrap_or_else(|_| "python3".into());
    fs::write(root.path().join("mode"), "normal").unwrap();
    let mut child = Command::new(python)
        .arg(root.path().join("mock.py"))
        .arg(root.path().join("port"))
        .arg(root.path().join("mode"))
        .spawn()
        .unwrap();
    struct Cleanup<'a>(&'a mut std::process::Child);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _cleanup = Cleanup(&mut child);
    for _ in 0..100 {
        if root.path().join("port").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let port = fs::read_to_string(root.path().join("port")).unwrap();
    fs::write(root.path().join("terraform.rc"),format!("provider_installation {{ dev_overrides {{ \"registry.terraform.io/kaji/example\" = {:?} }} }}",binary.to_string_lossy())).unwrap();
    let config = |name: &str| {
        format!(
            "terraform {{\n required_providers {{\n example = {{ source = \"kaji/example\" }}\n }}\n }}\nprovider \"example\" {{\n base_url = \"http://127.0.0.1:{port}\"\n auth_token = \"secret\"\n }}\nresource \"example_thing\" \"test\" {{\n config = {{ display_name = {name:?}, enabled = true }}\n rules = [{{ display_name = \"rule\", enabled = false }}]\n labels = {{ primary = {{ display_name = \"label\", enabled = true }} }}\n tags = [\"a\", \"b\"]\n properties = {{ color = \"blue\" }}\n }}\ndata \"example_thing\" \"existing\" {{ id = example_thing.test.id }}\noutput \"count\" {{ value = data.example_thing.existing.tags }}"
        )
    };
    fs::write(root.path().join("main.tf"), config("planned")).unwrap();
    let terraform = std::env::var("KAJI_TERRAFORM_BIN").unwrap_or_else(|_| "terraform".into());
    let run_expected = |args: &[&str], expected: i32| {
        let output = Command::new(&terraform)
            .args(args)
            .current_dir(root.path())
            .env("TF_CLI_CONFIG_FILE", root.path().join("terraform.rc"))
            .env("TF_IN_AUTOMATION", "1")
            .env("CHECKPOINT_DISABLE", "1")
            .output()
            .unwrap();
        assert!(
            output.status.code() == Some(expected),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    let run = |args: &[&str]| run_expected(args, 0);
    run(&["validate", "-no-color"]);
    run(&["apply", "-auto-approve", "-input=false", "-no-color"]);
    run(&["plan", "-detailed-exitcode", "-input=false", "-no-color"]);
    // A real refresh must detect remote drift while denied reads keep managed state.
    fs::write(root.path().join("mode"), "drift").unwrap();
    run_expected(
        &["plan", "-detailed-exitcode", "-input=false", "-no-color"],
        2,
    );
    fs::write(root.path().join("mode"), "denied").unwrap();
    run_expected(&["plan", "-input=false", "-no-color"], 1);
    assert!(
        String::from_utf8_lossy(&run(&["state", "list"]).stdout).contains("example_thing.test")
    );
    fs::write(root.path().join("mode"), "normal").unwrap();
    fs::write(root.path().join("main.tf"), config("updated")).unwrap();
    fs::write(root.path().join("mode"), "update-failure").unwrap();
    run_expected(&["apply", "-auto-approve", "-input=false", "-no-color"], 1);
    assert!(
        String::from_utf8_lossy(&run(&["state", "list"]).stdout).contains("example_thing.test")
    );
    fs::write(root.path().join("mode"), "normal").unwrap();
    run(&["apply", "-auto-approve", "-input=false", "-no-color"]);
    let state: serde_json::Value = serde_json::from_slice(&run(&["show", "-json"]).stdout).unwrap();
    let managed = state["values"]["root_module"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["address"] == "example_thing.test")
        .unwrap();
    assert_eq!(managed["values"]["config"]["display_name"], "updated");
    assert_eq!(managed["values"]["rules"][0]["enabled"], false);
    assert_eq!(
        managed["values"]["labels"]["primary"]["display_name"],
        "label"
    );
    assert_eq!(managed["values"]["tags"], serde_json::json!(["a", "b"]));
    assert_eq!(managed["values"]["properties"]["color"], "blue");
    run(&["state", "rm", "example_thing.test"]);
    run(&["import", "-input=false", "example_thing.test", "a/b"]);
    run(&["plan", "-detailed-exitcode", "-input=false", "-no-color"]);
    run(&["destroy", "-auto-approve", "-input=false", "-no-color"]);
}
