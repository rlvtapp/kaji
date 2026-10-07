//! Typed package integration for the existing complete Python generator.
use anyhow::Result;
use kaji_core::SdkClientStyle;
use kaji_core::engine::{
    Contract, Handle, Language, Meta, Package, Plugin, PluginContext, Provision, Requirement,
};

pub struct Python;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
    pub open_enums: bool,
}
impl Language for Python {
    const NAME: &'static str = "python";
    type Settings = Settings;
    type Workspace = ();
    fn bundle_middleware(
        tree: &mut kaji_core::GeneratedTree,
        middleware: &[kaji_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Python> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
    fn open_enums(self, enabled: bool) -> Self;
}
impl PackageExt for Package<Python> {
    fn open_enums(mut self, enabled: bool) -> Self {
        self.settings_mut().open_enums = enabled;
        self
    }
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    async_client: bool,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
        async_client: false,
    }
}
impl Sdk {
    pub fn models(&self) -> Handle<PythonModels> {
        self.meta.handle()
    }
    /// Generate native async methods and an optional httpx dependency extra.
    pub fn async_client(mut self, enabled: bool) -> Self {
        self.async_client = enabled;
        self
    }
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
}
impl Plugin<Python> for Sdk {
    fn kind(&self) -> &'static str {
        "python-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<PythonModels>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
        let mut api = cx.api.clone();
        if cx.settings.open_enums {
            for schema in &mut api.schemas {
                schema
                    .value
                    .extensions
                    .insert("x-kaji-open-enum".into(), serde_json::json!(true));
            }
        }
        cx.files.append(crate::render_sdk_with_async(
            &api,
            ".",
            cx.settings.package_name.as_deref(),
            self.client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced),
            self.async_client,
        )?)?;
        let distribution = cx
            .settings
            .package_name
            .clone()
            .unwrap_or_else(|| format!("{}-sdk", crate::kebab_case(&cx.api.name)));
        cx.publish(PythonModels {
            module: crate::python_module_name(&distribution),
            object_models: cx
                .api
                .schemas
                .iter()
                .filter(|schema| matches!(schema.value.kind, kaji_core::SchemaKind::Object { .. }))
                .map(|schema| (schema.name.clone(), crate::python_type_name(&schema.name)))
                .collect(),
        })
    }
}

/// Independent Standard Webhooks HMAC verification consumer.
pub struct Webhooks {
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks { meta: Meta::new() }
}
impl Plugin<Python> for Webhooks {
    fn kind(&self) -> &'static str {
        "python-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
        let distribution = cx
            .settings
            .package_name
            .clone()
            .unwrap_or_else(|| format!("{}-sdk", crate::kebab_case(&cx.api.name)));
        let module = crate::python_module_name(&distribution);
        cx.files.emit(kaji_core::GeneratedFile::new(
            format!("src/{module}/webhooks.py"),
            include_str!("webhooks.py"),
        )?)
    }
}

/// Actual import symbols published by the Python SDK model provider.
#[derive(Clone, Debug)]
pub struct PythonModels {
    pub module: String,
    pub object_models: std::collections::BTreeMap<String, String>,
}
impl Contract for PythonModels {
    const NAME: &'static str = "python-models";
}

pub struct Roundtrips {
    meta: Meta,
    provider: Option<Handle<PythonModels>>,
    options: kaji_core::samples::SampleOptions,
}
pub fn roundtrips() -> Roundtrips {
    Roundtrips {
        meta: Meta::new(),
        provider: None,
        options: Default::default(),
    }
}
impl Roundtrips {
    pub fn models_from(mut self, sdk: &Sdk) -> Self {
        self.provider = Some(sdk.models());
        self
    }
    pub fn sample_options(mut self, options: kaji_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
}
impl Plugin<Python> for Roundtrips {
    fn kind(&self) -> &'static str {
        "python-roundtrips"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Python>) -> Result<()> {
        let models = cx.inputs.get::<PythonModels>()?;
        let mut cases = Vec::new();
        let mut diagnostics = Vec::new();
        for schema in &cx.api.schemas {
            if let Some(symbol) = models.object_models.get(&schema.name) {
                let report =
                    kaji_core::samples::schema_samples(cx.api, &schema.value, self.options);
                for sample in report.samples {
                    cases.push(
                        serde_json::json!({"model":symbol,"name":sample.name,"wire":sample.value}),
                    );
                }
                for diagnostic in report.diagnostics {
                    diagnostics.push(format!("{}: {}", schema.name, diagnostic));
                }
            }
        }
        let fixtures = serde_json::to_string_pretty(
            &serde_json::json!({"cases":cases,"diagnostics":diagnostics}),
        )?;
        cx.files.emit(kaji_core::GeneratedFile::new(
            "tests/roundtrip-fixtures.json",
            fixtures,
        )?)?;
        let script = format!(
            "# Generated by Kaji. Do not edit.\nimport json\nfrom pathlib import Path\nimport unittest\nfrom {} import models\n\nclass ModelRoundtrips(unittest.TestCase):\n    def test_wire_roundtrips(self):\n        fixtures = json.loads(Path(__file__).with_name('roundtrip-fixtures.json').read_text())\n        for case in fixtures['cases']:\n            with self.subTest(model=case['model'], sample=case['name']):\n                model = getattr(models, case['model']).from_dict(case['wire'])\n                self.assertEqual(models._to_wire(model), case['wire'])\n\nif __name__ == '__main__':\n    unittest.main()\n",
            models.module
        );
        cx.files.emit(kaji_core::GeneratedFile::new(
            "tests/test_model_roundtrips.py",
            script,
        )?)
    }
}

#[path = "operation_tests.rs"]
mod operation_tests;
pub use operation_tests::{OperationTests, operation_tests};
