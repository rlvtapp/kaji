//! Typed composition of first-party and community SDK plugins.

use anyhow::{Result, bail};
use kaji_core::engine::{Language, Packages};
use kaji_core::{AdaptedApi, Adapter, Api, GeneratedFile, GeneratedTree, SecuritySchemeCatalog};
use std::path::Path;

pub mod mock;
pub use kaji_core::SdkClientStyle;
pub use kaji_core::engine::{Common, Enforce, Package, PluginPhase};
pub use kaji_plugin_dotnet as dotnet;
pub use kaji_plugin_elixir as elixir;
pub use kaji_plugin_go as go;
pub use kaji_plugin_java as java;
pub use kaji_plugin_php as php;
pub use kaji_plugin_python as python;
pub use kaji_plugin_rust as rust;
pub use kaji_plugin_typescript as ts;

pub mod prelude {
    pub use crate::{Common, Package, ProfileSet};
    pub use kaji_core::engine::{
        Contract, Enforce, Handle, Language, Meta, Plugin, PluginContext, PluginPhase, Provision,
        Requirement,
    };
    pub use kaji_core::{GeneratedFile, SdkClientStyle};
    pub use kaji_plugin_dotnet::PackageExt as _;
    pub use kaji_plugin_elixir::PackageExt as _;
    pub use kaji_plugin_go::PackageExt as _;
    pub use kaji_plugin_java::PackageExt as _;
    pub use kaji_plugin_php::PackageExt as _;
    pub use kaji_plugin_python::PackageExt as _;
    pub use kaji_plugin_rust::PackageExt as _;
    pub use kaji_plugin_typescript::PackageExt as _;
}

/// One release containing independently configured, typed packages.
pub struct ProfileSet {
    root: String,
    packages: Packages,
}
impl ProfileSet {
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            packages: Packages::new(),
        }
    }
    pub fn common(mut self, common: Common) -> Self {
        self.packages = self.packages.common(common);
        self
    }
    pub fn package<L: Language>(mut self, package: Package<L>) -> Self {
        self.packages = self.packages.package(package);
        self
    }
}

pub fn generate(api: &Api, profiles: ProfileSet) -> Result<GeneratedTree> {
    generate_with_security_catalog(api, profiles, None)
}

pub fn generate_with_security_catalog(
    api: &Api,
    profiles: ProfileSet,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    if profiles.packages.is_empty() {
        bail!("add at least one package to the release");
    }
    GeneratedFile::new(&profiles.root, "")?;
    let generated = profiles.packages.generate(api, security_schemes)?;
    let mut tree = GeneratedTree::default();
    for (file, custom) in generated.into_files() {
        let file = GeneratedFile::new(Path::new(&profiles.root).join(file.path), file.contents)?;
        if custom {
            tree.insert_custom(file)?;
        } else {
            tree.insert(file)?;
        }
    }
    Ok(tree)
}

/// Generates packages from any source-format [`Adapter`].
///
/// The adapter owns parsing and normalization; Kaji's language plugins only
/// see its target-neutral [`Api`] and named security definitions. This makes
/// custom input formats possible without a fork of the generator.
pub fn generate_with_adapter(adapter: &dyn Adapter, profiles: ProfileSet) -> Result<GeneratedTree> {
    let AdaptedApi {
        api,
        security_schemes,
    } = adapter.adapt()?;
    generate_with_security_catalog(&api, profiles, Some(&security_schemes))
}

/// Generates packages from the artifacts of Kaji's bundled OpenAPI compiler.
pub fn generate_openapi(
    compiler_output: &Path,
    name: impl Into<String>,
    version: impl Into<String>,
    profiles: ProfileSet,
) -> Result<GeneratedTree> {
    let adapter = kaji_core::adapter::OpenApiSidecar::new(compiler_output, name, version);
    generate_with_adapter(&adapter, profiles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::adapter::AdaptedApi;

    struct EmptyAdapter;

    impl Adapter for EmptyAdapter {
        fn adapt(&self) -> Result<AdaptedApi> {
            Ok(AdaptedApi::new(
                Api {
                    name: "Example".into(),
                    version: "1.0.0".into(),
                    ..Api::default()
                },
                SecuritySchemeCatalog::default(),
            ))
        }
    }

    #[test]
    fn validates_release_root_and_empty_packages() {
        let api = Api::default();
        assert!(generate(&api, ProfileSet::new("sdk")).is_err());
        assert!(
            generate(
                &api,
                ProfileSet::new("../escape").package(go::package("go").with(go::sdk()))
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_custom_input_adapters() {
        let tree = generate_with_adapter(
            &EmptyAdapter,
            ProfileSet::new("sdk").package(go::package("go").with(go::sdk())),
        )
        .unwrap();
        assert!(tree.get("sdk/go/go.mod").is_some());
    }
}
