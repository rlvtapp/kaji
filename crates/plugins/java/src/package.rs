//! Typed package integration for the existing complete Java generator.
use anyhow::Result;
use kaji_core::SdkClientStyle;
use kaji_core::engine::{
    Contract, Handle, Language, Meta, Package, Plugin, PluginContext, Provision,
};

pub struct Java;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Java {
    const NAME: &'static str = "java";
    type Settings = Settings;
    type Workspace = ();
    fn bundle_middleware(
        tree: &mut kaji_core::GeneratedTree,
        middleware: &[kaji_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Java> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Java> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
/// Native public SDK identity consumed by optional generated-operation tests.
pub struct NativeSdk {
    pub namespace: String,
}
impl Contract for NativeSdk {
    const NAME: &'static str = "native-sdk";
}
pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    open_enums: bool,
    preserve_presence: bool,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
        open_enums: false,
        preserve_presence: false,
    }
}
impl Sdk {
    pub fn preserve_presence(mut self, enabled: bool) -> Self {
        self.preserve_presence = enabled;
        self
    }
    /// Preserve future wire enum values using extensible value classes.
    /// Known constants remain available; Java enum switches require the default policy.
    pub fn open_enums(mut self, enabled: bool) -> Self {
        self.open_enums = enabled;
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
impl Sdk {
    pub fn contract(&self) -> Handle<NativeSdk> {
        self.meta.handle()
    }
}
impl Plugin<Java> for Sdk {
    fn kind(&self) -> &'static str {
        "java-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<NativeSdk>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        cx.files.append(crate::presence::render(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            self.client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced),
            self.open_enums,
            self.preserve_presence,
        )?)?;
        cx.publish(NativeSdk {
            namespace: crate::java_package_name(
                cx.settings
                    .package_name
                    .as_deref()
                    .unwrap_or(&format!("io.kaji.{}", crate::package_segment(&cx.api.name))),
            ),
        })
    }
}
