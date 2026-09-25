//! First-party SDK target presets.
//!
//! This crate is the stable composition layer above `kaji-core`.
//! Generator implementations stay in their own plugin crates (or, during the
//! initial migration, in `kaji-core::plugins`); profiles decide which
//! implementations form a publishable SDK package and where each package is
//! written.

use anyhow::{Result, bail};
use kaji_core::{
    Api, GeneratedFile, GeneratedTree, SdkClientStyle, SdkProfile, SdkSurface, SdkTransport,
    SecuritySchemeCatalog, generate_sdks_with_security_catalog,
};
use kaji_plugin_dotnet::generate_dotnet_sdk_with_style;
use kaji_plugin_elixir::generate_elixir_sdk_with_style;
use kaji_plugin_go::generate_go_sdk_with_style;
use kaji_plugin_java::generate_java_sdk_with_style;
use kaji_plugin_php::generate_php_sdk_with_style;
use kaji_plugin_python::generate_python_sdk_with_style;
use std::path::Path;

mod mock_server;

pub use mock_server::MockServerOptions;

pub use kaji_core::{SdkLanguage as Language, SdkStyle as Style, SdkTransport as Transport};

/// A maintained first-party SDK target. Each selected target writes one
/// independently publishable package under the profile-set output root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Rust,
    TypeScriptFetch,
    TypeScriptAxios,
    Go,
    Python,
    Php,
    Java,
    DotNet,
    Elixir,
    /// A language-neutral, standalone HTTP mock service derived from the
    /// OpenAPI contract. The generated package can be used by every SDK.
    MockServer,
}

/// User-facing controls for generated TypeScript SDK packages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeScriptOptions {
    pub client_name: Option<String>,
    pub client_style: SdkClientStyle,
    pub surface: SdkSurface,
    pub group_by_tag: bool,
}

impl Default for TypeScriptOptions {
    fn default() -> Self {
        Self {
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            surface: SdkSurface::Client,
            group_by_tag: true,
        }
    }
}

impl TypeScriptOptions {
    /// Direct models and operation functions, without an instantiated client.
    pub fn raw() -> Self {
        Self {
            surface: SdkSurface::Raw,
            ..Self::default()
        }
    }

    /// A product-style flat client, e.g. `client.createContact(...)`.
    pub fn flat_client() -> Self {
        Self {
            client_style: SdkClientStyle::Flat,
            ..Self::default()
        }
    }
}

impl Target {
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScriptFetch => "typescript-fetch",
            Self::TypeScriptAxios => "typescript-axios",
            Self::Go => "go",
            Self::Python => "python",
            Self::Php => "php",
            Self::Java => "java",
            Self::DotNet => "dotnet",
            Self::Elixir => "elixir",
            Self::MockServer => "mock-server",
        }
    }

    fn profile(self, root: &str, typescript: &TypeScriptOptions) -> Option<SdkProfile> {
        let output_dir = format!("{}/{}", root.trim_matches('/'), self.directory());
        match self {
            Self::Rust => Some(SdkProfile::rust(output_dir)),
            Self::TypeScriptFetch => Some(configured_typescript_profile(output_dir, typescript)),
            Self::TypeScriptAxios => Some(SdkProfile {
                transports: vec![SdkTransport::Axios],
                ..configured_typescript_profile(output_dir, typescript)
            }),
            Self::Go
            | Self::Python
            | Self::Php
            | Self::Java
            | Self::DotNet
            | Self::Elixir
            | Self::MockServer => None,
        }
    }
}

/// Distribution identity and public client layout for a native language
/// package. Kaji releases default to resource namespaces; choose [`Self::flat`]
/// when a package should expose only its conventional flat operation client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOptions {
    pub package_name: Option<String>,
    pub client_style: SdkClientStyle,
}

impl Default for PackageOptions {
    fn default() -> Self {
        Self {
            package_name: None,
            client_style: SdkClientStyle::Namespaced,
        }
    }
}

impl PackageOptions {
    /// A conventional flat client, without resource namespace facades.
    pub fn flat() -> Self {
        Self {
            client_style: SdkClientStyle::Flat,
            ..Self::default()
        }
    }
}

fn configured_typescript_profile(output_dir: String, options: &TypeScriptOptions) -> SdkProfile {
    SdkProfile {
        client_name: options.client_name.clone(),
        client_style: options.client_style,
        surface: options.surface,
        group_by_tag: options.group_by_tag,
        ..SdkProfile::typescript(output_dir)
    }
}

