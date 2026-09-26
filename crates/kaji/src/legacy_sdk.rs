//! Compatibility profile API. Language renderers live in plugin crates.
use anyhow::{Result, bail};
use kaji_core::{
    Api, GeneratedTree, GeneratorConfig, SdkClientStyle, SecuritySchemeCatalog, generate,
};
use kaji_plugin_rust::render::{RustModels, RustPackage, RustReqwest};
pub use kaji_plugin_typescript::sdk::{SdkStyle, SdkSurface};
use serde::{Deserialize, Serialize};
use std::path::Path;
/// Stable SDK languages exposed by the Rust-native generation API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkLanguage {
    Rust,
    TypeScript,
}

/// A transport implementation selected within one language profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkTransport {
    Reqwest,
    Fetch,
    Axios,
}

/// One independently generated SDK package.
///
/// `output_dir` is mandatory so multiple language targets can be generated
/// together without colliding. Configuration intentionally stays typed here;
/// config-file adapters can deserialize into this shape without adding a JS
/// runtime to generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkProfile {
    pub language: SdkLanguage,
    pub output_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    /// Public client class name for class-oriented SDK targets. When omitted,
    /// it is derived from the OpenAPI title (for example `KajiEmail`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(default)]
    pub client_style: SdkClientStyle,
    #[serde(default)]
    pub surface: SdkSurface,
    #[serde(default)]
    pub transports: Vec<SdkTransport>,
    #[serde(default)]
    pub style: SdkStyle,
    /// Split structured output by the first OpenAPI tag. This prevents operation/schema name
    /// collisions without forcing one giant file. Set this to `false` for a
    /// flat package layout.
    #[serde(default = "default_group_by_tag")]
    pub group_by_tag: bool,
}

fn default_group_by_tag() -> bool {
    true
}

impl SdkProfile {
    pub fn rust(output_dir: impl Into<String>) -> Self {
        Self {
            language: SdkLanguage::Rust,
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            transports: vec![SdkTransport::Reqwest],
            style: SdkStyle::Native,
            surface: SdkSurface::Client,
            group_by_tag: false,
        }
    }

    pub fn typescript(output_dir: impl Into<String>) -> Self {
        Self {
            language: SdkLanguage::TypeScript,
            output_dir: output_dir.into(),
            package_name: None,
            client_name: None,
            client_style: SdkClientStyle::Namespaced,
            transports: vec![SdkTransport::Fetch],
            style: SdkStyle::Structured,
            surface: SdkSurface::Client,
            group_by_tag: true,
        }
    }
}

/// Generates every requested SDK from a single language-neutral API model.
pub fn generate_sdks(api: &Api, profiles: &[SdkProfile]) -> Result<GeneratedTree> {
    generate_sdks_with_security_catalog(api, profiles, None)
}

