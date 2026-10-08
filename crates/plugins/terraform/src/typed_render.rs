//! Typed, explicitly planned Plugin Framework rendering. No lifecycle inference.
use crate::plan::{AuthenticationPlan, EntityCatalog, ResourcePlan, ScalarType};
use anyhow::Result;
use poolster_core::{Api, GeneratedFile, GeneratedTree};
use std::fmt::Write;

pub(crate) fn render(
    api: &Api,
    catalog: &EntityCatalog,
    module: &str,
    provider: &str,
) -> Result<GeneratedTree> {
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new(
        "go.mod",
        include_str!("../templates/go.mod.tmpl").replace("__MODULE__", module),
    )?)?;
    tree.insert(GeneratedFile::new(
        "go.sum",
        include_str!("../templates/go.sum"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "main.go",
        crate::main_go(module, provider),
    )?)?;
    tree.insert(GeneratedFile::new(
        "internal/provider/provider.go",
        provider_source(catalog, provider),
    )?)?;
    tree.insert(GeneratedFile::new(
        "internal/provider/nested_values.go",
        include_str!("../templates/nested_runtime.go.tmpl"),
    )?)?;
    if catalog
        .resources
        .iter()
        .any(|resource| resource.polling.is_some())
    {
        tree.insert(GeneratedFile::new(
            "internal/provider/polling.go",
            include_str!("../templates/polling_runtime.go.tmpl"),
        )?)?;
    }
    for resource in &catalog.resources {
        tree.insert(GeneratedFile::new(
            format!("internal/provider/resource_{}.go", resource.name),
            resource_source(resource),
        )?)?;
    }
    tree.insert(GeneratedFile::new(
        "internal/provider/transport_test.go",
        transport_test(catalog),
    )?)?;
    let mut readme = format!(
        "# {provider} Terraform provider\n\nGenerated from {} using validated lifecycle bindings. Go module: `{module}`.\n\n## Build and verify\n\n```sh\ngo mod tidy\ngo test ./...\ngo build -o terraform-provider-{provider} .\n```\n\nEmitted tests exercise the native transport, credential gating, redacted errors and identity encoding without calling a live API. Configure a Terraform CLI `provider_installation.dev_overrides` entry for `registry.terraform.io/poolster/{provider}` pointing at the directory containing this binary when developing locally. The generated package is not automatically published to a Terraform registry.\n\n## Provider configuration\n\n```hcl\nterraform {{\n  required_providers {{\n    {provider} = {{ source = \"poolster/{provider}\" }}\n  }}\n}}\n\nprovider \"{provider}\" {{\n  base_url = \"https://api.example.com\"\n",
        api.name
    );
    match &catalog.authentication {
        AuthenticationPlan::None => {}
        AuthenticationPlan::Basic => {
            readme.push_str("  username = var.api_username\n  password = var.api_password\n")
        }
        _ => readme.push_str("  auth_token = var.api_token\n"),
    }
    readme.push_str("}\n");
    match catalog.authentication {AuthenticationPlan::None=>{},AuthenticationPlan::Basic=>readme.push_str("\nvariable \"api_username\" {\n  type = string\n  sensitive = true\n}\nvariable \"api_password\" {\n  type = string\n  sensitive = true\n}\n"),_=>readme.push_str("\nvariable \"api_token\" {\n  type = string\n  sensitive = true\n}\n")}
    readme.push_str("```\n\nDeclare any referenced credential variables as sensitive strings and provide their values through your normal Terraform configuration. Only the selected OpenAPI authentication scheme is applied, and public resources do not receive credentials. `base_url` must be an absolute HTTP(S) URL without credentials, query or fragment. Native calls preserve cancellation, use a 30-second timeout, do not follow redirects and do not automatically replay CRUD requests.\n\n");
    for resource in &catalog.resources {
        let _ = writeln!(
            readme,
            "## `{provider}_{}`\n\nCreate: `{}`; Read: `{}`; Delete: `{}`. Identity uses response field `{}` and path parameter `{}`.\n\n| Attribute | Type | Configuration |\n| --- | --- | --- |\n| `id` | string | Computed; populated by create/import |",
            resource.name,
            resource.create.id,
            resource.read.id,
            resource.delete.id,
            resource.id_field,
            resource.id_parameter
        );
        for attr in &resource.attributes {
            if attr.name == "id" {
                continue;
            }
            let presence = if attr.required {
                "Required"
            } else if attr.optional && attr.computed {
                "Optional + Computed"
            } else if attr.optional {
                "Optional"
            } else {
                "Computed"
            };
            let _ = writeln!(
                readme,
                "| `{}` | {} | {}{}{} |",
                attr.name,
                attr.shape.as_ref().map_or_else(
                    || go_scalar(attr.ty).to_owned(),
                    |shape| format!(
                        "{} (typed)",
                        crate::nested_render::native(shape).to_ascii_lowercase()
                    )
                ),
                presence,
                if attr.replace_on_change {
                    "; changing replaces the resource"
                } else {
                    ""
                },
                if attr.sensitive { "; sensitive" } else { "" }
            );
        }
        let _ = writeln!(
            readme,
            "\n```hcl\nresource \"{provider}_{}\" \"example\" {{",
            resource.name
        );
        for attr in &resource.attributes {
            if attr.required && attr.name != "id" {
                let scalar_value = match attr.ty {
                    ScalarType::String => "\"example\"",
                    ScalarType::Bool => "true",
                    ScalarType::Int64 => "1",
                    ScalarType::Float64 => "1.0",
                };
                let value = attr
                    .shape
                    .as_ref()
                    .map_or_else(|| scalar_value.to_owned(), crate::nested_render::example);
                let _ = writeln!(readme, "  {} = {value}", attr.name);
            }
        }
        let import_id = if resource.identity.is_empty() {
            "remote-object-id".to_owned()
        } else {
            serde_json::to_string(
                &resource
                    .identity
                    .iter()
                    .map(|id| {
                        (
                            id.parameter.clone(),
                            if resource
                                .create
                                .parameters
                                .iter()
                                .any(|p| p.name == id.parameter)
                            {
                                "example"
                            } else {
                                "remote-object-id"
                            },
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, _>>(),
            )
            .unwrap()
        };
        let _ = writeln!(
            readme,
            "}}\n```\n\nImport an existing object with its API identity:\n\n```sh\nterraform import {provider}_{}.example '{import_id}'\n```\n",
            resource.name
        );
    }
    readme.push_str("## Lifecycle behavior\n\nCreate sends known configurable values and saves the returned identity. Create/update preserve known planned configuration and report a diagnostic if the API returns different values. Unknown computed values are hydrated; diagnostic create failures retain known identity and resolve remaining unknown state for recovery. Update sends only fields allowed by the update operation, then reads the object to hydrate final computed values, including after HTTP 204. PATCH sends supported known fields, not a changed-fields diff. Read refreshes drift and removes state on HTTP 404; HTTP authentication/transport/decoding failures retain state. Delete succeeds for an already-missing object and removes state. Import initializes identity, then Read refreshes the object.\n\nSupported attributes include bounded fixed nested objects, typed lists and typed maps; nullable values, recursive schemas, unions, constraints and nested readOnly/writeOnly projections are rejected. Composite identities are opt-in JSON objects keyed by configured path parameters; configured parent path components are required replacement attributes and the child identity is server-generated. Read-only data sources are opt-in. Explicit lifecycle polling can wait through the bound read operation after create/update/delete. The deadline includes the initial mutation, requests are never replayed, status/body conditions are AND groups with failure taking precedence, and deletion completes only after HTTP 404. Creates retain recovered identity and configured parent values when waiting fails; updates/deletes retain prior state. Without an explicit waiter HTTP 202 remains rejected. Terraform write-only arguments remain unsupported. Explicit state upgrades support lossless root attribute renames only, with each old version mapping directly to the current schema, not chaining intermediate migrations; all other state must retain compatible types and no data is discarded. Custom type transformations require an authored upgrade implementation. The emitted Go unit tests do not perform `terraform apply`; the Poolster repository provides an opt-in Terraform CLI lifecycle test against its local mock. Verify your API's lifecycle behavior before distributing the provider.\n");
    tree.insert(GeneratedFile::new("README.md", readme)?)?;
    Ok(tree)
}
fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
pub(crate) fn field(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}
fn native(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::String => "String",
        ScalarType::Bool => "Bool",
        ScalarType::Int64 => "Int64",
        ScalarType::Float64 => "Float64",
    }
}
fn go_scalar(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::String => "string",
        ScalarType::Bool => "bool",
        ScalarType::Int64 => "int64",
        ScalarType::Float64 => "float64",
    }
}
fn provider_source(catalog: &EntityCatalog, name: &str) -> String {
    let resources = catalog
        .resources
        .iter()
        .map(|r| format!("new{}Resource", field(&r.name)))
        .collect::<Vec<_>>()
        .join(", ");
    let auth = match &catalog.authentication {
        AuthenticationPlan::None => String::new(),
        AuthenticationPlan::Bearer => {
            "if secure { request.Header.Set(\"Authorization\", \"Bearer \"+c.token) }".into()
        }
        AuthenticationPlan::Basic => {
            "if secure { request.SetBasicAuth(c.username, c.password) }".into()
        }
        AuthenticationPlan::ApiKey { name, location } => match location.as_str() {
            "header" => format!(
                "if secure {{ request.Header.Set({}, c.token) }}",
                quoted(name)
            ),
            "query" => format!(
                "if secure {{ query:=request.URL.Query(); query.Set({}, c.token); request.URL.RawQuery=query.Encode() }}",
                quoted(name)
            ),
            "cookie" => format!(
                "if secure {{ request.AddCookie(&http.Cookie{{Name:{},Value:c.token}}) }}",
                quoted(name)
            ),
            _ => unreachable!("validated authentication location"),
        },
    };
    let credentials = match catalog.authentication {
        AuthenticationPlan::None => "",
        AuthenticationPlan::Basic => {
            "if data.Username.IsUnknown() || data.Password.IsUnknown() || data.Username.IsNull() || data.Password.IsNull() { resp.Diagnostics.AddError(\"Missing basic credentials\", \"Set known username and password provider arguments.\"); return }"
        }
        _ => {
            "if data.AuthToken.IsUnknown() || data.AuthToken.IsNull() || data.AuthToken.ValueString()==\"\" { resp.Diagnostics.AddError(\"Missing API credentials\", \"Set a known auth_token provider argument.\"); return }"
        }
    };
    let mut source = PROVIDER.to_owned();
    if catalog
        .resources
        .iter()
        .any(|resource| resource.polling.is_some())
    {
        source.push_str(crate::polling_render::TRANSPORT);
    }
    source
        .replace("__NAME__", &quoted(name))
        .replace("__RESOURCES__", &resources)
        .replace("__AUTH__", &auth)
        .replace("__CREDENTIALS__", credentials)
}
const PROVIDER: &str = r#"package provider
import (
 "context"
 "bytes"
 "fmt"
 "io"
 "net/http"
 "net/url"
 "strings"
 "time"
 "github.com/hashicorp/terraform-plugin-framework/datasource"
 "github.com/hashicorp/terraform-plugin-framework/provider"
 "github.com/hashicorp/terraform-plugin-framework/provider/schema"
 "github.com/hashicorp/terraform-plugin-framework/resource"
 "github.com/hashicorp/terraform-plugin-framework/types"
)
var _ provider.Provider = &generatedProvider{}
type generatedProvider struct{version string}
type providerModel struct {
 BaseURL types.String `tfsdk:"base_url"`
 AuthToken types.String `tfsdk:"auth_token"`
 Username types.String `tfsdk:"username"`
 Password types.String `tfsdk:"password"`
}
func New(version string) func() provider.Provider {return func()provider.Provider{return &generatedProvider{version:version}}}
func(p *generatedProvider)Metadata(_ context.Context,_ provider.MetadataRequest,resp *provider.MetadataResponse){resp.TypeName=__NAME__;resp.Version=p.version}
func(p *generatedProvider)Schema(_ context.Context,_ provider.SchemaRequest,resp *provider.SchemaResponse){resp.Schema=schema.Schema{Attributes:map[string]schema.Attribute{
 "base_url":schema.StringAttribute{Required:true,Description:"API origin and optional base path."},
 "auth_token":schema.StringAttribute{Optional:true,Sensitive:true,Description:"Credential for the declared bearer or API-key scheme."},
 "username":schema.StringAttribute{Optional:true,Sensitive:true},
 "password":schema.StringAttribute{Optional:true,Sensitive:true},
}}}
func(p *generatedProvider)Configure(ctx context.Context,req provider.ConfigureRequest,resp *provider.ConfigureResponse){
 var data providerModel;resp.Diagnostics.Append(req.Config.Get(ctx,&data)...);if resp.Diagnostics.HasError(){return}
 if data.BaseURL.IsUnknown()||data.BaseURL.IsNull(){resp.Diagnostics.AddError("Missing API origin","Set a known base_url provider argument.");return}
 endpoint,err:=url.Parse(data.BaseURL.ValueString());if err!=nil||endpoint.Host==""||(endpoint.Scheme!="http"&&endpoint.Scheme!="https")||endpoint.User!=nil||endpoint.RawQuery!=""||endpoint.Fragment!=""{resp.Diagnostics.AddError("Invalid API origin","base_url must be an absolute HTTP(S) URL without credentials, query or fragment.");return}
 __CREDENTIALS__
 client:=&apiClient{baseURL:strings.TrimRight(endpoint.String(),"/"),token:data.AuthToken.ValueString(),username:data.Username.ValueString(),password:data.Password.ValueString(),httpClient:&http.Client{Timeout:30*time.Second,CheckRedirect:func(*http.Request,[]*http.Request)error{return http.ErrUseLastResponse}}}
 resp.ResourceData=client
 resp.DataSourceData=client
}
func(p *generatedProvider)Resources(_ context.Context)[]func()resource.Resource{return []func()resource.Resource{__RESOURCES__}}
func(p *generatedProvider)DataSources(_ context.Context)[]func()datasource.DataSource{return nil}
func identityPath(id string)string{if id=="."{return "%2E"};if id==".."{return "%2E%2E"};return url.PathEscape(id)}
type httpExecutor interface{Do(*http.Request)(*http.Response,error)}
type apiClient struct{baseURL,token,username,password string;httpClient httpExecutor}
type apiStatusError struct{status int}
func(e *apiStatusError)Error()string{return fmt.Sprintf("API returned HTTP %d",e.status)}
func(c *apiClient)call(ctx context.Context,method,route string,body []byte,secure bool)([]byte,error){
 request,err:=http.NewRequestWithContext(ctx,method,c.baseURL+route,bytes.NewReader(body));if err!=nil{return nil,err}
 request.Header.Set("Accept","application/json");if len(body)>0{request.Header.Set("Content-Type","application/json")}
 __AUTH__
 response,err:=c.httpClient.Do(request);if err!=nil{return nil,fmt.Errorf("API transport failed")};defer response.Body.Close()
 if response.StatusCode==202{return nil,&apiStatusError{status:202}}
 data,err:=io.ReadAll(io.LimitReader(response.Body,(10<<20)+1));if err!=nil{return nil,fmt.Errorf("API response body could not be read")};if len(data)>10<<20{return nil,fmt.Errorf("API response exceeds 10 MiB")}
 if response.StatusCode<200||response.StatusCode>=300{return nil,&apiStatusError{status:response.StatusCode}}
 return data,nil
}
"#;
fn resource_source(plan: &ResourcePlan) -> String {
    let ty = format!("{}Resource", field(&plan.name));
    let model = format!("{ty}Model");
    let mut output = format!(
        r#"package provider
import (
 "context"
 "bytes"
 "encoding/json"
 "errors"
 "fmt"
 "strings"
 "github.com/hashicorp/terraform-plugin-framework/path"
 "github.com/hashicorp/terraform-plugin-framework/resource"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/boolplanmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/int64planmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/float64planmodifier"
 "github.com/hashicorp/terraform-plugin-framework/attr"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/objectplanmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/listplanmodifier"
 "github.com/hashicorp/terraform-plugin-framework/resource/schema/mapplanmodifier"
 "github.com/hashicorp/terraform-plugin-framework/types"
)
var _ resource.Resource = &{ty}{{}}
var _ resource.ResourceWithImportState = &{ty}{{}}
// Keep plan-modifier packages usable for scalar-only generated resources.
var _ = attr.Type(types.StringType)
var _ = objectplanmodifier.RequiresReplace
var _ = listplanmodifier.RequiresReplace
var _ = mapplanmodifier.RequiresReplace
var _ = boolplanmodifier.RequiresReplace
var _ = int64planmodifier.RequiresReplace
var _ = float64planmodifier.RequiresReplace
type {ty} struct{{client *apiClient}}
type {model} struct{{
 ID types.String `tfsdk:"id"`
"#
    );
    for attr in &plan.attributes {
        if attr.name != "id" {
            let _ = writeln!(
                output,
                " {} types.{} `tfsdk:{}`",
                field(&attr.name),
                attr.shape
                    .as_ref()
                    .map_or(native(attr.ty), crate::nested_render::native),
                quoted(&attr.name)
            );
        }
    }
    let _ = writeln!(
        output,
        "}}\nfunc new{ty}() resource.Resource{{return &{ty}{{}}}}\nfunc(r *{ty})Metadata(_ context.Context,req resource.MetadataRequest,resp *resource.MetadataResponse){{resp.TypeName=req.ProviderTypeName+{}}}",
        quoted(&format!("_{}", plan.name))
    );
    let _ = writeln!(
        output,
        "func(r *{ty})Schema(_ context.Context,_ resource.SchemaRequest,resp *resource.SchemaResponse){{resp.Schema=schema.Schema{{Version:{},Attributes:map[string]schema.Attribute{{\n\"id\":schema.StringAttribute{{Computed:true,PlanModifiers:[]planmodifier.String{{stringplanmodifier.UseStateForUnknown()}}}},",
        plan.schema_version
    );
    for attr in &plan.attributes {
        if attr.name == "id" {
            continue;
        }
        if let Some(shape) = &attr.shape {
            let _ = writeln!(
                output,
                "{}:{},",
                quoted(&attr.name),
                crate::nested_render::schema(
                    shape,
                    attr.required,
                    attr.optional,
                    attr.computed,
                    attr.sensitive,
                    attr.replace_on_change
                )
            );
            continue;
        }
        let native = native(attr.ty);
        let modifier = if attr.replace_on_change {
            format!(
                ",PlanModifiers:[]planmodifier.{native}{{{}planmodifier.RequiresReplace()}}",
                native.to_lowercase()
            )
        } else {
            String::new()
        };
        let _ = writeln!(
            output,
            "{}:schema.{native}Attribute{{Required:{},Optional:{},Computed:{},Sensitive:{}{modifier}}},",
            quoted(&attr.name),
            attr.required,
            attr.optional,
            attr.computed,
            attr.sensitive
        );
    }
    let _ = writeln!(
        output,
        "}}}}}}\nfunc(r *{ty})Configure(_ context.Context,req resource.ConfigureRequest,resp *resource.ConfigureResponse){{if req.ProviderData==nil{{return}};client,ok:=req.ProviderData.(*apiClient);if !ok{{resp.Diagnostics.AddError(\"Unexpected provider data\",\"Native provider configuration did not produce its API client.\");return}};r.client=client}}\nfunc(r *{ty})ImportState(ctx context.Context,req resource.ImportStateRequest,resp *resource.ImportStateResponse){{if req.ID==\"\"{{resp.Diagnostics.AddError(\"Invalid import identity\",\"Import requires a nonempty string ID.\");return}};resource.ImportStatePassthroughID(ctx,path.Root(\"id\"),req,resp)}}"
    );
    for attr in &plan.attributes {
        if let Some(shape) = &attr.shape {
            let _ = writeln!(
                output,
                "var {ty}{}Shape = parseWireShape({})",
                field(&attr.name),
                quoted(&serde_json::to_string(shape).expect("shape serialization"))
            );
        }
    }
    let _ = writeln!(
        output,
        "func(data *{model})payload(update bool)([]byte,error){{body:=map[string]any{{}}"
    );
    for attr in &plan.attributes {
        if attr.name == "id"
            || (!attr.required && !attr.optional)
            || plan.identity.iter().any(|id| {
                id.field == attr.wire_name
                    && plan
                        .create
                        .parameters
                        .iter()
                        .any(|p| p.name == id.parameter)
            })
        {
            continue;
        }
        let f = field(&attr.name);
        let _ = writeln!(output, "if !update||{}{{", attr.update_input);
        let _ = writeln!(
            output,
            "if ((!update&&{})||(update&&{}))&&(data.{f}.IsNull()||data.{f}.IsUnknown()){{return nil,errors.New({})}}",
            attr.required,
            attr.update_required,
            quoted(&format!("required attribute {} must be known", attr.name))
        );
        if attr.shape.is_some() {
            let _ = writeln!(
                output,
                "if !data.{f}.IsNull()&&!data.{f}.IsUnknown(){{wire,err:={ty}{f}Shape.encode(data.{f});if err!=nil{{return nil,err}};body[{}]=wire}}}}",
                quoted(&attr.wire_name)
            );
            continue;
        }
        let _ = writeln!(
            output,
            "if !data.{f}.IsNull()&&!data.{f}.IsUnknown(){{body[{}]=data.{f}.Value{}()}}\n}}",
            quoted(&attr.wire_name),
            native(attr.ty)
        );
    }
    output.push_str("return json.Marshal(body)\n}\n");
    let _ = writeln!(
        output,
        "func(data *{model})hydrate(body []byte,refresh bool,fromRead bool)error{{var values map[string]json.RawMessage;if err:=json.Unmarshal(body,&values);err!=nil{{return err}};if values==nil{{return fmt.Errorf(\"expected a JSON object response\")}}"
    );
    for attr in &plan.attributes {
        if attr.name == "id" {
            continue;
        }
        let f = field(&attr.name);
        let n = native(attr.ty);
        let missing = format!(
            "if !ok&&((fromRead&&{})||(!fromRead&&{})){{return errors.New({})}};if ok&&string(bytes.TrimSpace(value))==\"null\"{{return errors.New({})}}",
            attr.response_required,
            attr.required,
            quoted(&format!(
                "API response is missing required field {}",
                attr.wire_name
            )),
            quoted(&format!(
                "API returned null for nonnullable field {}",
                attr.wire_name
            ))
        );
        if let Some(shape) = &attr.shape {
            let n = crate::nested_render::native(shape);
            let _ = writeln!(
                output,
                "{{value,ok:=values[{}];{missing};incoming:={ty}{f}Shape.nullValue();if ok{{decoded,err:={ty}{f}Shape.decode(value);if err!=nil{{return err}};incoming=decoded}};merged,err:=mergeNested({ty}{f}Shape,data.{f},incoming,refresh);if err!=nil{{return err}};data.{f}=merged.(types.{n})}}",
                quoted(&attr.wire_name)
            );
            continue;
        }
        let decode_error = quoted(&format!(
            "invalid response field {}: %w",
            attr.wire_name.replace('%', "%%")
        ));
        let mismatch = quoted(&format!(
            "API changed known planned attribute {}; configure the server's accepted value explicitly",
            attr.name
        ));
        let _ = writeln!(
            output,
            "{{value,ok:=values[{}];{missing}; incoming:=types.{n}Null();if ok&&string(bytes.TrimSpace(value))!=\"null\"{{var decoded {};if err:=json.Unmarshal(value,&decoded);err!=nil{{return fmt.Errorf({decode_error},err)}};incoming=types.{n}Value(decoded)}};if refresh||data.{f}.IsUnknown(){{data.{f}=incoming}}else if ok&&!data.{f}.Equal(incoming){{return errors.New({mismatch})}}}}",
            quoted(&attr.wire_name),
            go_scalar(attr.ty)
        );
    }
    output.push_str("return nil\n}\n");
    let _ = writeln!(
        output,
        "func(data *{model})finalizeUnknowns(){{if data.ID.IsUnknown(){{data.ID=types.StringNull()}}"
    );
    for attr in &plan.attributes {
        if attr.name != "id" {
            let f = field(&attr.name);
            if let Some(shape) = &attr.shape {
                let n = crate::nested_render::native(shape);
                let _ = writeln!(
                    output,
                    "data.{f}=finalizeNested({ty}{f}Shape,data.{f}).(types.{n})"
                );
                continue;
            }
            let n = native(attr.ty);
            let _ = writeln!(
                output,
                "if data.{f}.IsUnknown(){{data.{f}=types.{n}Null()}}"
            );
        }
    }
    output.push_str("}\n");
    let _ = writeln!(
        output,
        "func {ty}Identity(body []byte)(string,error){{var values map[string]json.RawMessage;if err:=json.Unmarshal(body,&values);err!=nil{{return \"\",err}};var id string;if err:=json.Unmarshal(values[{}],&id);err!=nil||id==\"\"{{return \"\",fmt.Errorf(\"API response requires a nonempty string identity\")}};return id,nil}}",
        quoted(&plan.id_field)
    );
    let secure = plan.requires_auth;
    let _ = writeln!(
        output,
        "func(r *{ty})Create(ctx context.Context,req resource.CreateRequest,resp *resource.CreateResponse){{var data {model};resp.Diagnostics.Append(req.Plan.Get(ctx,&data)...);if resp.Diagnostics.HasError(){{return}};if r.client==nil{{resp.Diagnostics.AddError(\"Unconfigured provider\",\"Configure the provider before creating resources.\");return}};payload,err:=data.payload(false);if err!=nil{{resp.Diagnostics.AddError(\"Invalid resource plan\",err.Error());return}};body,err:=r.client.call(ctx,{},{},payload,{secure});if err!=nil{{resp.Diagnostics.AddError(\"Create failed\",err.Error());return}};id,err:={ty}Identity(body);if err!=nil{{resp.Diagnostics.AddError(\"Invalid create response\",err.Error());return}};data.ID=types.StringValue(id);if err:=data.hydrate(body,false,false);err!=nil{{resp.Diagnostics.AddError(\"Invalid create response\",err.Error())}};data.finalizeUnknowns();resp.Diagnostics.Append(resp.State.Set(ctx,&data)... )}}",
        quoted(plan.create.method.as_str()),
        quoted(&plan.create.path)
    );
    let template = quoted(&format!("{{{}}}", plan.id_parameter));
    let _ = writeln!(
        output,
        "func(r *{ty})Read(ctx context.Context,req resource.ReadRequest,resp *resource.ReadResponse){{var data {model};resp.Diagnostics.Append(req.State.Get(ctx,&data)...);if resp.Diagnostics.HasError(){{return}};if r.client==nil{{resp.Diagnostics.AddError(\"Unconfigured provider\",\"Configure the provider before reading resources.\");return}};if data.ID.IsNull()||data.ID.IsUnknown()||data.ID.ValueString()==\"\"{{resp.Diagnostics.AddError(\"Missing resource identity\",\"Import or create a nonempty resource ID before this operation.\");return}};route:=strings.Replace({}, {template},identityPath(data.ID.ValueString()),1);body,err:=r.client.call(ctx,{},route,nil,{secure});if err!=nil{{var status *apiStatusError;if errors.As(err,&status)&&status.status==404{{resp.State.RemoveResource(ctx);return}};resp.Diagnostics.AddError(\"Read failed\",err.Error());return}};id,err:={ty}Identity(body);if err!=nil||id!=data.ID.ValueString(){{resp.Diagnostics.AddError(\"Invalid read identity\",\"Read response identity must match the managed object.\");return}};if err:=data.hydrate(body,true,true);err!=nil{{resp.Diagnostics.AddError(\"Invalid read response\",err.Error());return}};resp.Diagnostics.Append(resp.State.Set(ctx,&data)... )}}",
        quoted(&plan.read.path),
        quoted(plan.read.method.as_str())
    );
    if let Some(update) = &plan.update {
        let _ = writeln!(
            output,
            "func(r *{ty})Update(ctx context.Context,req resource.UpdateRequest,resp *resource.UpdateResponse){{var data,prior {model};resp.Diagnostics.Append(req.Plan.Get(ctx,&data)...);resp.Diagnostics.Append(req.State.Get(ctx,&prior)...);if resp.Diagnostics.HasError(){{return}};if r.client==nil{{resp.Diagnostics.AddError(\"Unconfigured provider\",\"Configure the provider before updating resources.\");return}};data.ID=prior.ID;payload,err:=data.payload(true);if err!=nil{{resp.Diagnostics.AddError(\"Invalid resource plan\",err.Error());return}};if data.ID.IsNull()||data.ID.IsUnknown()||data.ID.ValueString()==\"\"{{resp.Diagnostics.AddError(\"Missing resource identity\",\"Import or create a nonempty resource ID before this operation.\");return}};route:=strings.Replace({}, {template},identityPath(prior.ID.ValueString()),1);updatedBody,err:=r.client.call(ctx,{},route,payload,{secure});if err!=nil{{resp.Diagnostics.AddError(\"Update failed\",err.Error());return}};if len(updatedBody)>0{{var updated map[string]json.RawMessage;if err:=json.Unmarshal(updatedBody,&updated);err!=nil||updated==nil{{resp.Diagnostics.AddError(\"Invalid update response\",\"Update must return a JSON object or no body.\");return}};if _,present:=updated[{}];present{{updatedID,idErr:={ty}Identity(updatedBody);if idErr!=nil||updatedID!=prior.ID.ValueString(){{resp.Diagnostics.AddError(\"Invalid update identity\",\"Update response must retain the managed identity.\");return}}}}}};readRoute:=strings.Replace({}, {template},identityPath(prior.ID.ValueString()),1);body,err:=r.client.call(ctx,{},readRoute,nil,{secure});if err!=nil{{resp.Diagnostics.AddError(\"Update refresh failed\",err.Error());return}};id,err:={ty}Identity(body);if err!=nil||id!=prior.ID.ValueString(){{resp.Diagnostics.AddError(\"Invalid update identity\",\"Update refresh identity must match the managed object.\");return}};if err:=data.hydrate(body,false,true);err!=nil{{resp.Diagnostics.AddError(\"Invalid update response\",err.Error());return}};resp.Diagnostics.Append(resp.State.Set(ctx,&data)... )}}",
            quoted(&update.path),
            quoted(update.method.as_str()),
            quoted(&plan.id_field),
            quoted(&plan.read.path),
            quoted(plan.read.method.as_str())
        );
    } else {
        let _ = writeln!(
            output,
            "func(r *{ty})Update(_ context.Context,_ resource.UpdateRequest,resp *resource.UpdateResponse){{resp.Diagnostics.AddError(\"Update unsupported\",\"This resource must be replaced to change configuration.\")}}"
        );
    }
    let _ = writeln!(
        output,
        "func(r *{ty})Delete(ctx context.Context,req resource.DeleteRequest,resp *resource.DeleteResponse){{var data {model};resp.Diagnostics.Append(req.State.Get(ctx,&data)...);if resp.Diagnostics.HasError(){{return}};if r.client==nil{{resp.Diagnostics.AddError(\"Unconfigured provider\",\"Configure the provider before deleting resources.\");return}};if data.ID.IsNull()||data.ID.IsUnknown()||data.ID.ValueString()==\"\"{{resp.Diagnostics.AddError(\"Missing resource identity\",\"Import or create a nonempty resource ID before this operation.\");return}};route:=strings.Replace({}, {template},identityPath(data.ID.ValueString()),1);_,err:=r.client.call(ctx,{},route,nil,{secure});if err!=nil{{var status *apiStatusError;if errors.As(err,&status)&&status.status==404{{resp.State.RemoveResource(ctx);return}};resp.Diagnostics.AddError(\"Delete failed\",err.Error());return}};resp.State.RemoveResource(ctx)}}",
        quoted(&plan.delete.path),
        quoted(plan.delete.method.as_str())
    );
    if !plan
        .attributes
        .iter()
        .any(|attribute| attribute.name != "id")
    {
        output = output.replace(" \"bytes\"\n", "");
    }
    output = crate::composite_render::apply(output, plan, &ty, false);
    output = crate::polling_render::apply(output, plan, &ty);
    if !plan.state_upgrades.is_empty() {
        output = output.replace(" \"context\"\n", " \"context\"\n \"github.com/hashicorp/terraform-plugin-go/tftypes\"\n \"github.com/hashicorp/terraform-plugin-go/tfprotov6\"\n");
        output.push_str(&crate::migration_render::source(plan, &ty));
    }
    output
}

#[cfg(test)]
#[path = "composite_lifecycle.rs"]
mod composite_lifecycle;
#[cfg(test)]
#[path = "native_lifecycle.rs"]
mod native_lifecycle;
#[cfg(test)]
#[path = "nested_lifecycle.rs"]
mod nested_lifecycle;
#[cfg(test)]
#[path = "polling_lifecycle.rs"]
mod polling_lifecycle;

const TRANSPORT_TEST: &str = r#"package provider
import("context";"fmt";"io";"net/http";"net/url";"strings";"testing")
type testExecutor func(*http.Request)(*http.Response,error)
func(execute testExecutor)Do(request *http.Request)(*http.Response,error){return execute(request)}
func TestTransportPreservesCancellationAndRedactsFailures(t *testing.T){
 ctx,cancel:=context.WithCancel(context.Background());defer cancel()
 client:=&apiClient{baseURL:"https://example.test",httpClient:testExecutor(func(request *http.Request)(*http.Response,error){
  if request.Context()!=ctx{t.Fatal("Terraform context was lost")}
  return nil,&url.Error{Op:"GET",URL:"https://example.test/?credential=secret",Err:fmt.Errorf("secret")}
 })}
 _,err:=client.call(ctx,"GET","/test",nil,false);if err==nil||strings.Contains(err.Error(),"secret"){t.Fatal("transport failure exposed credentials",err)}
 client.httpClient=testExecutor(func(*http.Request)(*http.Response,error){return &http.Response{StatusCode:401,Body:io.NopCloser(strings.NewReader("secret response"))},nil})
 _,err=client.call(ctx,"GET","/test",nil,false);if err==nil||strings.Contains(err.Error(),"secret"){t.Fatal("HTTP failure exposed body",err)}
}
func TestIdentityPathEscapesSegments(t *testing.T){for value,want:=range map[string]string{".":"%2E","..":"%2E%2E","a/b":"a%2Fb"}{if got:=identityPath(value);got!=want{t.Fatalf("%s => %s, expected %s",value,got,want)}}}
"#;

fn transport_test(catalog: &EntityCatalog) -> String {
    let check=match &catalog.authentication {
        AuthenticationPlan::None=>"if request.Header.Get(\"Authorization\")!=\"\"{t.Fatal(\"public provider sent authentication\")}".into(),
        AuthenticationPlan::Bearer=>"if request.Header.Get(\"Authorization\")!=\"Bearer secret\"{t.Fatal(\"bearer authentication missing\")}".into(),
        AuthenticationPlan::Basic=>"username,password,ok:=request.BasicAuth();if !ok||username!=\"login\"||password!=\"pass\"{t.Fatal(\"basic authentication missing\")}".into(),
        AuthenticationPlan::ApiKey{name,location}=>match location.as_str(){
            "header"=>format!("if request.Header.Get({})!=\"secret\"{{t.Fatal(\"API-key header missing\")}}",quoted(name)),
            "query"=>format!("if request.URL.Query().Get({})!=\"secret\"{{t.Fatal(\"API-key query missing\")}}",quoted(name)),
            "cookie"=>format!("cookie,err:=request.Cookie({});if err!=nil||cookie.Value!=\"secret\"{{t.Fatal(\"API-key cookie missing\")}}",quoted(name)),
            _=>unreachable!(),
        },
    };
    format!(
        r#"{TRANSPORT_TEST}
func TestDeclaredCredentialsApplyOnlyToSecuredOperations(t *testing.T){{
 client:=&apiClient{{baseURL:"https://example.test",token:"secret",username:"login",password:"pass"}}
 client.httpClient=testExecutor(func(request *http.Request)(*http.Response,error){{{check};return &http.Response{{StatusCode:204,Body:io.NopCloser(strings.NewReader(""))}},nil}})
 if _,err:=client.call(context.Background(),"GET","/test",nil,true);err!=nil{{t.Fatal(err)}}
 client.httpClient=testExecutor(func(request *http.Request)(*http.Response,error){{for _,values:=range request.Header{{for _,value:=range values{{if strings.Contains(value,"secret")||strings.Contains(value,"Basic "){{t.Fatal("public operation received credentials")}}}}}};if request.URL.RawQuery!=""{{t.Fatal("public operation received query credential")}};return &http.Response{{StatusCode:204,Body:io.NopCloser(strings.NewReader(""))}},nil}})
 if _,err:=client.call(context.Background(),"GET","/test",nil,false);err!=nil{{t.Fatal(err)}}
}}
"#
    )
}

/// Opt-in data sources reuse validated response hydration without resource mutation.
pub(crate) fn add_data_sources(
    tree: &mut GeneratedTree,
    catalog: &EntityCatalog,
    provider: &str,
) -> Result<()> {
    let factories = catalog
        .resources
        .iter()
        .map(|plan| format!("new{}DataSource", field(&plan.name)))
        .collect::<Vec<_>>()
        .join(", ");
    let path = "internal/provider/provider.go";
    let source = tree.get(path).expect("provider exists").replace("DataSources(_ context.Context)[]func()datasource.DataSource{return nil}", &format!("DataSources(_ context.Context)[]func()datasource.DataSource{{return []func()datasource.DataSource{{{factories}}}}}"));
    tree.replace(GeneratedFile::new(path, source)?)?;
    for plan in &catalog.resources {
        tree.insert(GeneratedFile::new(
            format!("internal/provider/data_source_{}.go", plan.name),
            data_source(plan),
        )?)?;
    }
    let mut readme = tree.get("README.md").unwrap().replace(
        "data sources, asynchronous",
        "independent/list data sources, asynchronous",
    );
    readme.push_str("\n## Read-only data sources\n\nOpt-in data sources reuse each validated resource GET operation. Supply a required string `id`; scalar response attributes are computed. Missing objects produce diagnostics rather than removing a managed resource. Lists, nested objects and independent read-only endpoint inference are unsupported.\n");
    for plan in &catalog.resources {
        let _ = writeln!(
            readme,
            "\n```hcl\ndata \"{provider}_{}\" \"existing\" {{\n  id = \"remote-object-id\"\n}}\n```",
            plan.name
        );
    }
    tree.replace(GeneratedFile::new("README.md", readme)?)?;
    Ok(())
}
fn data_source(plan: &ResourcePlan) -> String {
    let name = field(&plan.name);
    let ty = format!("{name}DataSource");
    let model = format!("{name}ResourceModel");
    let mut output = format!(
        r#"package provider
import("context";"strings";"github.com/hashicorp/terraform-plugin-framework/attr";"github.com/hashicorp/terraform-plugin-framework/types";"github.com/hashicorp/terraform-plugin-framework/datasource";"github.com/hashicorp/terraform-plugin-framework/datasource/schema")
var _ = attr.Type(types.StringType)
var _ datasource.DataSourceWithConfigure = &{ty}{{}}
type {ty} struct{{client *apiClient}}
func new{ty}() datasource.DataSource{{return &{ty}{{}}}}
func(d *{ty})Metadata(_ context.Context,req datasource.MetadataRequest,resp *datasource.MetadataResponse){{resp.TypeName=req.ProviderTypeName+{suffix}}}
func(d *{ty})Schema(_ context.Context,_ datasource.SchemaRequest,resp *datasource.SchemaResponse){{resp.Schema=schema.Schema{{Attributes:map[string]schema.Attribute{{"id":schema.StringAttribute{{Required:true}},
"#,
        suffix = quoted(&format!("_{}", plan.name))
    );
    for attribute in &plan.attributes {
        if attribute.name != "id" {
            if let Some(shape) = &attribute.shape {
                let _ = writeln!(
                    output,
                    "{}:{},",
                    quoted(&attribute.name),
                    crate::nested_render::schema(
                        shape,
                        false,
                        false,
                        true,
                        attribute.sensitive,
                        false
                    )
                );
                continue;
            }

            let _ = writeln!(
                output,
                "{}:schema.{}Attribute{{Computed:true,Sensitive:{}}},",
                quoted(&attribute.name),
                native(attribute.ty),
                attribute.sensitive
            );
        }
    }
    let _ = writeln!(
        output,
        r#"}}}}}}
func(d *{ty})Configure(_ context.Context,req datasource.ConfigureRequest,resp *datasource.ConfigureResponse){{if req.ProviderData==nil{{return}};client,ok:=req.ProviderData.(*apiClient);if !ok{{resp.Diagnostics.AddError("Unexpected provider data","Provider client type mismatch.");return}};d.client=client}}
func(d *{ty})Read(ctx context.Context,req datasource.ReadRequest,resp *datasource.ReadResponse){{var data {model};resp.Diagnostics.Append(req.Config.Get(ctx,&data)...);if resp.Diagnostics.HasError(){{return}};if d.client==nil{{resp.Diagnostics.AddError("Unconfigured provider","Configure the provider before reading.");return}};if data.ID.IsUnknown()||data.ID.IsNull()||data.ID.ValueString()==""{{resp.Diagnostics.AddError("Invalid identity","A known nonempty string ID is required.");return}};expected:=data.ID.ValueString();route:=strings.Replace({route},{placeholder},identityPath(expected),1);body,err:=d.client.call(ctx,"GET",route,nil,{secure});if err!=nil{{resp.Diagnostics.AddError("Data source read failed",err.Error());return}};id,err:={name}ResourceIdentity(body);if err!=nil||id!=expected{{resp.Diagnostics.AddError("Invalid response identity","Read response identity must match the requested ID.");return}};if err:=data.hydrate(body,true,true);err!=nil{{resp.Diagnostics.AddError("Invalid read response",err.Error());return}};resp.Diagnostics.Append(resp.State.Set(ctx,&data)... )}}
"#,
        route = quoted(&plan.read.path),
        placeholder = quoted(&format!("{{{}}}", plan.id_parameter)),
        secure = plan.requires_auth
    );
    crate::composite_render::apply(
        output,
        plan,
        &format!("{}Resource", field(&plan.name)),
        true,
    )
}
