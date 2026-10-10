//! HTTP SDK orchestration over the owned HTTP contract.
use super::*;
#[path = "lib_input.rs"]
mod input;

/// Generates Rust models, the Reqwest client, and Cargo package metadata.
pub struct Sdk {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    client_style: Option<SdkClientStyle>,
    operation_prefix: Option<String>,
}
pub fn sdk() -> Sdk {
    Sdk {
        http_input: Default::default(),
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
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "rust-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<poolster_core::engine::Provision> {
        vec![
            poolster_core::engine::Provision::of::<composition::Models>(),
            poolster_core::engine::Provision::of::<composition::Transport>(),
            poolster_core::engine::Provision::of::<composition::Operations>(),
            poolster_core::engine::Provision::of::<composition::Client>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            cx.workspace.http_api = Some(cx.api.clone());
            let style = self
                .client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced);
            let options = render::RenderOptions {
                crate_name: cx.settings.package_name.clone(),
                client_style: style,
                operation_prefix: self.operation_prefix.clone(),
                open_unions: cx.settings.open_unions,
                open_enums: cx.settings.open_enums,
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
        })
    }
}

fn style_guide(api: &poolster_core::Api, style: SdkClientStyle) -> String {
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
