//! Package implementation for generated typescript-cli packages.
use super::*;

pub struct TypeScriptCli;

#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}

impl Language for TypeScriptCli {
    const NAME: &'static str = "typescript-cli";
    type Settings = Settings;
    type Workspace = ();
}

pub fn package(dir: impl Into<String>) -> Package<TypeScriptCli> {
    Package::new(dir)
}

pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}

impl PackageExt for Package<TypeScriptCli> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
