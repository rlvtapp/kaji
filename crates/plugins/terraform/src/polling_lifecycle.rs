//! Native Plugin Framework lifecycle waiters against an in-memory HTTP driver.
use super::*;
use crate::plan::{IdentityBinding, LifecyclePollingBinding, PollCriterion, PollingBinding};

fn fixture() -> EntityCatalog {
    let mut catalog = super::native_lifecycle::fixture();
    let plan = &mut catalog.resources[0];
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
    plan.create.parameters = vec![kaji_core::OperationParameter {
        name: "org".into(),
        location: "path".into(),
        required: true,
        schema: Some(kaji_core::SchemaValue::new(kaji_core::SchemaKind::String)),
        description: None,
        annotations: Default::default(),
    }];
    plan.read.path = "/organizations/{org}/things/{thingId}".into();
    plan.delete.path = plan.read.path.clone();
    plan.update.as_mut().unwrap().path = plan.read.path.clone();
    plan.attributes
        .retain(|attribute| attribute.name == "name" || attribute.name == "status");
    let mut parent = plan.attributes[0].clone();
    parent.name = "organization_id".into();
    parent.wire_name = "organizationId".into();
    parent.replace_on_change = true;
    parent.update_input = false;
    parent.update_required = false;
    plan.attributes.push(parent);
    let waiter = |success| PollingBinding {
        delay_ms: 0,
        interval_ms: 1,
        max_attempts: 3,
        timeout_ms: 100,
        success,
        failure: vec![PollCriterion::Body {
            pointer: "/failed".into(),
            equals: serde_json::json!(true),
        }],
    };
    let ready = vec![
        PollCriterion::Status { status: 200 },
        PollCriterion::Body {
            pointer: "/status".into(),
            equals: serde_json::json!("ready"),
        },
    ];
    plan.polling = Some(LifecyclePollingBinding {
        create: Some(waiter(ready.clone())),
        update: Some(waiter(ready)),
        delete: Some(waiter(vec![PollCriterion::Status { status: 404 }])),
    });
    catalog
}

#[test]
#[ignore = "requires Go and cached Framework modules; bounded local polling only"]
fn generated_polling_provider_executes_native_framework_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let mut tree = render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap();
    tree.insert(
        GeneratedFile::new(
            "internal/provider/polling_lifecycle_test.go",
            include_str!("polling_lifecycle_test.go.txt"),
        )
        .unwrap(),
    )
    .unwrap();
    tree.write_to(directory.path()).unwrap();
    let mut command = std::process::Command::new("go");
    command
        .args(["test", "-mod=readonly", "./..."])
        .current_dir(directory.path())
        .env("GOCACHE", std::env::temp_dir().join("kaji-tf-go-cache"))
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
#[ignore = "requires Terraform CLI, Go and cached Framework; ephemeral local async API only"]
fn terraform_cli_polling_local_mock_lifecycle() {
    use std::{fs, process::Command, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("bin");
    fs::create_dir(&binary).unwrap();
    let mut catalog = fixture();
    let polling = catalog.resources[0].polling.as_mut().unwrap();
    for binding in [
        &mut polling.create,
        &mut polling.update,
        &mut polling.delete,
    ] {
        binding.as_mut().unwrap().timeout_ms = 2000;
    }
    render(&Api::default(), &catalog, "example.com/provider", "example")
        .unwrap()
        .write_to(root.path())
        .unwrap();
    let output = Command::new("go")
        .args(["build", "-mod=readonly", "-o"])
        .arg(binary.join("terraform-provider-example"))
        .arg(".")
        .current_dir(root.path())
        .env("GOCACHE", std::env::temp_dir().join("kaji-tf-go-cache"))
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
    fs::write(
        root.path().join("mock.py"),
        r#"import http.server,json,sys
state={'id':'child/id','organizationId':'a/b','name':'planned','status':'ready'}
pending=0
deleting=False
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def respond(self,status,body=None):
  self.send_response(status);self.send_header('Content-Type','application/json');self.end_headers()
  if body is not None:self.wfile.write(json.dumps(body).encode())
 def do_GET(self):
  global pending
  if self.path!='/organizations/a%2Fb/things/child%2Fid':return self.respond(404)
  pending=max(0,pending-1)
  if deleting and pending==0:return self.respond(404)
  state['status']='pending' if pending else 'ready'
  self.respond(200,state)
 def mutation(self):
  global pending
  with open(sys.argv[2],'a') as log:log.write(self.command+'\n')
  if self.command!='DELETE':
   payload=json.loads(self.rfile.read(int(self.headers.get('Content-Length','0'))))
   assert 'organizationId' not in payload
   state.update(payload)
  pending=2;state['status']='pending';self.respond(202,state)
 def do_POST(self):
  assert self.path=='/organizations/a%2Fb/things'
  self.mutation()
 def do_PATCH(self):self.mutation()
 def do_DELETE(self):
  global deleting
  deleting=True;self.mutation()
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
open(sys.argv[1],'w').write(str(server.server_port))
server.serve_forever()
"#,
    )
    .unwrap();
    let mut child =
        Command::new(std::env::var("KAJI_TEST_PYTHON").unwrap_or_else(|_| "python3".into()))
            .arg(root.path().join("mock.py"))
            .arg(root.path().join("port"))
            .arg(root.path().join("mutations"))
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
    fs::write(root.path().join("terraform.rc"), format!("provider_installation {{ dev_overrides {{ \"registry.terraform.io/kaji/example\" = {:?} }} }}", binary.to_string_lossy())).unwrap();
    let config = |name: &str| {
        format!(
            "terraform {{\n required_providers {{\n example = {{ source = \"kaji/example\" }}\n }}\n }}\nprovider \"example\" {{\n base_url = \"http://127.0.0.1:{port}\"\n auth_token = \"private-token\"\n }}\nresource \"example_thing\" \"test\" {{\n organization_id = \"a/b\"\n name = {name:?}\n }}\n"
        )
    };
    fs::write(root.path().join("main.tf"), config("planned")).unwrap();
    let terraform = std::env::var("KAJI_TERRAFORM_BIN").unwrap_or_else(|_| "terraform".into());
    let run = |args: &[&str]| {
        let output = Command::new(&terraform)
            .args(args)
            .current_dir(root.path())
            .env("TF_CLI_CONFIG_FILE", root.path().join("terraform.rc"))
            .env("TF_IN_AUTOMATION", "1")
            .env("CHECKPOINT_DISABLE", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&["validate", "-no-color"]);
    run(&["apply", "-auto-approve", "-input=false", "-no-color"]);
    run(&["plan", "-detailed-exitcode", "-input=false", "-no-color"]);
    fs::write(root.path().join("main.tf"), config("updated")).unwrap();
    run(&["apply", "-auto-approve", "-input=false", "-no-color"]);
    run(&["plan", "-detailed-exitcode", "-input=false", "-no-color"]);
    run(&["destroy", "-auto-approve", "-input=false", "-no-color"]);
    assert_eq!(
        fs::read_to_string(root.path().join("mutations")).unwrap(),
        "POST\nPATCH\nDELETE\n",
        "polling must never replay mutation"
    );
}