/// Builds a consistent multi-language SDK release from maintained targets.
///
/// The API deliberately selects packages rather than individual generators:
/// Fetch and Axios are distinct installable TypeScript packages, while a
/// future language plugin only needs to add another [`Target`] variant.
#[derive(Clone, Debug)]
pub struct ProfileSet {
    root: String,
    targets: Vec<Target>,
    typescript: TypeScriptOptions,
    go: PackageOptions,
    python: PackageOptions,
    php: PackageOptions,
    java: PackageOptions,
    dotnet: PackageOptions,
    elixir: PackageOptions,
    mock_server: MockServerOptions,
}

impl ProfileSet {
    pub fn new(output_root: impl Into<String>) -> Self {
        Self {
            root: output_root.into(),
            targets: Vec::new(),
            typescript: TypeScriptOptions::default(),
            go: PackageOptions::default(),
            python: PackageOptions::default(),
            php: PackageOptions::default(),
            java: PackageOptions::default(),
            dotnet: PackageOptions::default(),
            elixir: PackageOptions::default(),
            mock_server: MockServerOptions::default(),
        }
    }

    /// Adds a target once. Calling this repeatedly is idempotent.
    pub fn with(mut self, target: Target) -> Self {
        if !self.targets.contains(&target) {
            self.targets.push(target);
        }
        self
    }

    pub fn rust(self) -> Self {
        self.with(Target::Rust)
    }

    pub fn typescript_fetch(self) -> Self {
        self.with(Target::TypeScriptFetch)
    }

    pub fn typescript_axios(self) -> Self {
        self.with(Target::TypeScriptAxios)
    }

    pub fn go(self) -> Self {
        self.with(Target::Go)
    }

    pub fn python(self) -> Self {
        self.with(Target::Python)
    }

    pub fn php(self) -> Self {
        self.with(Target::Php)
    }

    pub fn java(self) -> Self {
        self.with(Target::Java)
    }

    pub fn dotnet(self) -> Self {
        self.with(Target::DotNet)
    }

    pub fn elixir(self) -> Self {
        self.with(Target::Elixir)
    }

    /// Adds an OpenAPI-derived standalone mock server. It emits httpmock YAML
    /// fixtures plus a Docker/Compose launcher; it is intentionally separate
    /// from language SDK packages so one server can test every client.
    pub fn mock_server(self) -> Self {
        self.with(Target::MockServer)
    }

    /// Applies the same API-surface controls to every selected TypeScript
    /// transport package.
    pub fn typescript_options(mut self, options: TypeScriptOptions) -> Self {
        self.typescript = options;
        self
    }

    pub fn go_options(mut self, options: PackageOptions) -> Self {
        self.go = options;
        self
    }

    pub fn python_options(mut self, options: PackageOptions) -> Self {
        self.python = options;
        self
    }

    pub fn php_options(mut self, options: PackageOptions) -> Self {
        self.php = options;
        self
    }

    pub fn java_options(mut self, options: PackageOptions) -> Self {
        self.java = options;
        self
    }

    pub fn dotnet_options(mut self, options: PackageOptions) -> Self {
        self.dotnet = options;
        self
    }

    pub fn elixir_options(mut self, options: PackageOptions) -> Self {
        self.elixir = options;
        self
    }

    /// Configures the standalone mock server package.
    pub fn mock_server_options(mut self, options: MockServerOptions) -> Self {
        self.mock_server = options;
        self
    }

    /// Returns the underlying generator profiles after validating the target
    /// set. This is useful when callers add their own Rust-native profiles.
    pub fn build(self) -> Result<Vec<SdkProfile>> {
        self.validate()?;
        let Self {
            root,
            targets,
            typescript,
            ..
        } = self;
        let root = root.trim_matches('/');
        Ok(targets
            .into_iter()
            .filter_map(|target| target.profile(root, &typescript))
            .collect())
    }

    fn validate(&self) -> Result<()> {
        if self.root.trim_matches('/').is_empty() {
            bail!("SDK profile output root cannot be empty")
        }
        if self.targets.is_empty() {
            bail!("SDK profile set must select at least one target")
        }
        Ok(())
    }
}

/// Generates all selected SDK packages from a normalized API model.
pub fn generate(api: &Api, profiles: ProfileSet) -> Result<GeneratedTree> {
    generate_with_security_catalog(api, profiles, None)
}

