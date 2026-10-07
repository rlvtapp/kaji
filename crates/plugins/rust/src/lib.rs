//! Rust SDK generation through typed packages and a Reqwest-backed client.
mod bundled;
pub mod composition;
mod render;
pub use composition::{client, models, operations, roundtrip_tests, transport};

use anyhow::Result;
use kaji_core::engine::{Language, Meta, Package, Plugin, PluginContext};
use kaji_core::{GeneratedFile, SdkClientStyle};

pub struct Rust;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Rust {
    const NAME: &'static str = "rust";
    type Settings = Settings;
    type Workspace = composition::Workspace;
    fn bundle_middleware(
        tree: &mut kaji_core::GeneratedTree,
        middleware: &[kaji_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled::bundle(tree, middleware)
    }
    fn finalize(cx: &mut kaji_core::engine::FinalizeContext<'_, Self>) -> Result<()> {
        composition::Workspace::finalize(cx)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Rust> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Rust> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}

/// Generates Rust models, the Reqwest client, and Cargo package metadata.
pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    operation_prefix: Option<String>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
        operation_prefix: None,
    }
}
impl Sdk {
    /// Prefixes direct operation method names; resource methods delegate to them.
    pub fn operation_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.operation_prefix = Some(prefix.into());
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
impl Plugin<Rust> for Sdk {
    fn kind(&self) -> &'static str {
        "rust-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<kaji_core::engine::Provision> {
        vec![
            kaji_core::engine::Provision::of::<composition::Models>(),
            kaji_core::engine::Provision::of::<composition::Transport>(),
            kaji_core::engine::Provision::of::<composition::Operations>(),
            kaji_core::engine::Provision::of::<composition::Client>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let style = self
            .client_style
            .or(cx.common.client_style)
            .unwrap_or(SdkClientStyle::Namespaced);
        let options = render::RenderOptions {
            crate_name: cx.settings.package_name.clone(),
            client_style: style,
            operation_prefix: self.operation_prefix.clone(),
        };
        for (file, _) in render::generate_sdk(cx.api, &options)?.into_files() {
            if !matches!(
                file.path.to_str(),
                Some("src/lib.rs" | "src/client/mod.rs" | "Cargo.toml")
            ) {
                cx.files.emit(file)?;
            }
        }
        cx.workspace.models = true;
        cx.workspace.operations = true;
        cx.workspace.resources = style == SdkClientStyle::Namespaced;
        cx.workspace.transport = Some(composition::default_transport());
        cx.publish(composition::model_contract(cx.api))?;
        cx.publish(composition::default_transport())?;
        cx.publish(composition::operation_contract(
            cx.api,
            self.operation_prefix.clone(),
        ))?;
        cx.publish(composition::Client {
            symbol: "crate::Client".into(),
        })?;
        cx.files.emit(GeneratedFile::new(
            "STYLE_GUIDE.md",
            style_guide(cx.api, style),
        )?)
    }
}

fn style_guide(api: &kaji_core::Api, style: SdkClientStyle) -> String {
    let surface = match style {
        SdkClientStyle::Flat => {
            "Call operation methods directly on `Client`. No resource accessors are generated."
        }
        SdkClientStyle::Namespaced => {
            "Call operations through resource accessors, such as `client.contacts().list().await`. Direct operation methods are also exposed on `Client`."
        }
    };
    format!(
        "# {} Rust SDK style guide\n\n{surface}\n\nOperations with path, query, or header parameters accept typed request structs. Supply parameter values rather than a completed URL; the client escapes path segments and serializes query values. Request bodies are passed separately.\n",
        api.name
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::engine::Packages;
    use kaji_core::{Api, HttpMethod, Operation};

    fn api() -> Api {
        Api {
            name: "Contacts".into(),
            version: "1.0.0".into(),
            operations: vec![Operation {
                id: "listContacts".into(),
                method: HttpMethod::Get,
                path: "/contacts".into(),
                ..Operation::default()
            }],
            ..Api::default()
        }
    }

    #[test]
    fn flat_sdk_readme_matches_its_direct_method_surface() {
        let tree = Packages::new()
            .package(
                package("sdk")
                    .name("custom-sdk")
                    .with(sdk().flat().operation_prefix("api")),
            )
            .generate(&api(), None)
            .unwrap();
        let client = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
        assert!(client.contains("pub async fn api_list_contacts"));
        assert!(!client.contains("pub fn contacts(&self)"));
        let readme = tree.get("sdk/README.md").unwrap();
        assert!(readme.contains("client.api_list_contacts().await?"));
        assert!(!readme.contains("client.contacts()"));
        assert!(
            tree.get("sdk/Cargo.toml")
                .unwrap()
                .contains("name = \"custom-sdk\"")
        );
        assert!(
            tree.get("sdk/STYLE_GUIDE.md")
                .unwrap()
                .contains("No resource accessors are generated")
        );
    }

    #[test]
    fn namespaced_sdk_delegates_to_the_configured_direct_method() {
        let tree = Packages::new()
            .package(package("sdk").with(sdk().namespaced().operation_prefix("api")))
            .generate(&api(), None)
            .unwrap();
        let operations = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
        let resources = tree
            .get("sdk/src/client/resources/contacts_1/chunk_0001.rs")
            .unwrap();
        assert!(operations.contains("pub async fn api_list_contacts"));
        assert!(resources.contains("pub fn contacts(&self)"));
        assert!(resources.contains("self.client.api_list_contacts().await"));
        assert!(
            tree.get("sdk/README.md")
                .unwrap()
                .contains("client.contacts().list().await?")
        );
        assert!(
            !tree
                .get("sdk/STYLE_GUIDE.md")
                .unwrap()
                .contains("compatibility")
        );
    }

    #[test]
    fn typed_operation_schemas_preserve_arrays_bodies_and_empty_responses() {
        use kaji_core::{
            OperationMediaType, OperationRequestBody, OperationResponse, SchemaKind, SchemaValue,
        };
        let mut api = api();
        api.operations = vec![
            Operation {
                id: "replaceContacts".into(),
                method: HttpMethod::Post,
                path: "/contacts".into(),
                request_body: Some(OperationRequestBody::json(
                    SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::String)),
                    }),
                    true,
                )),
                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                    }),
                )],
                ..Default::default()
            },
            Operation {
                id: "deleteContact".into(),
                method: HttpMethod::Delete,
                path: "/contacts".into(),
                responses: vec![OperationResponse {
                    status: "204".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            },
            Operation {
                id: "unknownBody".into(),
                method: HttpMethod::Post,
                path: "/unknown".into(),
                request_body: Some(OperationRequestBody {
                    required: true,
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: None,
                    }],
                }),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: "application/json".into(),
                        schema: None,
                    }],
                }],
                ..Default::default()
            },
        ];
        let tree = Packages::new()
            .package(package("sdk").with(sdk().flat()))
            .generate(&api, None)
            .unwrap();
        let source = tree.get("sdk/src/client/operations/chunk_0001.rs").unwrap();
        assert!(source.contains(
            "replace_contacts(&self, body: &Vec<String>) -> Result<Vec<i64>, ReplaceContactsError>"
        ));
        assert!(source.contains("delete_contact(&self) -> Result<(), DeleteContactError>"));
        assert!(source.contains("unknown_body(&self, body: &serde_json::Value) -> Result<serde_json::Value, UnknownBodyError>"));
        assert!(source.contains("request = request.json(body)"));
    }
}
