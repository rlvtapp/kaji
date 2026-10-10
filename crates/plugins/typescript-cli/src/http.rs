//! Http implementation for generated typescript-cli packages.
use super::*;

/// Provider-specific OAuth metadata. The OpenAPI security scheme describes
/// OAuth's standard authorization/token URLs; this configuration supplies the
/// public CLI client identity and optional device endpoint that OpenAPI cannot.
#[derive(Clone, Debug, Default)]
pub struct OAuthConfig {
    pub security_scheme: Option<String>,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub preferred_flow: Option<String>,
    pub authorization_url: Option<String>,
    pub device_authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub redirect_uri: Option<String>,
}

pub struct Cli {
    pub(crate) http_input: poolster_core::engine::HttpInput,
    pub(crate) meta: Meta,
    pub(crate) command_name: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) oauth: Option<OAuthConfig>,
}

pub fn cli() -> Cli {
    Cli {
        http_input: Default::default(),
        meta: Meta::new(),
        command_name: None,
        base_url: None,
        oauth: None,
    }
}

impl Plugin<TypeScriptCli> for Cli {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "typescript-cli"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScriptCli>) -> Result<()> {
        let selected = self.http_input.resolve(cx)?;
        let input_api = &selected.api;
        let command_name = self
            .command_name
            .clone()
            .unwrap_or_else(|| kebab_case(&input_api.name));
        if command_name.is_empty() {
            bail!("TypeScript CLI command name cannot be empty");
        }
        let package_name = cx
            .settings
            .package_name
            .clone()
            .unwrap_or_else(|| format!("{}-cli", command_name));
        let config = render_config(
            &command_name,
            self.base_url.as_deref(),
            self.oauth.as_ref(),
            selected.security_schemes.as_ref(),
        )?;
        cx.files.emit(GeneratedFile::new(
            "package.json",
            render_package_json(&package_name, &command_name, &input_api.version),
        )?)?;
        cx.files
            .emit(GeneratedFile::new("tsconfig.json", render_tsconfig())?)?;
        for (path, source) in render_command_modules(input_api, &command_name, &config) {
            cx.files.emit(GeneratedFile::new(path, source)?)?;
        }
        cx.files
            .emit(GeneratedFile::new("src/runtime.ts", render_runtime())?)?;
        cx.files.emit_custom(GeneratedFile::new(
            "src/poolster.extension.ts",
            render_extension(),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "README.md",
            render_readme(
                input_api,
                &package_name,
                &command_name,
                self.oauth.is_some(),
            ),
        )?)?;
        for (group, reference) in render_skill_references(input_api, &command_name) {
            cx.files.emit(GeneratedFile::new(
                format!("references/{group}.md"),
                reference,
            )?)?;
        }
        Ok(())
    }
}
