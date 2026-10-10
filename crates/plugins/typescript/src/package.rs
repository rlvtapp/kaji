//! TypeScript package settings and finalization.
use super::*;

pub struct TypeScript;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for TypeScript {
    const NAME: &'static str = "typescript";
    type Settings = Settings;
    type Workspace = Workspace;
    fn finalize(cx: &mut FinalizeContext<'_, Self>) -> Result<()> {
        workspace::finalize(cx)
    }
    fn finalize_files(tree: &mut poolster_core::GeneratedTree) -> Result<()> {
        esm::finalize(tree)
    }
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<TypeScript> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<TypeScript> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
