//! Typed composition of first-party and community SDK plugins.

use anyhow::{Result, bail};
use kaji_core::engine::{Language, Packages};
use kaji_core::{AdaptedApi, Adapter, Api, GeneratedFile, GeneratedTree, SecuritySchemeCatalog};
use std::path::Path;

pub mod mock;
pub use kaji_core::SdkClientStyle;
pub use kaji_core::api_reference::{ApiReference, ApiReferenceDocument, api_reference};
pub use kaji_core::customization::{
    BundledMiddleware, CodeCustomization, apply_code_customizations,
};
pub use kaji_core::engine::{Common, Enforce, Package, PluginPhase};
pub use kaji_core::idempotency::{IdempotencyConfig, IdempotencyRule};
pub use kaji_core::input;
pub use kaji_core::release;
/// First-party C# SDK generator.
///
/// This is the preferred name for the .NET/C# target. [`dotnet`] remains an
/// alias so existing embedded generation profiles continue to compile.
pub use kaji_plugin_csharp as csharp;
pub use kaji_plugin_dotnet as dotnet;
pub use kaji_plugin_elixir as elixir;
pub use kaji_plugin_go as go;
pub use kaji_plugin_java as java;
pub use kaji_plugin_php as php;
pub use kaji_plugin_postman as postman;
pub use kaji_plugin_python as python;
pub use kaji_plugin_ruby as ruby;
pub use kaji_plugin_rust as rust;
pub use kaji_plugin_rust_cli as rust_cli;
pub use kaji_plugin_swift as swift;
pub use kaji_plugin_symfony as symfony;
pub use kaji_plugin_terraform as terraform;
pub use kaji_plugin_typescript as ts;
pub use kaji_plugin_typescript_cli as ts_cli;

pub mod prelude {
    pub use crate::{
        BundledMiddleware, CodeCustomization, Common, IdempotencyConfig, IdempotencyRule, Package,
        ProfileSet,
    };
    pub use kaji_core::engine::{
        Contract, Enforce, Handle, Language, Meta, Plugin, PluginContext, PluginPhase, Provision,
        Requirement,
    };
    pub use kaji_core::input::{InputContract, InputPlugin, InputProvider, InputRegistry};
    pub use kaji_core::{GeneratedFile, SdkClientStyle};
    pub use kaji_plugin_csharp::PackageExt as _;
    pub use kaji_plugin_elixir::PackageExt as _;
    pub use kaji_plugin_go::PackageExt as _;
    pub use kaji_plugin_java::PackageExt as _;
    pub use kaji_plugin_php::PackageExt as _;
    pub use kaji_plugin_postman::PackageExt as _;
    pub use kaji_plugin_python::PackageExt as _;
    pub use kaji_plugin_ruby::PackageExt as _;
    pub use kaji_plugin_rust::PackageExt as _;
    pub use kaji_plugin_rust_cli::PackageExt as _;
    pub use kaji_plugin_swift::PackageExt as _;
    pub use kaji_plugin_symfony::PackageExt as _;
    pub use kaji_plugin_terraform::PackageExt as _;
    pub use kaji_plugin_typescript::PackageExt as _;
    pub use kaji_plugin_typescript_cli::PackageExt as _;
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
    for (file, custom, owner) in generated.into_owned_files() {
        let file = GeneratedFile::new(Path::new(&profiles.root).join(file.path), file.contents)?;
        let path = file.path.clone();
        if custom {
            tree.insert_custom(file)?;
        } else {
            tree.insert(file)?;
        }
        if let Some(owner) = owner {
            tree.set_owner(path, owner)?;
        }
    }
    Ok(tree)
}

/// Generates HTTP SDK packages from the normalized API published by an input plugin.
/// Native GraphQL/event/RPC inputs require consumers for their own typed contracts;
/// they are never silently coerced into HTTP operations.
pub fn generate_with_input(
    input: &kaji_core::input::InputContract,
    profiles: ProfileSet,
) -> Result<GeneratedTree> {
    let adapted = input.get::<AdaptedApi>()?;
    generate_with_security_catalog(&adapted.api, profiles, Some(&adapted.security_schemes))
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
    #[test]
    fn registered_input_can_supply_normalized_api_to_existing_sdk_generators() {
        use kaji_core::input::{InputContract, InputPlugin, InputRegistry, InputSummary};
        struct JsonApiInput;
        impl InputPlugin for JsonApiInput {
            fn id(&self) -> &str {
                "http.json"
            }
            fn format(&self) -> &str {
                "http-json"
            }
            fn load(&self, path: &Path) -> Result<InputContract> {
                let api: Api = serde_json::from_slice(&std::fs::read(path)?)?;
                let mut input = InputContract::new(InputSummary {
                    format: "http-json".into(),
                    title: api.name.clone(),
                    version: Some(api.version.clone()),
                    types: vec![],
                    operations: vec![],
                });
                input.publish(AdaptedApi::new(api, SecuritySchemeCatalog::default()))?;
                Ok(input)
            }
        }
        let source = tempfile::NamedTempFile::new().unwrap();
        let api = Api {
            name: "Inventory".into(),
            version: "2.0.0".into(),
            schemas: vec![kaji_core::Schema::new(
                "InventoryItem",
                kaji_core::SchemaValue::new(kaji_core::SchemaKind::String),
            )],
            ..Default::default()
        };
        std::fs::write(source.path(), serde_json::to_vec(&api).unwrap()).unwrap();
        let mut registry = InputRegistry::new();
        registry.register(JsonApiInput).unwrap();
        let input = registry.load("http-json", None, source.path()).unwrap();
        let tree = generate_with_input(
            &input.contract,
            ProfileSet::new("sdk").package(go::package("go").with(go::sdk())),
        )
        .unwrap();
        assert!(
            tree.iter()
                .any(|(_, contents)| contents.contains("type InventoryItem"))
        );
        let native_only = InputContract::new(InputSummary {
            format: "graphql".into(),
            title: "Native".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        let error = generate_with_input(
            &native_only,
            ProfileSet::new("sdk").package(go::package("go").with(go::sdk())),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("kaji.http-api"));
    }
}