fn generate_with_security_catalog(
    api: &Api,
    profiles: ProfileSet,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    profiles.validate()?;
    let ProfileSet {
        root,
        targets,
        typescript,
        go,
        python,
        php,
        java,
        dotnet,
        elixir,
        mock_server,
    } = profiles;
    let root = root.trim_matches('/');
    let native_profiles = targets
        .iter()
        .filter_map(|target| target.profile(root, &typescript))
        .collect::<Vec<_>>();
    let mut tree = if native_profiles.is_empty() {
        GeneratedTree::default()
    } else {
        generate_sdks_with_security_catalog(api, &native_profiles, security_schemes)?
    };
    for target in &targets {
        match target {
            Target::Rust => tree.insert(GeneratedFile::new(
                format!("{root}/rust/STYLE_GUIDE.md"),
                rust_style_guide(api),
            )?)?,
            Target::TypeScriptFetch | Target::TypeScriptAxios => {
                tree.insert(GeneratedFile::new(
                    format!("{root}/{}/STYLE_GUIDE.md", target.directory()),
                    typescript_style_guide(api, &typescript),
                )?)?
            }
            Target::Go
            | Target::Python
            | Target::Php
            | Target::Java
            | Target::DotNet
            | Target::Elixir => {}
            Target::MockServer => {}
        }
    }
    for target in targets {
        match target {
            Target::Go => tree.append(generate_go_sdk_with_style(
                api,
                &format!("{root}/go"),
                go.package_name.as_deref(),
                go.client_style,
            )?)?,
            Target::Python => tree.append(generate_python_sdk_with_style(
                api,
                &format!("{root}/python"),
                python.package_name.as_deref(),
                python.client_style,
            )?)?,
            Target::Php => tree.append(generate_php_sdk_with_style(
                api,
                &format!("{root}/php"),
                php.package_name.as_deref(),
                php.client_style,
            )?)?,
            Target::Java => tree.append(generate_java_sdk_with_style(
                api,
                &format!("{root}/java"),
                java.package_name.as_deref(),
                java.client_style,
            )?)?,
            Target::DotNet => tree.append(generate_dotnet_sdk_with_style(
                api,
                &format!("{root}/dotnet"),
                dotnet.package_name.as_deref(),
                dotnet.client_style,
            )?)?,
            Target::Elixir => tree.append(generate_elixir_sdk_with_style(
                api,
                &format!("{root}/elixir"),
                elixir.package_name.as_deref(),
                elixir.client_style,
            )?)?,
            Target::MockServer => tree.append(mock_server::generate(
                api,
                &format!("{root}/mock-server"),
                &mock_server,
            )?)?,
            Target::Rust | Target::TypeScriptFetch | Target::TypeScriptAxios => {}
        }
    }
    Ok(tree)
}

fn rust_style_guide(api: &Api) -> String {
    format!(
        "# {} Rust SDK style guide\n\nThis package provides Kaji's resource-first Rust client: `client.contacts().list().await`. Direct `Client` operation methods remain available for compatibility. Path-parameter operations accept a completed path; typed path and query input structs are the next Rust-surface enhancement.\n",
        api.name
    )
}

fn typescript_style_guide(api: &Api, options: &TypeScriptOptions) -> String {
    let selected = match options.surface {
        SdkSurface::Raw => "raw exports",
        SdkSurface::Client => match options.client_style {
            SdkClientStyle::Flat => "flat instantiated client",
            SdkClientStyle::Namespaced => "namespaced instantiated client",
        },
    };
    format!(
        "# {} TypeScript SDK style guide\n\nThis package selected the **{selected}** surface. Generated models and direct operation exports remain available in every mode.\n\n- `SdkSurface::Raw`: direct models and operation functions only.\n- `SdkClientStyle::Flat`: `client.createContact(...)`.\n- `SdkClientStyle::Namespaced`: `client.contacts.create(...)`.\n\nConfigure the package with `TypeScriptOptions` in `kaji`.\n",
        api.name
    )
}

/// Generates all selected SDK packages from an existing Go OpenAPI sidecar
/// output directory.
pub fn generate_openapi(
    sidecar_output: &Path,
    name: impl Into<String>,
    version: impl Into<String>,
    profiles: ProfileSet,
) -> Result<GeneratedTree> {
    let api = kaji_core::adapter::openapi_sidecar::load_operations(
        sidecar_output,
        name.into(),
        version.into(),
    )?;
    let security_schemes_path = sidecar_output.join("security-schemes.json");
    let security_schemes = security_schemes_path
        .exists()
        .then(|| kaji_core::adapter::openapi_sidecar::load_security_schemes(sidecar_output))
        .transpose()?;
    generate_with_security_catalog(&api, profiles, security_schemes.as_ref())
}