/// Generates SDK packages with the optional reusable OpenAPI security-scheme
/// catalog.  The catalog is intentionally a separate argument: callers with
/// a pre-existing `Api` retain the exact historical output, while the Docs
/// Compiler sidecar can opt into named, per-operation credentials.
pub fn generate_sdks_with_security_catalog(
    api: &Api,
    profiles: &[SdkProfile],
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Result<GeneratedTree> {
    let mut result = GeneratedTree::default();
    for profile in profiles {
        let config = profile_config(profile)?;
        let tree = match profile.language {
            SdkLanguage::Rust => {
                require_exact_transports(profile, &[SdkTransport::Reqwest])?;
                let models = RustModels;
                let client = RustReqwest;
                let package = RustPackage;
                generate(
                    api,
                    &[
                        (&models, config.clone()),
                        (&client, config.clone()),
                        (&package, config),
                    ],
                )?
            }
            SdkLanguage::TypeScript => {
                if profile.transports.is_empty() {
                    bail!("TypeScript SDK profiles need at least one transport")
                }
                if profile
                    .transports
                    .iter()
                    .any(|transport| matches!(transport, SdkTransport::Reqwest))
                {
                    bail!("Reqwest is only available for Rust SDK profiles")
                }
                kaji_plugin_typescript::sdk::generate_sdk(
                    api,
                    &kaji_plugin_typescript::sdk::SdkProfile {
                        output_dir: profile.output_dir.clone(),
                        package_name: profile.package_name.clone(),
                        client_name: profile.client_name.clone(),
                        client_style: profile.client_style,
                        surface: profile.surface,
                        style: profile.style,
                        group_by_tag: profile.group_by_tag,
                        transports: profile
                            .transports
                            .iter()
                            .map(|t| match t {
                                SdkTransport::Fetch => {
                                    kaji_plugin_typescript::sdk::SdkTransport::Fetch
                                }
                                SdkTransport::Axios => {
                                    kaji_plugin_typescript::sdk::SdkTransport::Axios
                                }
                                SdkTransport::Reqwest => unreachable!("validated above"),
                            })
                            .collect(),
                    },
                    security_schemes,
                )?
            }
        };
        result.append(tree)?;
    }
    Ok(result)
}

/// Loads embedded Go compiler artifacts and generates legacy SDK profiles.
pub fn generate_openapi_sdks(
    sidecar_output: &Path,
    name: impl Into<String>,
    version: impl Into<String>,
    profiles: &[SdkProfile],
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
    generate_sdks_with_security_catalog(&api, profiles, security_schemes.as_ref())
}

fn profile_config(profile: &SdkProfile) -> Result<GeneratorConfig> {
    let output_dir = profile.output_dir.trim_matches('/');
    if output_dir.is_empty() || output_dir == "." {
        bail!("SDK profiles need a non-empty output_dir")
    }
    let mut config = GeneratorConfig::from([("output_dir".into(), output_dir.into())]);
    config.insert(
        "sdk_surface".into(),
        match profile.surface {
            SdkSurface::Raw => "raw",
            SdkSurface::Client => "client",
        }
        .into(),
    );
    config.insert(
        "client_style".into(),
        match profile.client_style {
            SdkClientStyle::Flat => "flat",
            SdkClientStyle::Namespaced => "namespaced",
        }
        .into(),
    );
    if let Some(package_name) = &profile.package_name {
        config.insert("package_name".into(), package_name.clone());
        config.insert("crate_name".into(), package_name.clone());
    }
    let clients = profile
        .transports
        .iter()
        .filter_map(|transport| match transport {
            SdkTransport::Fetch => Some("fetch"),
            SdkTransport::Axios => Some("axios"),
            SdkTransport::Reqwest => None,
        })
        .collect::<Vec<_>>();
    if !clients.is_empty() {
        config.insert("clients".into(), clients.join(","));
    }
    Ok(config)
}

fn require_exact_transports(profile: &SdkProfile, allowed: &[SdkTransport]) -> Result<()> {
    if profile.transports.is_empty()
        || profile
            .transports
            .iter()
            .any(|transport| !allowed.contains(transport))
    {
        bail!("Rust SDK profiles currently support the reqwest transport")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{HttpMethod, Operation, Schema, SchemaKind, SchemaValue};
    use std::fs;
    #[test]
    fn one_openapi_model_generates_isolated_rust_and_typescript_sdks() {
        let api = Api {
            name: "Example API".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new("Message", SchemaValue::new(SchemaKind::String))],
            operations: vec![Operation {
                id: "sendMessage".into(),
                method: HttpMethod::Post,
                path: "/messages".into(),
                response_type: "Message".into(),
                request_type: Some("Message".into()),
                ..Operation::default()
            }],
            ..Api::default()
        };
        let tree = generate_sdks(
            &api,
            &[
                SdkProfile::rust("sdk/rust"),
                SdkProfile {
                    transports: vec![SdkTransport::Fetch, SdkTransport::Axios],
                    style: SdkStyle::Native,
                    ..SdkProfile::typescript("sdk/typescript")
                },
            ],
        )
        .unwrap();
        assert!(tree.get("sdk/rust/src/client.rs").is_some());
        assert!(tree.get("sdk/typescript/fetch.ts").is_some());
        assert!(tree.get("sdk/typescript/axios.ts").is_some());
        assert!(tree.get("sdk/typescript/package.json").is_some());
    }

    #[test]
    fn profiles_reject_a_transport_for_the_wrong_language() {
        let error = generate_sdks(
            &Api::default(),
            &[SdkProfile {
                language: SdkLanguage::Rust,
                output_dir: "sdk/rust".into(),
                package_name: None,
                client_name: None,
                client_style: SdkClientStyle::Flat,
                surface: SdkSurface::Raw,
                transports: vec![SdkTransport::Fetch],
                style: SdkStyle::Native,
                group_by_tag: false,
            }],
        )
        .unwrap_err();
        assert!(error.to_string().contains("reqwest"));
    }

    #[test]
    fn sidecar_openapi_output_uses_the_same_language_profiles() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("operations.json"),
            r#"{"GET /messages":"get_messages.json"}"#,
        )
        .unwrap();
        fs::write(
            directory.path().join("operations-order.json"),
            r#"["GET /messages"]"#,
        )
        .unwrap();
        fs::create_dir(directory.path().join("operations")).unwrap();
        fs::write(
            directory.path().join("operations/get_messages.json"),
            r#"{"path":"/messages","method":"GET","responses":[]}"#,
        )
        .unwrap();
        let tree = generate_openapi_sdks(
            directory.path(),
            "Any API",
            "1.0.0",
            &[SdkProfile::typescript("sdk/typescript")],
        )
        .unwrap();
        assert!(
            tree.get("sdk/typescript/clients/messages/getMessages.ts")
                .is_some()
        );
        let client = tree.get("sdk/typescript/client.ts").unwrap();
        assert!(client.contains("export class Any"));
        assert!(client.contains("class MessagesClient"));
        assert!(client.contains("readonly get: typeof getMessages"));
        assert!(!client.contains("bind()"));
    }
}
