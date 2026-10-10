//! Http implementation for generated rust-cli packages.
use super::*;

pub struct Cli {
    pub(crate) http_input: poolster_core::engine::HttpInput,
    pub(crate) meta: Meta,
    pub(crate) command_name: Option<String>,
    pub(crate) base_url: Option<String>,
}
pub fn cli() -> Cli {
    Cli {
        http_input: Default::default(),
        meta: Meta::new(),
        command_name: None,
        base_url: None,
    }
}
impl Cli {
    pub fn command_name(mut self, value: impl Into<String>) -> Self {
        self.command_name = Some(value.into());
        self
    }
    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = Some(value.into());
        self
    }
}
impl Plugin<RustCli> for Cli {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "rust-cli"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, RustCli>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let command = self
                .command_name
                .clone()
                .unwrap_or_else(|| kebab_case(&cx.api.name));
            if command.is_empty() {
                bail!("Rust CLI command name cannot be empty");
            }
            let package = cx
                .settings
                .package_name
                .clone()
                .unwrap_or_else(|| format!("{}-cli", command));
            cx.files.emit(GeneratedFile::new(
                "Cargo.toml",
                cargo_toml(&package, &command, &cx.api.version),
            )?)?;
            for (path, source) in render_command_modules(
                cx.api,
                &command,
                self.base_url.as_deref(),
                cx.security_schemes,
            ) {
                cx.files.emit(GeneratedFile::new(path, source)?)?;
            }
            cx.files.emit_custom(GeneratedFile::new(
                "src/poolster_extension.rs",
                extension_template(),
            )?)?;
            cx.files
                .emit(GeneratedFile::new("README.md", readme(cx.api, &command))?)?;
            for (group, reference) in render_skill_references(cx.api, &command) {
                cx.files.emit(GeneratedFile::new(
                    format!("references/{group}.md"),
                    reference,
                )?)?;
            }
            Ok(())
        })
    }
}
