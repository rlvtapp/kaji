//! Package implementation for generated rust-cli packages.
use super::*;

pub struct RustCli;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for RustCli {
    const NAME: &'static str = "rust-cli";
    type Settings = Settings;
    type Workspace = ();
}
pub fn package(dir: impl Into<String>) -> Package<RustCli> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, value: impl Into<String>) -> Self;
}
impl PackageExt for Package<RustCli> {
    fn name(mut self, value: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(value.into());
        self
    }
}
