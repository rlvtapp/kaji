//! Rust package settings and assembly.
use super::*;

pub struct Rust;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
    pub open_unions: bool,
    pub open_enums: bool,
}
impl Language for Rust {
    const NAME: &'static str = "rust";
    type Settings = Settings;
    type Workspace = composition::Workspace;
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled::bundle(tree, middleware)
    }
    fn finalize(cx: &mut poolster_core::engine::FinalizeContext<'_, Self>) -> Result<()> {
        composition::Workspace::finalize(cx)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Rust> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
    /// Retain unknown named union variants as JSON; default decoding stays strict.
    fn open_unions(self, enabled: bool) -> Self;
    /// Generate extensible typed string enums with known-value helpers.
    fn open_enums(self, enabled: bool) -> Self;
}
impl PackageExt for Package<Rust> {
    fn open_enums(mut self, enabled: bool) -> Self {
        self.settings_mut().open_enums = enabled;
        self
    }
    fn open_unions(mut self, enabled: bool) -> Self {
        self.settings_mut().open_unions = enabled;
        self
    }
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
