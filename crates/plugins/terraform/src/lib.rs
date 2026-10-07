//! Terraform Plugin Framework provider generation.
//!
//! `provider()` consumes a validated typed entity catalog, inferred conservatively
//! from conventional CRUD paths or supplied explicit bindings. The initial v1
//! supports root scalar objects and a single string identity. `entities()` exposes
//! the same catalog independently for custom consumers and renderers.
//!
//! `sdk().resource(...)` remains the legacy raw JSON-body prototype; its state
//! representation is not migrated implicitly to the typed provider.

use anyhow::{Result, bail};
use kaji_core::{Api, GeneratedFile, GeneratedTree, HttpMethod, Operation};

pub mod plan;
mod provider;
mod release;
mod typed_render;
pub use plan::{
    AttributePlan, AuthenticationPlan, EntityCatalog, PlanDiagnostic, ResourceBinding,
    ResourcePlan, ScalarType, analyze, analyze_with_security,
};
pub use provider::{Entities, Provider, entities, provider};
pub use release::{ReleaseScaffold, release_scaffold};
mod package;
pub use package::{PackageExt, Sdk, Settings, Terraform, TerraformResource, package, sdk};

fn render_provider(
    api: &Api,
    output_dir: &str,
    module: Option<&str>,
    provider_name: Option<&str>,
    resources: &[TerraformResource],
) -> Result<GeneratedTree> {
    if resources.is_empty() {
        bail!("Terraform provider generation requires at least one mapped resource");
    }
    let provider = provider_name
        .filter(|v| !v.trim().is_empty())
        .map(slug)
        .unwrap_or_else(|| slug(&api.name));
    let module = module
        .filter(|v| !v.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("terraform-provider-{provider}"));
    let root = output_dir.trim_matches('/');
    let mut resolved = Vec::with_capacity(resources.len());
    for resource in resources {
        resolved.push(resolve_resource(api, resource)?);
    }
    let mut tree = GeneratedTree::default();
    insert(&mut tree, root, "go.mod", go_mod(&module))?;
    insert(&mut tree, root, "main.go", main_go(&module, &provider))?;
    insert(
        &mut tree,
        root,
        "internal/provider/provider.go",
        provider_go(&provider, &resolved),
    )?;
    for resource in &resolved {
        insert(
            &mut tree,
            root,
            &format!("internal/provider/resource_{}.go", slug(&resource.name)),
            resource_go(resource),
        )?;
    }
    insert(
        &mut tree,
        root,
        "README.md",
        readme(api, &provider, resources),
    )?;
    Ok(tree)
}

#[derive(Clone)]
struct ResolvedResource {
    name: String,
    create: Operation,
    read: Operation,
    update: Operation,
    delete: Operation,
    id_parameter: String,
}

fn resolve_resource(api: &Api, resource: &TerraformResource) -> Result<ResolvedResource> {
    let find = |id: &str| {
        api.operations
            .iter()
            .find(|operation| operation.id == id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Terraform resource {:?} references unknown operation {:?}",
                    resource.name,
                    id
                )
            })
    };
    let create = find(&resource.create)?;
    let read = find(&resource.read)?;
    let update = find(&resource.update)?;
    let delete = find(&resource.delete)?;
    if !matches!(create.method, HttpMethod::Post) {
        bail!(
            "Terraform resource {:?} create operation must use POST",
            resource.name
        );
    }
    if !matches!(read.method, HttpMethod::Get) {
        bail!(
            "Terraform resource {:?} read operation must use GET",
            resource.name
        );
    }
    if !matches!(update.method, HttpMethod::Put | HttpMethod::Patch) {
        bail!(
            "Terraform resource {:?} update operation must use PUT or PATCH",
            resource.name
        );
    }
    if !matches!(delete.method, HttpMethod::Delete) {
        bail!(
            "Terraform resource {:?} delete operation must use DELETE",
            resource.name
        );
    }
    let id_parameter = resource.id_parameter.clone().unwrap_or_else(|| "id".into());
    for operation in [&read, &update, &delete] {
        if !operation.path.contains(&format!("{{{id_parameter}}}")) {
            bail!(
                "Terraform resource {:?} operation {:?} must contain path parameter {{{id_parameter}}}",
                resource.name,
                operation.id
            );
        }
    }
    Ok(ResolvedResource {
        name: resource.name.clone(),
        create,
        read,
        update,
        delete,
        id_parameter,
    })
}

