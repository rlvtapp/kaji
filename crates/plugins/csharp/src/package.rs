//! Typed package integration for Poolster's C# generator.
use anyhow::Result;
use poolster_core::SdkClientStyle;
use poolster_core::engine::{
    Contract, Handle, Language, Meta, Package, Plugin, PluginContext, Provision,
};

/// The canonical C#/.NET language target.
pub struct CSharp;
/// Legacy .NET target identity retained for Rust embedding compatibility.
///
/// Prefer [`CSharp`]. Both identities use the same renderer and settings; the
/// separate type preserves the legacy `Language::NAME` for existing profiles.
pub struct DotNet;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for CSharp {
    const NAME: &'static str = "csharp";
    type Settings = Settings;
    type Workspace = ();
    fn finalize_files(tree: &mut poolster_core::GeneratedTree) -> Result<()> {
        crate::operation_tests::finalize(tree)
    }
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
}
impl Language for DotNet {
    const NAME: &'static str = "dotnet";
    type Settings = Settings;
    type Workspace = ();
    fn finalize_files(tree: &mut poolster_core::GeneratedTree) -> Result<()> {
        crate::operation_tests::finalize(tree)
    }
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<CSharp> {
    Package::new(dir)
}
/// Creates a package using the legacy `dotnet` target identity.
pub fn dotnet_package(dir: impl Into<String>) -> Package<DotNet> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<CSharp> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
impl PackageExt for Package<DotNet> {
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
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    open_enums: bool,
    preserve_presence: bool,
}
pub fn sdk() -> Sdk {
    Sdk {
        http_input: Default::default(),
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
    /// Preserve unknown string enum wire values with extensible value records.
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
impl Plugin<CSharp> for Sdk {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "csharp-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<NativeSdk>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, CSharp>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
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
                namespace: crate::dotnet_namespace(
                    cx.settings
                        .package_name
                        .as_deref()
                        .unwrap_or(&format!("{}-sdk", crate::kebab_case(&cx.api.name))),
                ),
            })
        })
    }
}
impl Plugin<DotNet> for Sdk {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "dotnet-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<NativeSdk>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, DotNet>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
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
                namespace: crate::dotnet_namespace(
                    cx.settings
                        .package_name
                        .as_deref()
                        .unwrap_or(&format!("{}-sdk", crate::kebab_case(&cx.api.name))),
                ),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_csharp_as_the_canonical_language_name() {
        assert_eq!(CSharp::NAME, "csharp");
        assert_eq!(DotNet::NAME, "dotnet");
    }
}

#[path = "package_input.rs"]
mod http_input;
