use anyhow::Result;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Terraform;
#[derive(Default)]
pub struct Settings {
    pub module: Option<String>,
    pub provider_name: Option<String>,
    pub registry_namespace: Option<String>,
}
impl Language for Terraform {
    const NAME: &'static str = "terraform";
    type Settings = Settings;
    type Workspace = Vec<TerraformResource>;
}
pub fn package(dir: impl Into<String>) -> Package<Terraform> {
    Package::new(dir)
}
pub trait PackageExt {
    fn module(self, name: impl Into<String>) -> Self;
    fn provider_name(self, name: impl Into<String>) -> Self;
    fn registry_namespace(self, namespace: impl Into<String>) -> Self;
}
impl PackageExt for Package<Terraform> {
    fn registry_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.settings_mut().registry_namespace = Some(namespace.into());
        self
    }
    fn module(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().module = Some(name.into());
        self
    }
    fn provider_name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().provider_name = Some(name.into());
        self
    }
}
#[derive(Clone, Debug)]
pub struct TerraformResource {
    pub name: String,
    pub create: String,
    pub read: String,
    pub update: String,
    pub delete: String,
    pub id_parameter: Option<String>,
}
impl TerraformResource {
    pub fn new(
        name: impl Into<String>,
        create: impl Into<String>,
        read: impl Into<String>,
        update: impl Into<String>,
        delete: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            create: create.into(),
            read: read.into(),
            update: update.into(),
            delete: delete.into(),
            id_parameter: None,
        }
    }
    pub fn id_parameter(mut self, name: impl Into<String>) -> Self {
        self.id_parameter = Some(name.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
    pub(crate) http: poolster_core::engine::HttpInput,
    resources: Vec<TerraformResource>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        http: Default::default(),
        resources: Vec::new(),
    }
}
impl Sdk {
    pub fn resource(mut self, resource: TerraformResource) -> Self {
        self.resources.push(resource);
        self
    }
}
impl Plugin<Terraform> for Sdk {
    fn kind(&self) -> &'static str {
        "terraform-provider"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Terraform>) -> Result<()> {
        self.http.run(cx, |cx| {
            cx.files.append(crate::render_provider(
                cx.api,
                ".",
                cx.settings.module.as_deref(),
                cx.settings.provider_name.as_deref(),
                &self.resources,
            )?)
        })
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http.requirements()
    }

    fn supports_native_input(&self) -> bool {
        self.http.is_explicit()
    }
}

impl Sdk {
    /// Select the authoritative HTTP contract produced by an input or transform.
    pub fn input(
        mut self,
        input: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http = self.http.input(input);
        self
    }
    /// Consume complete model blocks from the selected HTTP contract revision.
    pub fn input_models(
        mut self,
        models: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http = self.http.input_models(models);
        self
    }
    /// Consume complete endpoint blocks from the selected HTTP contract revision.
    pub fn input_endpoints(
        mut self,
        endpoints: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http = self.http.input_endpoints(endpoints);
        self
    }
}