fn insert(tree: &mut GeneratedTree, root: &str, path: &str, contents: String) -> Result<()> {
    tree.insert(GeneratedFile::new(
        if root.is_empty() || root == "." {
            path.into()
        } else {
            format!("{root}/{path}")
        },
        contents,
    )?)
}
fn go_mod(module: &str) -> String {
    format!(
        "module {module}\n\ngo 1.22\n\nrequire github.com/hashicorp/terraform-plugin-framework v1.16.1\n"
    )
}
fn main_go(module: &str, provider: &str) -> String {
    format!(
        "package main\n\nimport (\n  \"context\"\n  \"log\"\n  \"github.com/hashicorp/terraform-plugin-framework/providerserver\"\n  \"{module}/internal/provider\"\n)\n\nvar version = \"dev\"\n\nfunc main() {{\n  err := providerserver.Serve(context.Background(), provider.New(version), providerserver.ServeOpts{{Address: \"registry.terraform.io/kaji/{provider}\"}})\n  if err != nil {{ log.Fatal(err) }}\n}}\n"
    )
}
fn provider_go(provider: &str, resources: &[ResolvedResource]) -> String {
    let constructors = resources
        .iter()
        .map(|r| format!("new{}Resource", pascal(&r.name)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "package provider\n\nimport (\n  \"context\"\n  \"fmt\"\n  \"io\"\n  \"net/http\"\n  \"strings\"\n  \"github.com/hashicorp/terraform-plugin-framework/datasource\"\n  \"github.com/hashicorp/terraform-plugin-framework/provider\"\n  \"github.com/hashicorp/terraform-plugin-framework/provider/schema\"\n  \"github.com/hashicorp/terraform-plugin-framework/resource\"\n  \"github.com/hashicorp/terraform-plugin-framework/types\"\n)\n\nvar _ provider.Provider = &generatedProvider{{}}\n\ntype generatedProvider struct {{ version string }}\ntype providerModel struct {{ BaseURL types.String `tfsdk:\"base_url\"`; APIKey types.String `tfsdk:\"api_key\"` }}\n\nfunc New(version string) func() provider.Provider {{ return func() provider.Provider {{ return &generatedProvider{{version: version}} }} }}\nfunc (p *generatedProvider) Metadata(_ context.Context, _ provider.MetadataRequest, resp *provider.MetadataResponse) {{ resp.TypeName = \"{provider}\"; resp.Version = p.version }}\nfunc (p *generatedProvider) Schema(_ context.Context, _ provider.SchemaRequest, resp *provider.SchemaResponse) {{ resp.Schema = schema.Schema{{Attributes: map[string]schema.Attribute{{\n  \"base_url\": schema.StringAttribute{{Optional: true, Description: \"API base URL.\"}},\n  \"api_key\": schema.StringAttribute{{Optional: true, Sensitive: true, Description: \"API key.\"}},\n}}}} }}\nfunc (p *generatedProvider) Configure(ctx context.Context, req provider.ConfigureRequest, resp *provider.ConfigureResponse) {{\n var data providerModel; diagnostics := req.Config.Get(ctx, &data); resp.Diagnostics.Append(diagnostics...); if resp.Diagnostics.HasError() {{ return }}\n baseURL := data.BaseURL.ValueString(); if baseURL == \"\" {{ resp.Diagnostics.AddError(\"Missing API base URL\", \"Set the provider base_url argument.\"); return }}\n client := &apiClient{{baseURL: strings.TrimRight(baseURL, \"/\"), apiKey: data.APIKey.ValueString(), httpClient: http.DefaultClient}}\n resp.ResourceData = client\n}}\nfunc (p *generatedProvider) Resources(_ context.Context) []func() resource.Resource {{ return []func() resource.Resource{{{constructors}}} }}\nfunc (p *generatedProvider) DataSources(_ context.Context) []func() datasource.DataSource {{ return nil }}\n\ntype apiClient struct {{ baseURL string; apiKey string; httpClient *http.Client }}\nfunc (c *apiClient) call(ctx context.Context, method, path, body string) (string, error) {{ request, err := http.NewRequestWithContext(ctx, method, c.baseURL+path, strings.NewReader(body)); if err != nil {{ return \"\", err }}; request.Header.Set(\"Content-Type\", \"application/json\"); if c.apiKey != \"\" {{ request.Header.Set(\"Authorization\", \"Bearer \"+c.apiKey) }}; response, err := c.httpClient.Do(request); if err != nil {{ return \"\", err }}; defer response.Body.Close(); bytes, _ := io.ReadAll(response.Body); if response.StatusCode < 200 || response.StatusCode >= 300 {{ return \"\", fmt.Errorf(\"API returned %d: %s\", response.StatusCode, string(bytes)) }}; return string(bytes), nil }}\n"
    )
}

fn resource_go(resource: &ResolvedResource) -> String {
    let type_name = format!("{}Resource", pascal(&resource.name));
    let variable = camel(&resource.name);
    let template = format!("{{{}}}", resource.id_parameter);
    format!(
        "package provider\n\nimport (\n  \"context\"\n  \"encoding/json\"\n  \"net/url\"\n  \"strings\"\n  \"github.com/hashicorp/terraform-plugin-framework/diag\"\n  \"github.com/hashicorp/terraform-plugin-framework/resource\"\n  \"github.com/hashicorp/terraform-plugin-framework/resource/schema\"\n  \"github.com/hashicorp/terraform-plugin-framework/types\"\n)\n\nvar _ resource.Resource = &{variable}{{}}\ntype {variable} struct {{ client *apiClient }}\ntype {variable}Model struct {{ ID types.String `tfsdk:\"id\"`; Body types.String `tfsdk:\"body\"` }}\nfunc new{type_name}() resource.Resource {{ return &{variable}{{}} }}\nfunc (r *{variable}) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {{ resp.TypeName = req.ProviderTypeName + \"_{resource_name}\" }}\nfunc (r *{variable}) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {{ resp.Schema = schema.Schema{{Attributes: map[string]schema.Attribute{{\"id\": schema.StringAttribute{{Computed: true}}, \"body\": schema.StringAttribute{{Required: true, Description: \"JSON request document managed by this resource.\"}}}}}} }}\nfunc (r *{variable}) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {{ if req.ProviderData == nil {{ return }}; client, ok := req.ProviderData.(*apiClient); if !ok {{ resp.Diagnostics.AddError(\"Unexpected provider data\", \"Kaji provider internal configuration failed.\"); return }}; r.client = client }}\nfunc (r *{variable}) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {{ var plan {variable}Model; resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...); if resp.Diagnostics.HasError() {{ return }}; body, err := r.client.call(ctx, \"{create_method}\", \"{create_path}\", plan.Body.ValueString()); if err != nil {{ resp.Diagnostics.AddError(\"Create {resource_name}\", err.Error()); return }}; id, diagnostics := responseID(body); resp.Diagnostics.Append(diagnostics...); if resp.Diagnostics.HasError() {{ return }}; plan.ID = types.StringValue(id); plan.Body = types.StringValue(body); resp.Diagnostics.Append(resp.State.Set(ctx, &plan)... ) }}\nfunc (r *{variable}) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {{ var state {variable}Model; resp.Diagnostics.Append(req.State.Get(ctx, &state)...); if resp.Diagnostics.HasError() {{ return }}; body, err := r.client.call(ctx, \"{read_method}\", strings.Replace(\"{read_path}\", \"{template}\", url.PathEscape(state.ID.ValueString()), 1), \"\"); if err != nil {{ resp.Diagnostics.AddError(\"Read {resource_name}\", err.Error()); return }}; state.Body = types.StringValue(body); resp.Diagnostics.Append(resp.State.Set(ctx, &state)... ) }}\nfunc (r *{variable}) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {{ var plan {variable}Model; var state {variable}Model; resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...); resp.Diagnostics.Append(req.State.Get(ctx, &state)...); if resp.Diagnostics.HasError() {{ return }}; body, err := r.client.call(ctx, \"{update_method}\", strings.Replace(\"{update_path}\", \"{template}\", url.PathEscape(state.ID.ValueString()), 1), plan.Body.ValueString()); if err != nil {{ resp.Diagnostics.AddError(\"Update {resource_name}\", err.Error()); return }}; plan.ID = state.ID; plan.Body = types.StringValue(body); resp.Diagnostics.Append(resp.State.Set(ctx, &plan)... ) }}\nfunc (r *{variable}) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {{ var state {variable}Model; resp.Diagnostics.Append(req.State.Get(ctx, &state)...); if resp.Diagnostics.HasError() {{ return }}; _, err := r.client.call(ctx, \"{delete_method}\", strings.Replace(\"{delete_path}\", \"{template}\", url.PathEscape(state.ID.ValueString()), 1), \"\"); if err != nil {{ resp.Diagnostics.AddError(\"Delete {resource_name}\", err.Error()) }} }}\nfunc responseID(body string) (string, diag.Diagnostics) {{ var value map[string]any; if err := json.Unmarshal([]byte(body), &value); err != nil {{ return \"\", diag.Diagnostics{{diag.NewErrorDiagnostic(\"Invalid create response\", err.Error())}} }}; id, ok := value[\"id\"].(string); if !ok || id == \"\" {{ return \"\", diag.Diagnostics{{diag.NewErrorDiagnostic(\"Missing response id\", \"Create response must contain a non-empty string id field.\")}} }}; return id, nil }}\n",
        resource_name = resource.name,
        create_method = resource.create.method.as_str(),
        create_path = resource.create.path,
        read_method = resource.read.method.as_str(),
        read_path = resource.read.path,
        update_method = resource.update.method.as_str(),
        update_path = resource.update.path,
        delete_method = resource.delete.method.as_str(),
        delete_path = resource.delete.path
    )
}
fn readme(api: &Api, provider: &str, resources: &[TerraformResource]) -> String {
    let names = resources
        .iter()
        .map(|r| {
            format!(
                "- `{provider}_{}`: `{}` → `{}` → `{}` → `{}`\n",
                r.name, r.create, r.read, r.update, r.delete
            )
        })
        .collect::<String>();
    format!(
        "# {} Terraform provider\n\nGenerated from an explicit Kaji Terraform mapping. This provider exposes JSON-body resources: the `body` argument is a JSON request document and state is the normalized JSON response.\n\n## Resources\n\n{names}\n\nConfigure `base_url` (or the provider environment setting) before use. The mapping is intentionally explicit because OpenAPI cannot infer Terraform lifecycle or state semantics.\n",
        api.name
    )
}
fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = true;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            result.extend(c.to_lowercase());
            separator = false;
        } else if !separator && !result.is_empty() {
            result.push('-');
            separator = true;
        }
    }
    result.trim_matches('-').to_owned().if_empty("api")
}
fn pascal(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|v| !v.is_empty())
        .map(|v| {
            let mut chars = v.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}
fn camel(value: &str) -> String {
    let value = pascal(value);
    let mut chars = value.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().collect::<String>() + chars.as_str())
        .unwrap_or_else(|| "resource".into())
}
trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(id: &str, method: HttpMethod, path: &str) -> Operation {
        Operation {
            id: id.into(),
            method,
            path: path.into(),
            ..Operation::default()
        }
    }

    fn resource() -> TerraformResource {
        TerraformResource::new(
            "project",
            "createProject",
            "getProject",
            "updateProject",
            "deleteProject",
        )
        .id_parameter("projectId")
    }

    #[test]
    fn emits_a_provider_from_an_explicit_crud_mapping() {
        let api = Api {
            name: "Acme".into(),
            operations: vec![
                operation("createProject", HttpMethod::Post, "/projects"),
                operation("getProject", HttpMethod::Get, "/projects/{projectId}"),
                operation("updateProject", HttpMethod::Patch, "/projects/{projectId}"),
                operation("deleteProject", HttpMethod::Delete, "/projects/{projectId}"),
            ],
            ..Api::default()
        };
        let tree = render_provider(
            &api,
            "terraform",
            Some("github.com/acme/terraform-provider-acme"),
            Some("acme"),
            &[resource()],
        )
        .unwrap();
        assert!(
            tree.get("terraform/internal/provider/resource_project.go")
                .unwrap()
                .contains("req.ProviderTypeName + \"_project\"")
        );
        assert!(
            tree.get("terraform/main.go")
                .unwrap()
                .contains("registry.terraform.io/kaji/acme")
        );
    }

    #[test]
    fn rejects_a_non_get_read_mapping() {
        let api = Api {
            operations: vec![
                operation("createProject", HttpMethod::Post, "/projects"),
                operation("getProject", HttpMethod::Post, "/projects/{projectId}"),
                operation("updateProject", HttpMethod::Patch, "/projects/{projectId}"),
                operation("deleteProject", HttpMethod::Delete, "/projects/{projectId}"),
            ],
            ..Api::default()
        };
        assert!(render_provider(&api, ".", None, None, &[resource()]).is_err());
    }
}