/// Exposes the underlying typed profile for advanced native composition. New
/// target crates should normally offer a [`Target`] variant or an equivalent
/// preset builder rather than ask consumers to construct plugin lists.
pub fn custom(profile: SdkProfile) -> SdkProfile {
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_set_emits_one_isolated_package_per_target() {
        let profiles = ProfileSet::new("sdks")
            .rust()
            .typescript_fetch()
            .typescript_axios()
            .build()
            .unwrap();
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].output_dir, "sdks/rust");
        assert_eq!(profiles[1].output_dir, "sdks/typescript-fetch");
        assert_eq!(profiles[2].output_dir, "sdks/typescript-axios");
        assert_eq!(profiles[2].transports, vec![SdkTransport::Axios]);
    }

    #[test]
    fn selecting_a_target_twice_does_not_duplicate_a_package() {
        let profiles = ProfileSet::new("sdks")
            .typescript_fetch()
            .typescript_fetch()
            .build()
            .unwrap();
        assert_eq!(profiles.len(), 1);
    }

    #[test]
    fn typescript_options_control_the_sdk_surface() {
        let profiles = ProfileSet::new("sdks")
            .typescript_fetch()
            .typescript_options(TypeScriptOptions::raw())
            .build()
            .unwrap();
        assert_eq!(profiles[0].surface, SdkSurface::Raw);
        assert_eq!(profiles[0].client_style, SdkClientStyle::Namespaced);
    }

    #[test]
    fn a_release_can_mix_native_language_packages() {
        let tree = generate(
            &Api {
                name: "Test API".into(),
                version: "1.0.0".into(),
                ..Api::default()
            },
            ProfileSet::new("sdks")
                .rust()
                .typescript_fetch()
                .go()
                .python()
                .php()
                .java()
                .dotnet()
                .elixir(),
        )
        .unwrap();
        assert!(tree.get("sdks/rust/Cargo.toml").is_some());
        assert!(tree.get("sdks/rust/STYLE_GUIDE.md").is_some());
        assert!(tree.get("sdks/typescript-fetch/STYLE_GUIDE.md").is_some());
        assert!(tree.get("sdks/go/go.mod").is_some());
        assert!(tree.get("sdks/python/pyproject.toml").is_some());
        assert!(tree.get("sdks/php/composer.json").is_some());
        assert!(tree.get("sdks/java/build.gradle").is_some());
        assert!(tree.get("sdks/dotnet/TestApiSdk.csproj").is_some());
        assert!(tree.get("sdks/elixir/mix.exs").is_some());
        for target in ["go", "python", "php", "java", "dotnet", "elixir"] {
            assert!(
                tree.get(format!("sdks/{target}/STYLE_GUIDE.md")).is_some(),
                "{target} should export its namespaced client style guide"
            );
        }
    }

    #[test]
    fn a_release_can_include_the_language_neutral_mock_server() {
        let tree = generate(
            &Api {
                name: "Test API".into(),
                version: "1.0.0".into(),
                ..Api::default()
            },
            ProfileSet::new("release")
                .typescript_fetch()
                .rust()
                .mock_server(),
        )
        .unwrap();

        assert!(tree.get("release/rust/Cargo.toml").is_some());
        assert!(tree.get("release/typescript-fetch/package.json").is_some());
        assert!(tree.get("release/mock-server/compose.yaml").is_some());
        assert!(tree.get("release/mock-server/Dockerfile").is_some());
        assert!(tree.get("release/mock-server/fixtures/README.md").is_some());
    }

    #[test]
    fn mock_server_options_control_its_public_port() {
        let tree = generate(
            &Api {
                name: "Test API".into(),
                version: "1.0.0".into(),
                ..Api::default()
            },
            ProfileSet::new("release")
                .mock_server()
                .mock_server_options(MockServerOptions {
                    image: "example.test/kaji-httpmock:1".into(),
                    port: 8123,
                }),
        )
        .unwrap();
        assert!(
            tree.get("release/mock-server/Dockerfile")
                .unwrap()
                .contains("example.test/kaji-httpmock:1")
        );
        assert!(
            tree.get("release/mock-server/compose.yaml")
                .unwrap()
                .contains("8123")
        );
    }

    #[test]
    fn native_packages_can_opt_into_the_flat_client() {
        let tree = generate(
            &Api {
                name: "Test API".into(),
                version: "1.0.0".into(),
                ..Api::default()
            },
            ProfileSet::new("sdks")
                .go_options(PackageOptions::flat())
                .go(),
        )
        .unwrap();
        assert!(tree.get("sdks/go/STYLE_GUIDE.md").is_none());
        assert!(
            tree.get("sdks/go/README.md")
                .unwrap()
                .to_ascii_lowercase()
                .contains("flat")
        );
    }

    #[test]
    fn an_empty_target_set_is_rejected() {
        assert!(ProfileSet::new("sdks").build().is_err());
    }
}
