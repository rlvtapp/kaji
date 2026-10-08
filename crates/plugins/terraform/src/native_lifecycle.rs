//! Execute the emitted provider through real Plugin Framework plan/state values.
use super::*;
use poolster_core::{HttpMethod, Operation};
use std::{fs, process::Command};

pub(super) fn fixture() -> EntityCatalog {
    let operation = |id: &str, method, path: &str| Operation {
        id: id.into(),
        method,
        path: path.into(),
        ..Default::default()
    };
    let attr = |name: &str, ty, required, optional, computed| crate::plan::AttributePlan {
        name: if name == "count" {
            "quantity".into()
        } else {
            name.into()
        },
        wire_name: name.into(),
        ty,
        shape: None,
        required,
        optional,
        computed,
        sensitive: false,
        replace_on_change: false,
        update_input: required || optional,
        update_required: required,
        response_required: required || name == "status",
    };
    EntityCatalog {
        schema_version: 1,
        diagnostics: vec![],
        authentication: AuthenticationPlan::Bearer,
        resources: vec![ResourcePlan {
            name: "thing".into(),
            create: operation("createThing", HttpMethod::Post, "/things"),
            read: operation("getThing", HttpMethod::Get, "/things/{thingId}"),
            update: Some(operation(
                "updateThing",
                HttpMethod::Patch,
                "/things/{thingId}",
            )),
            delete: operation("deleteThing", HttpMethod::Delete, "/things/{thingId}"),
            id_parameter: "thingId".into(),
            id_field: "id".into(),
            requires_auth: true,
            schema_version: 0,
            state_upgrades: vec![],
            identity: vec![],
            polling: None,
            attributes: vec![
                attr("name", ScalarType::String, true, false, false),
                attr("enabled", ScalarType::Bool, true, false, false),
                attr("count", ScalarType::Int64, false, true, true),
                attr("ratio", ScalarType::Float64, false, true, true),
                attr("status", ScalarType::String, false, false, true),
            ],
        }],
    }
}
#[test]
fn typed_sources_preserve_plans_and_declared_authentication() {
    let source = render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap();
    assert!(
        source
            .get("internal/provider/transport_test.go")
            .unwrap()
            .contains("TestDeclaredCredentialsApplyOnlyToSecuredOperations")
    );
    let documentation = source.get("README.md").unwrap();
    assert!(documentation.contains("resource \"example_thing\" \"example\""));
    assert!(documentation.contains("terraform import example_thing.example"));
    let resource = source.get("internal/provider/resource_thing.go").unwrap();
    assert!(resource.contains("data.Name.Equal(incoming)"));
    assert!(resource.contains("resp.State.RemoveResource(ctx)"));
    assert!(resource.contains("resource.ImportStatePassthroughID"));
    assert!(
        source
            .get("internal/provider/provider.go")
            .unwrap()
            .contains("if secure { request.Header.Set(\"Authorization\", \"Bearer \"+c.token) }")
    );
    for (auth, expected) in [
        (
            AuthenticationPlan::Basic,
            "request.SetBasicAuth(c.username, c.password)",
        ),
        (
            AuthenticationPlan::ApiKey {
                name: "X-Key".into(),
                location: "header".into(),
            },
            "request.Header.Set(\"X-Key\", c.token)",
        ),
        (
            AuthenticationPlan::ApiKey {
                name: "key".into(),
                location: "query".into(),
            },
            "query.Set(\"key\", c.token)",
        ),
        (
            AuthenticationPlan::ApiKey {
                name: "key".into(),
                location: "cookie".into(),
            },
            "http.Cookie{Name:\"key\",Value:c.token}",
        ),
    ] {
        let mut plan = fixture();
        plan.authentication = auth;
        assert!(
            render(&Api::default(), &plan, "example.com/provider", "example")
                .unwrap()
                .get("internal/provider/provider.go")
                .unwrap()
                .contains(expected)
        );
    }
}
#[test]
#[ignore = "requires Go and cached/downloadable official Terraform Framework dependencies"]
fn generated_provider_executes_native_framework_lifecycle() {
    let root = tempfile::tempdir().unwrap();
    render(
        &Api::default(),
        &fixture(),
        "example.com/provider",
        "example",
    )
    .unwrap()
    .write_to(root.path())
    .unwrap();
    fs::write(
        root.path().join("internal/provider/lifecycle_test.go"),
        GO_TEST,
    )
    .unwrap();
    let cache = std::env::var("KAJI_TERRAFORM_GOMODCACHE")
        .unwrap_or_else(|_| "/tmp/kaji-tf-mod-cache".into());
    let mut command = Command::new("go");
    command
        .args(["test", "-mod=mod", "./..."])
        .current_dir(root.path())
        .env("GOCACHE", "/tmp/kaji-tf-go-cache")
        .env("GOMODCACHE", cache);
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
    for authentication in [
        AuthenticationPlan::None,
        AuthenticationPlan::Basic,
        AuthenticationPlan::ApiKey {
            name: "X-Key".into(),
            location: "header".into(),
        },
        AuthenticationPlan::ApiKey {
            name: "key".into(),
            location: "query".into(),
        },
        AuthenticationPlan::ApiKey {
            name: "key".into(),
            location: "cookie".into(),
        },
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut catalog = fixture();
        catalog.authentication = authentication;
        render(&Api::default(), &catalog, "example.com/provider", "example")
            .unwrap()
            .write_to(directory.path())
            .unwrap();
        let before_module = fs::read(directory.path().join("go.mod")).unwrap();
        let before_sum = fs::read(directory.path().join("go.sum")).unwrap();
        let mut tidy = Command::new("go");
        tidy.args(["mod", "tidy"])
            .current_dir(directory.path())
            .env("GOCACHE", "/tmp/kaji-tf-go-cache")
            .env(
                "GOMODCACHE",
                std::env::var("KAJI_TERRAFORM_GOMODCACHE")
                    .unwrap_or_else(|_| "/tmp/kaji-tf-mod-cache".into()),
            );
        if std::env::var_os("KAJI_TERRAFORM_OFFLINE").is_some() {
            tidy.env("GOPROXY", "off");
        }
        let output = tidy.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(directory.path().join("go.mod")).unwrap(),
            before_module,
            "go mod tidy changed owned manifest"
        );
        assert_eq!(
            fs::read(directory.path().join("go.sum")).unwrap(),
            before_sum,
            "go mod tidy changed owned lock"
        );
        let mut command = Command::new("go");
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
            "Authentication {:?}: {}\n{}",
            catalog.authentication,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
const GO_TEST: &str = r#"package provider
import (
 "context"
 "fmt"
 "io"
 "net/http"
 "strings"
 "testing"
 "github.com/hashicorp/terraform-plugin-framework/resource"
 rschema "github.com/hashicorp/terraform-plugin-framework/resource/schema"
 "github.com/hashicorp/terraform-plugin-framework/provider"
 "github.com/hashicorp/terraform-plugin-framework/tfsdk"
 "github.com/hashicorp/terraform-plugin-framework/types"
 "github.com/hashicorp/terraform-plugin-go/tftypes"
)
type fakeHTTP struct{mode string;calls int;lastBody string}
func(f *fakeHTTP)Do(req *http.Request)(*http.Response,error){
 f.calls++;if req.Header.Get("Authorization")!="Bearer secret"{return nil,fmt.Errorf("missing declared bearer credential")}
 if req.Method!="POST"&&req.URL.EscapedPath()!="/things/a%2Fb"{return nil,fmt.Errorf("identity not escaped: %s",req.URL.EscapedPath())}
 if req.Body!=nil{body,_:=io.ReadAll(req.Body);f.lastBody=string(body)}
 status:=200;body:=`{"id":"a/b","name":"planned","enabled":true,"count":9007199254740993,"status":"ready"}`
 switch f.mode {
 case "drift":body=`{"id":"a/b","name":"drifted","enabled":false,"count":9007199254740994,"ratio":1.5,"status":"changed"}`
 case "missing":status=404;body=""
 case "pending":status=202;body=""
 case "denied":status=401;body=""
 case "malformed":body=`{"id":"a/b","name":42}`
 case "normalized":body=`{"id":"a/b","name":"server-normalized","enabled":true,"count":9007199254740993,"status":"ready"}`
 case "update":body=`{"id":"a/b","name":"updated","enabled":true,"count":9007199254740993,"status":"updated"}`
 case "no-content":if req.Method=="PATCH"{status=204;body=""}else{body=`{"id":"a/b","name":"updated","enabled":true,"count":9007199254740993,"status":"updated"}`}
 case "missing-computed":body=`{"id":"a/b","name":"planned","enabled":true,"count":9007199254740993}`
 case "null-required":body=`{"id":"a/b","name":null,"enabled":true,"count":9007199254740993,"status":"ready"}`
 case "changed-id":body=`{"id":"different","name":"updated","enabled":true,"count":9007199254740993,"status":"updated"}`
 }
 return &http.Response{StatusCode:status,Header:http.Header{"Content-Type":[]string{"application/json"}},Body:io.NopCloser(strings.NewReader(body))},nil
}
func schemaFor(ctx context.Context,r *ThingResource)rschema.Schema{var response resource.SchemaResponse;r.Schema(ctx,resource.SchemaRequest{},&response);return response.Schema}
func planFor(t *testing.T,ctx context.Context,s rschema.Schema,data ThingResourceModel)tfsdk.Plan{
 value:=tfsdk.Plan{Schema:s,Raw:tftypes.NewValue(s.Type().TerraformType(ctx),nil)};if d:=value.Set(ctx,&data);d.HasError(){t.Fatal(d)};return value
}
func modelFrom(t *testing.T,ctx context.Context,state tfsdk.State)ThingResourceModel{var data ThingResourceModel;if d:=state.Get(ctx,&data);d.HasError(){t.Fatal(d)};return data}
func TestNativeLifecycle(t *testing.T){
 ctx:=context.Background();transport:=&fakeHTTP{};r:=&ThingResource{client:&apiClient{baseURL:"https://example.test",token:"secret",httpClient:transport}};schema:=schemaFor(ctx,r)
 planned:=ThingResourceModel{ID:types.StringUnknown(),Name:types.StringValue("planned"),Enabled:types.BoolValue(true),Quantity:types.Int64Value(9007199254740993),Ratio:types.Float64Null(),Status:types.StringUnknown()}
 plan:=planFor(t,ctx,schema,planned);created:=resource.CreateResponse{State:tfsdk.State{Schema:schema,Raw:plan.Raw}}
 r.Create(ctx,resource.CreateRequest{Plan:plan},&created);if created.Diagnostics.HasError(){t.Fatal(created.Diagnostics)}
 data:=modelFrom(t,ctx,created.State);if data.ID.ValueString()!="a/b"||data.Name.ValueString()!="planned"||data.Quantity.ValueInt64()!=9007199254740993||data.Status.ValueString()!="ready"||!data.Ratio.IsNull(){t.Fatalf("create state %#v",data)}
 if strings.Contains(transport.lastBody,"status")||strings.Contains(transport.lastBody,"ratio")||strings.Contains(transport.lastBody,`"id"`)||!strings.Contains(transport.lastBody,"9007199254740993"){t.Fatal("incorrect request body",transport.lastBody)}
 transport.mode="drift";read:=resource.ReadResponse{State:created.State};r.Read(ctx,resource.ReadRequest{State:created.State},&read);if read.Diagnostics.HasError(){t.Fatal(read.Diagnostics)}
 drift:=modelFrom(t,ctx,read.State);if drift.Name.ValueString()!="drifted"||drift.Enabled.ValueBool()||drift.Ratio.ValueFloat64()!=1.5{t.Fatal("drift not refreshed")}
 planned.Name=types.StringValue("updated");plan=planFor(t,ctx,schema,planned);transport.mode="update";updated:=resource.UpdateResponse{State:created.State};r.Update(ctx,resource.UpdateRequest{State:created.State,Plan:plan},&updated);if updated.Diagnostics.HasError(){t.Fatal(updated.Diagnostics)};data=modelFrom(t,ctx,updated.State);if data.ID.ValueString()!="a/b"||data.Name.ValueString()!="updated"{t.Fatal("update lost identity or planned value")}
 transport.mode="no-content";calls:=transport.calls;noContent:=resource.UpdateResponse{State:created.State};r.Update(ctx,resource.UpdateRequest{State:created.State,Plan:plan},&noContent);if noContent.Diagnostics.HasError()||transport.calls!=calls+2{t.Fatal("204 update did not refresh",noContent.Diagnostics)}
 transport.mode="changed-id";changedIdentity:=resource.UpdateResponse{State:created.State};r.Update(ctx,resource.UpdateRequest{State:created.State,Plan:plan},&changedIdentity);if !changedIdentity.Diagnostics.HasError(){t.Fatal("update identity changed silently")}
 for _,mode:=range []string{"missing-computed","null-required"}{transport.mode=mode;invalid:=resource.ReadResponse{State:created.State};r.Read(ctx,resource.ReadRequest{State:created.State},&invalid);if !invalid.Diagnostics.HasError()||invalid.State.Raw.IsNull(){t.Fatal("invalid read response removed state")}}
 transport.mode="denied";denied:=resource.ReadResponse{State:created.State};r.Read(ctx,resource.ReadRequest{State:created.State},&denied);if !denied.Diagnostics.HasError()||denied.State.Raw.IsNull(){t.Fatal("401 removed state")}
 transport.mode="pending";pendingDelete:=resource.DeleteResponse{State:created.State};r.Delete(ctx,resource.DeleteRequest{State:created.State},&pendingDelete);if !pendingDelete.Diagnostics.HasError()||pendingDelete.State.Raw.IsNull(){t.Fatal("202 pending delete lost state")}
 transport.mode="missing";missing:=resource.ReadResponse{State:created.State};r.Read(ctx,resource.ReadRequest{State:created.State},&missing);if missing.Diagnostics.HasError()||!missing.State.Raw.IsNull(){t.Fatal("404 did not remove state")}
 deleted:=resource.DeleteResponse{State:created.State};r.Delete(ctx,resource.DeleteRequest{State:created.State},&deleted);if deleted.Diagnostics.HasError()||!deleted.State.Raw.IsNull(){t.Fatal("delete did not remove state",deleted.Diagnostics)}
 imported:=resource.ImportStateResponse{State:tfsdk.State{Schema:schema,Raw:tftypes.NewValue(schema.Type().TerraformType(ctx),nil)}};r.ImportState(ctx,resource.ImportStateRequest{ID:"a/b"},&imported);if imported.Diagnostics.HasError(){t.Fatal(imported.Diagnostics)};if modelFrom(t,ctx,imported.State).ID.ValueString()!="a/b"{t.Fatal("import identity missing")}
 transport.mode="normalized";plan=planFor(t,ctx,schema,ThingResourceModel{ID:types.StringUnknown(),Name:types.StringValue("planned"),Enabled:types.BoolValue(true),Quantity:types.Int64Value(9007199254740993),Ratio:types.Float64Null(),Status:types.StringUnknown()});bad:=resource.CreateResponse{State:tfsdk.State{Schema:schema,Raw:plan.Raw}};r.Create(ctx,resource.CreateRequest{Plan:plan},&bad);if !bad.Diagnostics.HasError(){t.Fatal("server changed known plan without diagnostic")};if modelFrom(t,ctx,bad.State).ID.ValueString()!="a/b"||!bad.State.Raw.IsFullyKnown(){t.Fatal("recoverable create identity or fully known state was lost")}
}
func TestProviderUnknownConfiguration(t *testing.T){
 ctx:=context.Background();p:=&generatedProvider{};var s provider.SchemaResponse;p.Schema(ctx,provider.SchemaRequest{},&s)
 config:=tfsdk.Config{Schema:s.Schema,Raw:tftypes.NewValue(s.Schema.Type().TerraformType(ctx),map[string]tftypes.Value{"base_url":tftypes.NewValue(tftypes.String,tftypes.UnknownValue),"auth_token":tftypes.NewValue(tftypes.String,"secret"),"username":tftypes.NewValue(tftypes.String,nil),"password":tftypes.NewValue(tftypes.String,nil)})};var result provider.ConfigureResponse;p.Configure(ctx,provider.ConfigureRequest{Config:config},&result);if !result.Diagnostics.HasError(){t.Fatal("unknown base URL accepted")}
}
"#;

#[test]
fn data_sources_are_opt_in_and_computed() {
    let catalog = fixture();
    let mut tree = render(&Api::default(), &catalog, "example.com/provider", "example").unwrap();
    assert!(
        tree.get("internal/provider/provider.go")
            .unwrap()
            .contains("DataSource{return nil}")
    );
    add_data_sources(&mut tree, &catalog, "example").unwrap();
    let source = tree.get("internal/provider/data_source_thing.go").unwrap();
    assert!(source.contains("StringAttribute{Required:true}"));
    assert!(source.contains("Computed:true"));
    assert!(source.contains("data.hydrate(body,true,true)"));
}
#[test]
#[ignore = "requires Go and official Framework dependencies"]
fn generated_data_source_executes_native_framework_read() {
    let root = tempfile::tempdir().unwrap();
    let catalog = fixture();
    let mut tree = render(&Api::default(), &catalog, "example.com/provider", "example").unwrap();
    add_data_sources(&mut tree, &catalog, "example").unwrap();
    tree.write_to(root.path()).unwrap();
    fs::write(
        root.path().join("internal/provider/lifecycle_test.go"),
        GO_TEST,
    )
    .unwrap();
    fs::write(
        root.path().join("internal/provider/data_source_test.go"),
        DATA_SOURCE_TEST,
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-mod=readonly", "./..."])
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
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
const DATA_SOURCE_TEST: &str = r#"package provider
import("context";"testing";"github.com/hashicorp/terraform-plugin-framework/datasource";"github.com/hashicorp/terraform-plugin-framework/tfsdk";"github.com/hashicorp/terraform-plugin-framework/types";"github.com/hashicorp/terraform-plugin-go/tftypes")
func TestDataSourceRead(t *testing.T){ctx:=context.Background();transport:=&fakeHTTP{};d:=&ThingDataSource{client:&apiClient{baseURL:"https://example.test",token:"secret",httpClient:transport}};var schema datasource.SchemaResponse;d.Schema(ctx,datasource.SchemaRequest{},&schema);config:=tfsdk.Config{Schema:schema.Schema,Raw:tftypes.NewValue(schema.Schema.Type().TerraformType(ctx),nil)};model:=ThingResourceModel{ID:types.StringValue("a/b"),Name:types.StringNull(),Enabled:types.BoolNull(),Quantity:types.Int64Null(),Ratio:types.Float64Null(),Status:types.StringNull()};state:=tfsdk.State{Schema:schema.Schema,Raw:config.Raw};if diag:=state.Set(ctx,&model);diag.HasError(){t.Fatal(diag)};config.Raw=state.Raw
 for _,mode:=range []string{"","missing","denied","malformed","missing-computed","changed-id"}{transport.mode=mode;response:=datasource.ReadResponse{State:tfsdk.State{Schema:schema.Schema,Raw:config.Raw}};d.Read(ctx,datasource.ReadRequest{Config:config},&response);if mode==""{if response.Diagnostics.HasError(){t.Fatal(response.Diagnostics)};var actual ThingResourceModel;if diag:=response.State.Get(ctx,&actual);diag.HasError(){t.Fatal(diag)};if actual.Quantity.ValueInt64()!=9007199254740993||actual.Name.ValueString()!="planned"{t.Fatal(actual)}}else if !response.Diagnostics.HasError(){t.Fatal("missing diagnostic",mode)}}
}
"#;

#[test]
#[ignore = "requires Terraform CLI (KAJI_TERRAFORM_BIN), Go and official Framework cache"]
fn terraform_cli_local_mock_lifecycle() {
    use std::time::Duration;
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
state={'id':'a/b','name':'planned','enabled':True,'count':9007199254740993,'status':'ready'}
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def respond(self,status,body=None):
  self.send_response(status);self.send_header('Content-Type','application/json');self.end_headers()
  if body is not None:self.wfile.write(json.dumps(body).encode())
 def do_GET(self):
  mode=pathlib.Path(sys.argv[2]).read_text()
  if mode=='denied':return self.respond(401)
  if mode=='missing':return self.respond(404)
  if mode=='drift':state['name']='drifted'
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
            "terraform {{\n required_providers {{\n example = {{ source = \"kaji/example\" }}\n }}\n }}\nprovider \"example\" {{\n base_url = \"http://127.0.0.1:{port}\"\n auth_token = \"secret\"\n }}\nresource \"example_thing\" \"test\" {{\n name = {name:?}\n enabled = true\n quantity = 9007199254740993\n }}\ndata \"example_thing\" \"existing\" {{ id = example_thing.test.id }}\noutput \"count\" {{ value = data.example_thing.existing.quantity }}"
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
    assert_eq!(
        managed["values"]["quantity"],
        serde_json::json!(9007199254740993_i64)
    );
    assert_eq!(managed["values"]["name"], "updated");
    run(&["state", "rm", "example_thing.test"]);
    run(&["import", "-input=false", "example_thing.test", "a/b"]);
    run(&["plan", "-detailed-exitcode", "-input=false", "-no-color"]);
    run(&["destroy", "-auto-approve", "-input=false", "-no-color"]);
}
