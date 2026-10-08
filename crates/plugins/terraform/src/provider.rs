use crate::{
    Terraform,
    plan::{EntityCatalog, ResourceBinding, analyze_with_security, attribute_name},
};
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
};

pub struct Entities {
    meta: Meta,
    infer: bool,
    resources: Vec<ResourceBinding>,
}
pub fn entities() -> Entities {
    Entities {
        meta: Meta::new(),
        infer: true,
        resources: Vec::new(),
    }
}
impl Entities {
    pub fn infer(mut self, value: bool) -> Self {
        self.infer = value;
        self
    }
    pub fn resource(mut self, value: ResourceBinding) -> Self {
        self.resources.push(value);
        self
    }
    pub fn catalog_handle(&self) -> Handle<EntityCatalog> {
        self.meta.handle()
    }
}
impl Plugin<Terraform> for Entities {
    fn kind(&self) -> &'static str {
        "terraform-entities-v1"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EntityCatalog>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Terraform>) -> Result<()> {
        cx.publish(analyze_with_security(
            cx.api,
            &self.resources,
            self.infer,
            cx.security_schemes,
        )?)
    }
}
pub struct Provider {
    meta: Meta,
    data_sources: bool,
    infer: bool,
    resources: Vec<ResourceBinding>,
    catalog: Option<Handle<EntityCatalog>>,
}
pub fn provider() -> Provider {
    Provider {
        meta: Meta::new(),
        data_sources: false,
        infer: true,
        resources: Vec::new(),
        catalog: None,
    }
}
impl Provider {
    /// Emit read-only single-identity data sources from validated resource read plans.
    pub fn data_sources(mut self, enabled: bool) -> Self {
        self.data_sources = enabled;
        self
    }
    pub fn infer(mut self, value: bool) -> Self {
        self.infer = value;
        self
    }
    pub fn resource(mut self, value: ResourceBinding) -> Self {
        self.resources.push(value);
        self
    }
    pub fn using_entities(mut self, value: Handle<EntityCatalog>) -> Self {
        self.catalog = Some(value);
        self
    }
    pub fn catalog_handle(&self) -> Handle<EntityCatalog> {
        self.meta.handle()
    }
}
impl Plugin<Terraform> for Provider {
    fn kind(&self) -> &'static str {
        "terraform-provider-v1"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        if self.catalog.is_none() {
            vec![Provision::of::<EntityCatalog>()]
        } else {
            vec![]
        }
    }
    fn requires(&self) -> Vec<Requirement> {
        if self.catalog.is_some() {
            vec![Requirement::on(self.catalog)]
        } else {
            vec![]
        }
    }
    fn generate(&self, cx: &mut PluginContext<'_, Terraform>) -> Result<()> {
        ensure!(
            self.catalog.is_none() || (self.resources.is_empty() && self.infer),
            "using_entities cannot be combined with explicit provider resource/inference overrides"
        );
        let catalog = if self.catalog.is_some() {
            cx.inputs.get::<EntityCatalog>()?.clone()
        } else {
            analyze_with_security(cx.api, &self.resources, self.infer, cx.security_schemes)?
        };
        ensure!(
            !catalog.resources.is_empty(),
            "No supported Terraform resources: {}",
            catalog
                .diagnostics
                .iter()
                .map(|d| format!("{}: {}", d.operation, d.reason))
                .collect::<Vec<_>>()
                .join("; ")
        );
        let inferred_name = crate::slug(&cx.api.name).replace('-', "_");
        let inferred_name = if inferred_name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            format!("api_{inferred_name}")
        } else {
            inferred_name
        };
        let provider = attribute_name(
            cx.settings
                .provider_name
                .as_deref()
                .unwrap_or(&inferred_name),
        )?;
        let namespace = cx
            .settings
            .registry_namespace
            .as_deref()
            .unwrap_or("poolster");
        ensure!(
            namespace
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !namespace.is_empty(),
            "registry_namespace must contain lowercase letters, digits or hyphens"
        );
        let module = cx
            .settings
            .module
            .clone()
            .unwrap_or_else(|| format!("terraform-provider-{provider}"));
        ensure!(
            !module.is_empty()
                && module.split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))),
            "Terraform module must be a safe Go module path"
        );
        let mut tree = crate::typed_render::render(cx.api, &catalog, &module, &provider)?;
        for path in ["main.go", "README.md"] {
            let contents = tree.get(path).unwrap().replace(
                "registry.terraform.io/poolster/",
                &format!("registry.terraform.io/{namespace}/"),
            );
            let contents = if path == "README.md" {
                contents.replace(
                    &format!("source = \"poolster/{provider}\""),
                    &format!("source = \"{namespace}/{provider}\""),
                )
            } else {
                contents
            };
            tree.replace(GeneratedFile::new(path, contents)?)?;
        }
        if self.data_sources {
            crate::typed_render::add_data_sources(&mut tree, &catalog, &provider)?;
        }
        tree.insert(GeneratedFile::new(
            ".poolster/terraform-plan.json",
            serde_json::to_string_pretty(&catalog.explanation())?,
        )?)?;
        cx.files.append(tree)?;
        if self.catalog.is_none() {
            cx.publish(catalog)?;
        }
        Ok(())
    }
}
