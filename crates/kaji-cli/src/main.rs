use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use check_rules::CHECK_RULES;
use cli_args::parse;
use generation_lock::{
    GENERATION_LOCK_PATH, GENERATION_LOCK_VERSION, GenerationReplayLock, UpdateInputLock,
    UpdateLock, generation_lock,
};
#[cfg(test)]
use mock_http::{
    mock_path_matches, mock_path_parameters, mock_path_specificity, mock_response_body,
    mock_target_parts,
};
use mock_server::serve_mock;
use openapi_sources::{compiler_source_origin, download_openapi, remote_spec_url};
use poolster::ts::artifacts::{
    ArtifactOptions, McpToolManifest, ReDoc, TypeScriptCypress, TypeScriptFaker, TypeScriptMsw,
    TypeScriptReactQuery, TypeScriptSwr, TypeScriptVueQuery, TypeScriptZod,
};
use poolster::{
    SdkClientStyle, csharp, dotnet, elixir, go, java, mock, php, postman, prelude::*, python, ruby,
    rust, rust_cli, swift, symfony, terraform, ts, ts_cli,
};
use poolster_core::{Api, GeneratedFile, GeneratedTree};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use update_command::replay_input_is_unchanged;
use update_command::update;

mod check_command;
mod check_rules;
mod cli_args;
mod config_defaults;
mod contract;
mod credentials;
mod eject;
mod generation_lock;
mod mcp;
mod migration;
mod mock_http;
mod mock_server;
mod openapi_sources;
mod registry;
mod sdk_automation;
mod sdk_doctor;
mod sdk_install;
mod sdk_status;
mod show_command;
mod update_command;

const HELP: &str = "Poolster — native multi-language OpenAPI SDK generator

Usage:
  poolster migrate [project-or-config] [--input <file>] [--output <new-directory>] [--strict]
  poolster init [--config <file>] [--input <openapi-file>] [--output <directory>]
  poolster generate                         # reads ./poolster.json
  poolster generate --config <file>
  poolster generate <openapi-file> --output <directory> --language <target>...
  poolster generate --artifacts <directory> --output <directory> --language <target>...
  poolster mcp <openapi-file> --base-url <url>
  poolster mcp generator
  poolster mock serve <openapi-file> [--port <port>]
  poolster contract plugins [--format human|json]
  poolster contract inspect <file> --input-format <format> [--provider <id>] [--format human|json]
  poolster check <openapi-file> [--format human|json]
  poolster show <openapi-file> [--include-path <pattern>] [--exclude-path <pattern>]
  poolster update [--output <directory>] [--force]
  poolster auth <login|logout|status> ...
  poolster discover <query> [--limit <count>] [--format human|json]
  poolster download <api-id> --output <openapi-file> [--version <version>]
  poolster languages
  poolster eject --language <target> --out <source-workspace>
  poolster sdk <init|sync|app|list|run|diff|pr|releases|connect|install|status|doctor|inspect> ...
  poolster --version

Config commands:
  init                                  Write a starter poolster.json; never overwrites it
  generate                              Read the config by default
      --config <file>                   Read a specific config file
      --color <mode>                    auto (default), always, or never

Generate options (both modes):
      --check                          Report drift without writing output
      --format human|json              Change report format
  -o, --output <directory>             Output root (required)
  -l, --language <target,...>          Repeatable; use all for every SDK (required)
      --name <name>                   API name (default: API)
      --sdk-version <version>         Generated package version (default: 0.1.0)
      --client-style <style>          namespaced (default) or flat
      --typescript-transport <kind>   fetch (default) or axios
      --typescript-surface <surface>  client (default) or raw
      --typescript-client-name <name> TypeScript client class name
      --jobs <count>                  Go emission workers (default: bounded auto)
      --artifacts <directory>         Reuse compiled OpenAPI JSON artifacts
      --openapi-compiler <file>       Override bundled poolster-openapi executable
      --include-path <pattern>        Generate only matching OpenAPI paths; repeatable
      --exclude-path <pattern>        Omit matching OpenAPI paths; repeatable
  -h, --help                          Show help

Targets: postman, terraform, rust, rust-cli, typescript, typescript-cli, go, python, php, symfony, java, csharp, dotnet (legacy alias), elixir, ruby, swift

MCP commands:
  mcp                                   Serve an OpenAPI document as MCP tools over stdio
      --base-url <url>                  API origin used when a tool is called (required)
      --openapi-compiler <file>         Override bundled poolster-openapi executable
  mcp generator                         Serve Poolster generation controls as MCP tools over stdio

Mock commands:
  mock serve                            Serve OpenAPI-derived happy-path responses without Docker
      --port <port>                     Local port (default: 4010)
      --openapi-compiler <file>         Override bundled poolster-openapi executable

Contract commands:
  check                                 Find API-contract issues that make generated SDKs and CLIs awkward
      --openapi-compiler <file>         Override bundled poolster-openapi executable
      --format <format>                  human (default) or json for automation
      --severity <rule=level>            Override a rule as warning or error; repeatable
      --fail-on <level>                  error (default), warning, or none
      --baseline <file>                  Suppress matching, known diagnostic fingerprints
      --write-baseline <file>            Record current diagnostics as a baseline
      --ignore <rule>                    Suppress a rule entirely; repeatable

Update commands:
  show <openapi-file>                    Inspect the generated path and operation tree
      --include-path <pattern>            Repeatable path filter
      --exclude-path <pattern>            Repeatable path exclusion
      --format <format>                   human (default) or json for automation
  update                                 Replay direct-generation lock files below output
      --output <directory>                Search root (default: current directory)
      --force                             Regenerate even when local input is unchanged

Auth commands:
  auth login <profile> --token-env <name> Register a named, environment-backed token
  auth logout <profile>                   Remove a named token profile
  auth status                             List profiles without exposing tokens

Registry commands:
  discover <query>                       Search the public OpenAPI directory
      --limit <count>                    Results to return (default: 20)
      --format <format>                  human (default) or json for automation
  download <api-id>                      Download its preferred OpenAPI version
      --version <version>                Select a directory version explicitly
      --output <openapi-file>            Destination; must not already exist

Each target is written to its own subdirectory. Owned generated files are updated;
custom starter files and unrelated files are preserved. Generation accepts local files,
HTTPS URLs, or a remote input object in config. The registry commands use APIs.guru.
The npm distribution bundles both native executables; Rust and Go are not required.
";

const LANGUAGES: &[&str] = &[
    "rust",
    "rust-cli",
    "typescript",
    "typescript-cli",
    "go",
    "python",
    "php",
    "symfony",
    "java",
    "csharp",
    "dotnet",
    "elixir",
    "ruby",
    "swift",
    "postman",
    "terraform",
];

// `all` intentionally remains the established shortcut for SDK packages. A
// generated executable needs product-specific configuration, so users select
// `typescript-cli` explicitly when they want one.
const SDK_LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "go",
    "python",
    "php",
    "java",
    "csharp",
    "elixir",
    "ruby",
    "swift",
];

#[derive(Debug)]
struct Generate {
    source: Option<OpenApiInput>,
    config: Option<PathBuf>,
    config_packages: Option<Vec<PackageConfig>>,
    artifacts: Option<PathBuf>,
    output: PathBuf,
    languages: Vec<String>,
    name: String,
    version: String,
    style: SdkClientStyle,
    raw: bool,
    typescript_transport: Option<TypeScriptTransport>,
    client_name: Option<String>,
    compiler: Option<PathBuf>,
    path_selection: PathSelection,
    config_sha256: Option<String>,
    source_sha256: Option<String>,
    jobs: usize,
    color: ColorChoice,
    check: bool,
    json_changes: bool,
}

#[derive(Debug)]
struct Mcp {
    source: PathBuf,
    base_url: String,
    compiler: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeScriptTransport {
    Fetch,
    Axios,
}

impl TypeScriptTransport {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "fetch" => Ok(Self::Fetch),
            "axios" => Ok(Self::Axios),
            _ => bail!("--typescript-transport must be fetch or axios"),
        }
    }
}

impl ColorChoice {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => bail!("--color must be auto, always, or never"),
        }
    }

    fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                std::io::stderr().is_terminal()
                    && env::var_os("NO_COLOR").is_none()
                    && env::var("TERM").is_ok_and(|term| term != "dumb")
            }
        }
    }
}

struct Reporter {
    color: bool,
    started: Instant,
}

impl Reporter {
    fn new(choice: ColorChoice) -> Self {
        Self {
            color: choice.enabled(),
            started: Instant::now(),
        }
    }

    fn paint(&self, code: &str, value: impl std::fmt::Display) -> String {
        if self.color {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    fn gold(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;208", value)
    }

    fn blue(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;117", value)
    }

    fn green(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;77", value)
    }

    fn dim(&self, value: impl std::fmt::Display) -> String {
        self.paint("2", value)
    }

    fn started(&self) {
        eprintln!(
            "\n{} {}",
            self.gold("$"),
            self.paint("1", "poolster generate")
        );
        eprintln!("  {} Generation started", self.gold("◆"));
    }

    fn compiling(&self, source: &Path) {
        eprintln!(
            "  {} Compiling {}",
            self.dim("◇"),
            self.dim(source.display())
        );
    }

    fn downloading(&self, url: &str) {
        eprintln!("  {} Downloading {}", self.dim("◇"), self.blue(url));
    }

    fn phase(&self, label: &str, elapsed: std::time::Duration) {
        eprintln!(
            "  {} {} {}",
            self.green("◇"),
            self.blue(label),
            self.dim(format!("completed in {}", duration(elapsed)))
        );
    }

    fn generated(&self, options: &Generate, elapsed: std::time::Duration) {
        for label in configured_labels(options) {
            eprintln!("  {} {}", self.green("◇"), self.blue(label));
        }
        self.phase("Generation", elapsed);
    }

    fn completed(&self, plugins: usize, files: usize, output: &Path) {
        eprintln!("  {} Generation completed", self.green("◇"));
        eprintln!();
        eprintln!(
            "  {:<10} {} {}",
            self.dim("Plugins"),
            self.green(plugins),
            self.dim("passed")
        );
        eprintln!(
            "  {:<10} {} {}",
            self.dim("Files"),
            self.green(files),
            self.dim("generated")
        );
        eprintln!(
            "  {:<10} {}",
            self.dim("Duration"),
            self.green(duration(self.started.elapsed()))
        );
        eprintln!(
            "  {:<10} {}",
            self.dim("Output"),
            self.blue(output.display())
        );
        eprintln!();
    }
}

fn duration(value: std::time::Duration) -> String {
    if value.as_secs_f64() >= 1.0 {
        format!("{:.2}s", value.as_secs_f64())
    } else {
        format!("{}ms", value.as_millis())
    }
}

fn configured_labels(options: &Generate) -> Vec<String> {
    if let Some(packages) = &options.config_packages {
        return packages
            .iter()
            .map(|package| {
                let plugins = package
                    .plugins
                    .iter()
                    .map(|plugin| plugin.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" + ");
                format!("{}  {}", package.path, plugins)
            })
            .collect();
    }
    options.languages.clone()
}

fn configured_plugin_count(options: &Generate) -> usize {
    options
        .config_packages
        .as_ref()
        .map(|packages| packages.iter().map(|package| package.plugins.len()).sum())
        .unwrap_or(options.languages.len())
}

#[derive(Debug)]
struct Init {
    config: PathBuf,
    input: PathBuf,
    output: PathBuf,
    name: String,
    version: String,
}

#[derive(Debug)]
struct MockServe {
    source: PathBuf,
    port: u16,
    compiler: Option<PathBuf>,
}

#[derive(Debug)]
struct Check {
    source: PathBuf,
    compiler: Option<PathBuf>,
    format: CheckFormat,
    severity_overrides: BTreeMap<String, CheckSeverity>,
    fail_on: CheckFailureThreshold,
    baseline: Option<PathBuf>,
    write_baseline: Option<PathBuf>,
    ignored_rules: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckSeverity {
    Warning,
    Error,
}

impl CheckSeverity {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            _ => bail!("severity must be warning or error, got {value:?}"),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckFailureThreshold {
    Warning,
    Error,
    None,
}

impl CheckFailureThreshold {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "none" => Ok(Self::None),
            _ => bail!("--fail-on must be warning, error, or none, got {value:?}"),
        }
    }

    fn fails(self, severity: CheckSeverity) -> bool {
        match self {
            Self::Warning => true,
            Self::Error => severity == CheckSeverity::Error,
            Self::None => false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectConfig {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    openapi: OpenApiConfig,
    output: OutputConfig,
    #[serde(default)]
    defaults: DefaultsConfig,
    #[serde(default)]
    packages: Vec<PackageConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenApiConfig {
    input: Option<OpenApiInput>,
    artifacts: Option<PathBuf>,
    #[serde(default = "default_api_name")]
    name: String,
    #[serde(default = "default_sdk_version")]
    version: String,
    compiler: Option<PathBuf>,
    #[serde(default)]
    paths: PathSelection,
}

/// Path filters intentionally use the same small glob language as Poolster's
/// operation filters: `*` matches any sequence (including `/`) and `?` one
/// Unicode scalar. Includes form an OR-set; an exclusion always wins.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PathSelection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum OpenApiInput {
    Path(PathBuf),
    Remote(RemoteInput),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteInput {
    url: String,
    #[serde(default)]
    headers: BTreeMap<String, SecretValue>,
    auth: Option<RemoteAuth>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum SecretValue {
    Literal(String),
    Environment { env: String },
    Profile { profile: String },
}

impl SecretValue {
    fn resolve(&self, field: &str) -> Result<String> {
        match self {
            Self::Literal(value) => Ok(value.clone()),
            Self::Environment { env: variable } => env::var(variable)
                .with_context(|| format!("read environment variable {variable:?} for {field}")),
            Self::Profile { profile } => credentials::resolve(profile)
                .with_context(|| format!("resolve Poolster auth profile {profile:?} for {field}")),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum RemoteAuth {
    Basic {
        username: SecretValue,
        password: SecretValue,
    },
    Bearer {
        token: SecretValue,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputConfig {
    path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsConfig {
    client_style: Option<String>,
    layout: Option<poolster_core::SourceLayout>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageConfig {
    language: String,
    path: String,
    name: Option<String>,
    version: Option<String>,
    release: Option<poolster_core::release::PackageMetadata>,
    client_style: Option<String>,
    layout: Option<poolster_core::SourceLayout>,
    #[serde(default)]
    api_reference: bool,
    #[serde(default)]
    idempotency: poolster_core::idempotency::IdempotencyConfig,
    #[serde(default)]
    plugins: Vec<PluginConfig>,
    #[serde(default)]
    customizations: Vec<CodeCustomizationConfig>,
    #[serde(skip)]
    resolved_customizations: Vec<poolster_core::customization::CodeCustomization>,
    #[serde(default)]
    middleware: Vec<BundledMiddlewareConfig>,
    #[serde(skip)]
    resolved_middleware: Vec<poolster_core::customization::BundledMiddleware>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundledMiddlewareConfig {
    source: PathBuf,
    path: PathBuf,
    symbol: String,
    async_symbol: Option<String>,
}
impl BundledMiddlewareConfig {
    fn load(&self, base: &Path) -> Result<poolster_core::customization::BundledMiddleware> {
        let source = config_path(base, self.source.clone());
        let middleware = poolster_core::customization::BundledMiddleware {
            path: self.path.clone(),
            contents: std::fs::read_to_string(&source)
                .with_context(|| format!("read bundled middleware source {}", source.display()))?,
            symbol: self.symbol.clone(),
            async_symbol: self.async_symbol.clone(),
        };
        middleware.validate()?;
        Ok(middleware)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum CodeCustomizationConfig {
    Add {
        path: PathBuf,
        source: PathBuf,
    },
    Replace {
        path: PathBuf,
        source: PathBuf,
    },
    Patch {
        path: PathBuf,
        find: String,
        source: PathBuf,
    },
}
impl CodeCustomizationConfig {
    fn load(&self, base: &Path) -> Result<poolster_core::customization::CodeCustomization> {
        use poolster_core::customization::CodeCustomization;
        let (path, source) = match self {
            Self::Add { path, source }
            | Self::Replace { path, source }
            | Self::Patch { path, source, .. } => (path, source),
        };
        GeneratedFile::new(path, "")?;
        let source = config_path(base, source.clone());
        let contents = std::fs::read_to_string(&source)
            .with_context(|| format!("read customization source {}", source.display()))?;
        Ok(match self {
            Self::Add { .. } => CodeCustomization::Add {
                path: path.clone(),
                contents,
            },
            Self::Replace { .. } => CodeCustomization::Replace {
                path: path.clone(),
                contents,
            },
            Self::Patch { find, .. } => CodeCustomization::Patch {
                path: path.clone(),
                find: find.clone(),
                replacement: contents,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginConfig {
    name: String,
    id: Option<String>,
    #[serde(default)]
    uses: BTreeMap<String, String>,
    integer_as_string: Option<bool>,
    int64: Option<String>,
    async_client: Option<bool>,
    open_enums: Option<bool>,
    open_unions: Option<bool>,
    preserve_presence: Option<bool>,
    transport: Option<String>,
    surface: Option<String>,
    client_name: Option<String>,
    group_by_tag: Option<bool>,
    split_by_group: Option<bool>,
    max_operations_per_file: Option<usize>,
    layout: Option<poolster_core::SourceLayout>,
    include_operations: Option<Vec<String>>,
    operation_kinds: Option<BTreeMap<String, String>>,
    operation_names: Option<BTreeMap<String, String>>,
    fixture_options: Option<poolster::ts::FixtureOptions>,
    cypress_options: Option<poolster::ts::CypressOptions>,
    max_file_bytes: Option<usize>,
    throw_on_error: Option<bool>,
    jobs: Option<usize>,
    output: Option<String>,
    clients_import: Option<String>,
    openapi_spec: Option<String>,
    title: Option<String>,
    image: Option<String>,
    port: Option<u16>,
    command_name: Option<String>,
    base_url: Option<String>,
    oauth: Option<CliOAuthConfig>,
    sdk_package: Option<String>,
    strict: Option<bool>,
    infer: Option<bool>,
    data_sources: Option<bool>,
    module: Option<String>,
    provider_name: Option<String>,
    registry_namespace: Option<String>,
    #[serde(default)]
    resources: Vec<terraform::ResourceBinding>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CliOAuthConfig {
    client_id: String,
    security_scheme: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
    preferred_flow: Option<String>,
    authorization_url: Option<String>,
    device_authorization_url: Option<String>,
    token_url: Option<String>,
    redirect_uri: Option<String>,
}

fn default_api_name() -> String {
    "API".into()
}
fn default_sdk_version() -> String {
    "0.1.0".into()
}

enum Action {
    Eject(Vec<OsString>),
    Migrate(Vec<OsString>),
    Contract(Vec<OsString>),
    Sdk(sdk_automation::Options),
    Help,
    Version,
    Languages,
    Show(Show),
    Update(Update),
    Auth(Auth),
    Discover(Discover),
    Download(Download),
    Init(Init),
    Mcp(Mcp),
    McpGenerator,
    MockServe(MockServe),
    Check(Check),
    Generate(Box<Generate>),
}

#[derive(Debug)]
struct Show {
    source: PathBuf,
    compiler: Option<PathBuf>,
    paths: PathSelection,
    format: ShowFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShowFormat {
    Human,
    Json,
}

#[derive(Debug)]
struct Update {
    output: PathBuf,
    force: bool,
}

#[derive(Debug)]
enum Auth {
    Login { profile: String, token_env: String },
    Logout { profile: String },
    Status,
}

#[derive(Debug)]
struct Discover {
    query: String,
    limit: usize,
    format: DiscoverFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoverFormat {
    Human,
    Json,
}

#[derive(Debug)]
struct Download {
    id: String,
    version: Option<String>,
    output: PathBuf,
}

fn compiler_path(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = override_path.or_else(|| {
        env::var_os("POOLSTER_OPENAPI_BIN")
            .or_else(|| env::var_os("KAJI_OPENAPI_BIN"))
            .map(PathBuf::from)
    }) {
        return std::fs::canonicalize(&path)
            .with_context(|| format!("cannot find OpenAPI compiler {}", path.display()));
    }
    let executable = env::current_exe().context("cannot locate the Poolster executable")?;
    let sibling = executable.with_file_name(if cfg!(windows) {
        "poolster-openapi.exe"
    } else {
        "poolster-openapi"
    });
    if !sibling.is_file() {
        bail!(
            "bundled OpenAPI compiler is missing: {}. Reinstall the platform package, or set --openapi-compiler /path/to/poolster-openapi (POOLSTER_OPENAPI_BIN is also supported). Source builds: build openapi/ with Go first",
            sibling.display()
        );
    }
    Ok(sibling)
}

fn parse_style(value: Option<&str>) -> Result<SdkClientStyle> {
    match value.unwrap_or("namespaced") {
        "namespaced" => Ok(SdkClientStyle::Namespaced),
        "flat" => Ok(SdkClientStyle::Flat),
        other => bail!("client_style must be \"namespaced\" or \"flat\", got {other:?}"),
    }
}

fn package_common(style: SdkClientStyle) -> Common {
    Common::default().client_style(style)
}

fn configured_common(style: SdkClientStyle, package: &PackageConfig) -> Common {
    let mut common = package_common(style);
    common.package_version = package.version.clone();
    common.layout = package.layout.clone();
    common
}

fn profiles(options: &Generate) -> Result<ProfileSet> {
    if let Some(packages) = &options.config_packages {
        return config_profiles(options.style, packages);
    }

    let mut profiles = ProfileSet::new(".").common(Common::default().client_style(options.style));
    for target in &options.languages {
        profiles = match target.as_str() {
            "rust" => profiles.package(rust::package("rust").with(rust::sdk())),
            "rust-cli" => profiles.package(rust_cli::package(target).with(rust_cli::cli())),
            "go" => profiles.package(go::package("go").with(go::sdk().jobs(options.jobs))),
            "python" => profiles.package(python::package("python").with(python::sdk())),
            "php" => profiles.package(php::package("php").with(php::sdk())),
            "symfony" => profiles.package(symfony::package("symfony").with(symfony::sdk())),
            "java" => profiles.package(java::package("java").with(java::sdk())),
            "csharp" => profiles.package(csharp::package("csharp").with(csharp::sdk())),
            // Keep the established selector for existing scripts. New
            // configuration and direct commands should use `csharp`.
            "dotnet" => profiles.package(dotnet::package("dotnet").with(dotnet::sdk())),
            "elixir" => profiles.package(elixir::package("elixir").with(elixir::sdk())),
            "ruby" => profiles.package(ruby::package("ruby").with(ruby::sdk())),
            "swift" => profiles.package(swift::package("swift").with(swift::sdk())),
            "postman" => profiles.package(
                postman::package("postman")
                    .with(postman::collection())
                    .with(postman::environment()),
            ),
            "terraform" => {
                profiles.package(terraform::package("terraform").with(terraform::provider()))
            }
            "typescript" => {
                let mut sdk = match options
                    .typescript_transport
                    .unwrap_or(TypeScriptTransport::Fetch)
                {
                    TypeScriptTransport::Fetch => ts::sdk().fetch(),
                    TypeScriptTransport::Axios => ts::sdk().axios(),
                };
                if options.raw {
                    sdk = sdk.raw();
                }
                if let Some(name) = &options.client_name {
                    sdk = sdk.client_name(name);
                }
                profiles.package(ts::package(target).with(sdk))
            }
            "typescript-cli" => profiles.package(ts_cli::package(target).with(ts_cli::cli())),
            _ => unreachable!("validated target"),
        };
    }
    Ok(profiles)
}

fn sdk_plugin(package: &PackageConfig) -> Result<&PluginConfig> {
    let plugins = package
        .plugins
        .iter()
        .filter(|plugin| plugin.name == "sdk")
        .collect::<Vec<_>>();
    match plugins.as_slice() {
        [plugin] => Ok(plugin),
        [] => bail!(
            "package {:?} ({}) requires exactly one {{\"name\":\"sdk\"}} plugin",
            package.path,
            package.language
        ),
        _ => bail!("package {:?} declares sdk more than once", package.path),
    }
}

fn has_only_known_plugins(package: &PackageConfig, allowed: &[&str]) -> Result<()> {
    for plugin in &package.plugins {
        if !allowed.contains(&plugin.name.as_str()) {
            bail!(
                "package {:?} uses plugin {:?}, which is not bundled by this Poolster binary",
                package.path,
                plugin.name
            );
        }
    }
    Ok(())
}

fn has_typescript_provider(package: &PackageConfig) -> bool {
    package.plugins.iter().any(|plugin| {
        matches!(
            plugin.name.as_str(),
            "sdk" | "models" | "transport" | "operations" | "client"
        )
    })
}

fn typescript_models(plugin: &PluginConfig) -> Result<ts::ModelOptions> {
    let int64_type = match plugin.int64.as_deref().unwrap_or("number") {
        "number" => ts::Int64Type::Number,
        "string" => ts::Int64Type::String,
        "bigint" => ts::Int64Type::BigInt,
        other => bail!("TypeScript int64 must be number, string, or bigint; got {other:?}"),
    };
    Ok(ts::ModelOptions {
        open_enums: plugin.open_enums.unwrap_or(false),
        integer_as_string: plugin.integer_as_string.unwrap_or(false),
        int64_type,
        ..Default::default()
    })
}

fn typescript_profile(
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<Package<ts::TypeScript>> {
    use ts::composition::{self, Models, Operations, Transport};
    has_only_known_plugins(
        package,
        &[
            "sdk",
            "models",
            "transport",
            "operations",
            "client",
            "zod",
            "tanstack-react-query",
            "tanstack-vue-query",
            "swr",
            "faker",
            "msw",
            "cypress",
            "operation-tests",
            "webhooks",
            "oauth",
        ],
    )?;
    if package.plugins.is_empty() {
        bail!("TypeScript package must declare plugins");
    }
    let mut output = ts::package(&package.path).common(configured_common(style, package));
    if let Some(name) = &package.name {
        output = output.name(name);
    }
    if !has_typescript_provider(package) {
        return Ok(output);
    }
    let mut models = BTreeMap::<String, Handle<Models>>::new();
    let mut transports = BTreeMap::<String, Handle<Transport>>::new();
    let mut operations = BTreeMap::<String, Handle<Operations>>::new();
    let mut ids = BTreeSet::new();
    let mut providers = Vec::new();
    for plugin in &package.plugins {
        let id = plugin.id.clone().unwrap_or_else(|| plugin.name.clone());
        if !ids.insert(id.clone()) {
            bail!("duplicate TypeScript plugin instance {id:?}; give each instance a unique id");
        }
        if plugin.name == "sdk" {
            if package.plugins.iter().any(|p| {
                matches!(
                    p.name.as_str(),
                    "models" | "transport" | "operations" | "client"
                )
            }) {
                bail!("select either sdk convenience or explicit SDK providers in one package");
            }
            if !plugin.uses.is_empty() {
                bail!("sdk convenience does not accept provider bindings; use explicit providers");
            }
            let mut sdk = match plugin.transport.as_deref().unwrap_or("fetch") {
                "fetch" => ts::sdk().fetch(),
                "axios" => ts::sdk().axios(),
                other => bail!("unknown TypeScript transport {other:?}"),
            }
            .label(&id)
            .model_options(typescript_models(plugin)?);
            match plugin.surface.as_deref().unwrap_or("client") {
                "raw" => sdk = sdk.raw(),
                "client" => {}
                other => bail!("unknown TypeScript surface {other:?}"),
            }
            if let Some(name) = &plugin.client_name {
                sdk = sdk.client_name(name);
            }
            if let Some(group) = plugin.group_by_tag {
                sdk = sdk.group_by_tag(group);
            }
            if let Some(throws) = plugin.throw_on_error {
                sdk = sdk.throw_on_error(throws);
            }
            models.insert(id.clone(), sdk.meta().handle());
            transports.insert(id.clone(), sdk.meta().handle());
            operations.insert(id, sdk.meta().handle());
            output = output.with(sdk);
        } else if matches!(
            plugin.name.as_str(),
            "models" | "transport" | "operations" | "client"
        ) {
            let mut provider = match plugin.name.as_str() {
                "models" => composition::models().model_options(typescript_models(plugin)?),
                "transport" => match plugin.transport.as_deref().unwrap_or("fetch") {
                    "fetch" => composition::transport(),
                    "axios" => composition::transport().axios(),
                    other => bail!("unknown TypeScript transport {other:?}"),
                },
                "operations" => composition::operations(),
                "client" => composition::client(),
                _ => unreachable!(),
            }
            .label(&id);
            if let Some(module) = &plugin.output {
                provider = provider.output(module.trim_end_matches(".ts"));
            }
            if let Some(name) = &plugin.client_name {
                provider = provider.client_name(name);
            }
            if let Some(throws) = plugin.throw_on_error {
                provider = provider.throw_on_error(throws);
            }
            if plugin.name == "client" && style == SdkClientStyle::Namespaced {
                provider = provider.namespaced();
            }
            match plugin.name.as_str() {
                "models" => {
                    models.insert(id, provider.models_handle());
                }
                "transport" => {
                    transports.insert(id, provider.transport_handle());
                }
                "operations" => {
                    operations.insert(id, provider.operations_handle());
                }
                _ => {}
            }
            providers.push((plugin, provider));
        }
    }
    for (plugin, mut provider) in providers {
        for (role, id) in &plugin.uses {
            let allowed = match plugin.name.as_str() {
                "operations" => matches!(role.as_str(), "models" | "transport"),
                "client" => matches!(role.as_str(), "models" | "transport" | "operations"),
                _ => false,
            };
            ensure!(
                allowed,
                "{} provider does not accept uses.{role}",
                plugin.name
            );
            provider = match role.as_str() {
                "models" => provider.using_models(
                    *models
                        .get(id)
                        .with_context(|| format!("no model provider {id:?}"))?,
                ),
                "transport" => provider.using_transport(
                    *transports
                        .get(id)
                        .with_context(|| format!("no transport provider {id:?}"))?,
                ),
                "operations" => provider.using_operations(
                    *operations
                        .get(id)
                        .with_context(|| format!("no operation provider {id:?}"))?,
                ),
                other => bail!("unknown provider binding {other:?}"),
            };
        }
        output = output.with(provider);
    }
    for plugin in &package.plugins {
        let query_consumer = matches!(
            plugin.name.as_str(),
            "tanstack-react-query" | "tanstack-vue-query" | "swr"
        );
        let auxiliary_consumer =
            matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress");
        ensure!(
            plugin.layout.is_none() || query_consumer || auxiliary_consumer,
            "layout applies to query and auxiliary consumers only"
        );
        ensure!(
            query_consumer
                || (plugin.include_operations.is_none()
                    && plugin.operation_kinds.is_none()
                    && plugin.operation_names.is_none()),
            "operation selection/classification/names apply to query consumers only"
        );
        ensure!(
            plugin.fixture_options.is_none()
                || matches!(plugin.name.as_str(), "faker" | "msw" | "cypress"),
            "fixture_options applies to Faker/MSW/Cypress only"
        );
        ensure!(
            plugin.cypress_options.is_none() || plugin.name == "cypress",
            "cypress_options applies to Cypress only"
        );
        if plugin.max_file_bytes.is_some()
            && !matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress")
        {
            bail!("max_file_bytes applies to auxiliary consumers only");
        }
        if plugin.max_operations_per_file.is_some()
            && !matches!(
                plugin.name.as_str(),
                "tanstack-react-query" | "tanstack-vue-query" | "swr"
            )
        {
            bail!("max_operations_per_file applies to query consumers only");
        }
        let id = plugin.id.as_deref().unwrap_or(&plugin.name);
        let module = plugin.output.as_deref().map(|directory| {
            let stem = match plugin.name.as_str() {
                "tanstack-react-query" => "react-query",
                "tanstack-vue-query" => "vue-query",
                "cypress" => "api.cy",
                other => other,
            };
            Path::new(directory)
                .join(stem)
                .to_string_lossy()
                .into_owned()
        });
        if plugin.name == "oauth" {
            let mut consumer = ts::oauth();
            for (role, id) in &plugin.uses {
                ensure!(
                    role == "transport",
                    "OAuth accepts only a transport provider binding"
                );
                consumer = consumer.using_transport(
                    *transports
                        .get(id)
                        .with_context(|| format!("no transport provider {id:?}"))?,
                );
            }
            output = output.with(consumer);
        } else if plugin.name == "webhooks" {
            ensure!(
                plugin.uses.is_empty(),
                "webhooks does not accept provider bindings"
            );
            output = output.with(ts::webhooks());
        } else if plugin.name == "operation-tests" {
            let mut consumer = ts::operation_tests();
            for (role, id) in &plugin.uses {
                consumer = match role.as_str() {
                    "operations" => consumer.using_operations(
                        *operations
                            .get(id)
                            .with_context(|| format!("no operation provider {id:?}"))?,
                    ),
                    "transport" => consumer.using_transport(
                        *transports
                            .get(id)
                            .with_context(|| format!("no transport provider {id:?}"))?,
                    ),
                    "models" => consumer.using_models(
                        *models
                            .get(id)
                            .with_context(|| format!("no model provider {id:?}"))?,
                    ),
                    _ => bail!(
                        "operation-tests only accepts uses.models, uses.operations and uses.transport"
                    ),
                };
            }
            output = output.with(consumer);
        } else if matches!(
            plugin.name.as_str(),
            "tanstack-react-query" | "tanstack-vue-query" | "swr"
        ) {
            if plugin.clients_import.is_some() {
                bail!(
                    "native query plugins resolve imports through providers; use uses.operations instead of clients_import"
                );
            }
            let mut query = match plugin.name.as_str() {
                "tanstack-react-query" => composition::react_query(),
                "tanstack-vue-query" => composition::vue_query(),
                _ => composition::swr(),
            }
            .label(id);
            if let Some(count) = plugin.max_operations_per_file {
                query = query.max_operations_per_file(count);
            }
            if let Some(layout) = &plugin.layout {
                query = query.layout(layout.clone());
            }
            if let Some(ids) = &plugin.include_operations {
                query = query.include_operations(ids.clone());
            }
            for (operation, kind) in plugin.operation_kinds.iter().flatten() {
                query = query.operation_kind(
                    operation,
                    match kind.as_str() {
                        "query" => composition::QueryKind::Query,
                        "mutation" => composition::QueryKind::Mutation,
                        _ => bail!("operation kind must be query or mutation"),
                    },
                );
            }
            for (operation, name) in plugin.operation_names.iter().flatten() {
                query = query.operation_name(operation, name);
            }
            if let Some(module) = &module {
                query = query.output(module);
            }
            for (role, id) in &plugin.uses {
                if role != "operations" {
                    bail!("query consumer only accepts uses.operations");
                }
                query = query.using_operations(
                    *operations
                        .get(id)
                        .with_context(|| format!("no operation provider {id:?}"))?,
                );
            }
            output = output.with(query);
        } else if matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress") {
            let mut consumer = match plugin.name.as_str() {
                "zod" => composition::zod(),
                "faker" => composition::faker(),
                "msw" => composition::msw(),
                _ => composition::cypress(),
            }
            .label(id);
            if let Some(bytes) = plugin.max_file_bytes {
                consumer = consumer.max_file_bytes(bytes);
            }
            if let Some(layout) = &plugin.layout {
                consumer = consumer.layout(layout.clone());
            }
            if let Some(fixtures) = &plugin.fixture_options {
                consumer = consumer.fixture_options(fixtures.clone());
            }
            if let Some(options) = &plugin.cypress_options {
                consumer = consumer.cypress_options(options.clone());
            }
            if plugin.name == "cypress" && module.is_none() {
                consumer = consumer.output("api.cy");
            }
            if let Some(module) = &module {
                consumer = consumer.output(module);
            }
            for (role, id) in &plugin.uses {
                ensure!(
                    role != "operations" || matches!(plugin.name.as_str(), "msw" | "cypress"),
                    "{} consumer does not accept uses.operations",
                    plugin.name
                );
                consumer = match role.as_str() {
                    "models" => consumer.using_models(
                        *models
                            .get(id)
                            .with_context(|| format!("no model provider {id:?}"))?,
                    ),
                    "operations" => consumer.using_operations(
                        *operations
                            .get(id)
                            .with_context(|| format!("no operation provider {id:?}"))?,
                    ),
                    other => bail!("unknown auxiliary binding {other:?}"),
                };
            }
            output = output.with(consumer);
        }
    }
    Ok(output)
}

fn with_configured_middleware<L: poolster_core::engine::Language>(
    builder: Package<L>,
    package: &PackageConfig,
) -> Package<L> {
    let builder = builder.idempotency(package.idempotency.clone());
    let builder = if package.api_reference {
        builder.with(poolster_core::api_reference::<L>())
    } else {
        builder
    };
    package
        .resolved_middleware
        .iter()
        .cloned()
        .fold(builder, |builder, middleware| {
            builder.middleware(middleware)
        })
}

fn config_profiles(
    default_style: SdkClientStyle,
    packages: &[PackageConfig],
) -> Result<ProfileSet> {
    if packages.is_empty() {
        bail!("poolster.json must declare at least one package");
    }
    let mut profiles = ProfileSet::new(".").common(Common::default().client_style(default_style));
    for package in packages {
        ensure!(
            package
                .plugins
                .iter()
                .all(|plugin| plugin.open_unions.is_none()
                    || (package.language == "rust" && plugin.name == "sdk")),
            "open_unions is only supported by the Rust SDK plugin"
        );
        if !matches!(package.language.as_str(), "java" | "csharp" | "dotnet") {
            ensure!(
                package
                    .plugins
                    .iter()
                    .all(|plugin| plugin.preserve_presence.is_none()),
                "preserve_presence is only supported by Java and C# SDK plugins"
            );
        }
        if !matches!(package.language.as_str(), "typescript" | "ts") {
            ensure!(
                package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "oauth")
                    .all(|plugin| plugin.uses.is_empty()),
                "OAuth recipe bindings are supported only for TypeScript; use the native library API for other targets"
            );
        }
        if matches!(
            package.language.as_str(),
            "php" | "java" | "csharp" | "dotnet" | "elixir" | "ruby" | "swift"
        ) {
            for consumer in &package.plugins {
                if consumer.name == "operation-tests" {
                    ensure!(
                        consumer.uses.is_empty(),
                        "{} operation-tests recipes infer the bundled SDK; explicit uses bindings require the typed Rust API",
                        package.language
                    );
                }
            }
        }
        let style = match package.client_style.as_deref() {
            Some(style) => parse_style(Some(style))?,
            None => default_style,
        };
        profiles = match package.language.as_str() {
            "typescript" => profiles.package(with_configured_middleware(
                typescript_profile(package, style)?,
                package,
            )),
            "typescript-cli" => {
                has_only_known_plugins(package, &["cli"])?;
                let plugins = package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "cli")
                    .collect::<Vec<_>>();
                let [plugin] = plugins.as_slice() else {
                    bail!(
                        "typescript-cli package {:?} requires exactly one {{\"name\":\"cli\"}} plugin",
                        package.path
                    )
                };
                let mut generator = ts_cli::cli();
                if let Some(command_name) = &plugin.command_name {
                    generator = generator.command_name(command_name);
                }
                if let Some(base_url) = &plugin.base_url {
                    generator = generator.base_url(base_url);
                }
                if let Some(oauth) = &plugin.oauth {
                    if let Some(flow) = oauth.preferred_flow.as_deref()
                        && !matches!(flow, "device" | "browser")
                    {
                        bail!(
                            "typescript-cli oauth.preferred_flow must be \"device\" or \"browser\""
                        );
                    }
                    let mut settings = ts_cli::OAuthConfig::default().client_id(&oauth.client_id);
                    if let Some(scheme) = &oauth.security_scheme {
                        settings = settings.security_scheme(scheme);
                    }
                    if !oauth.scopes.is_empty() {
                        settings = settings.scopes(oauth.scopes.clone());
                    }
                    if let Some(flow) = &oauth.preferred_flow {
                        settings = settings.preferred_flow(flow);
                    }
                    if let Some(url) = &oauth.authorization_url {
                        settings = settings.authorization_url(url);
                    }
                    if let Some(url) = &oauth.device_authorization_url {
                        settings = settings.device_authorization_url(url);
                    }
                    if let Some(url) = &oauth.token_url {
                        settings = settings.token_url(url);
                    }
                    if let Some(uri) = &oauth.redirect_uri {
                        settings = settings.redirect_uri(uri);
                    }
                    generator = generator.oauth(settings);
                }
                let package_builder =
                    ts_cli::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(with_configured_middleware(
                    package_builder.with(generator),
                    package,
                ))
            }
            "rust-cli" => {
                has_only_known_plugins(package, &["cli"])?;
                let plugins = package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "cli")
                    .collect::<Vec<_>>();
                let [plugin] = plugins.as_slice() else {
                    bail!(
                        "rust-cli package {:?} requires exactly one {{\"name\":\"cli\"}} plugin",
                        package.path
                    )
                };
                let mut generator = rust_cli::cli();
                if let Some(command_name) = &plugin.command_name {
                    generator = generator.command_name(command_name);
                }
                if let Some(base_url) = &plugin.base_url {
                    generator = generator.base_url(base_url);
                }
                let package_builder =
                    rust_cli::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(with_configured_middleware(
                    package_builder.with(generator),
                    package,
                ))
            }
            "rust" => {
                has_only_known_plugins(package, &["sdk", "operation-tests", "webhooks", "oauth"])?;
                let package_builder =
                    rust::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let open_unions = package
                    .plugins
                    .iter()
                    .find(|plugin| plugin.name == "sdk")
                    .and_then(|plugin| plugin.open_unions)
                    .unwrap_or(false);
                let open_enums = package
                    .plugins
                    .iter()
                    .find(|plugin| plugin.name == "sdk")
                    .and_then(|plugin| plugin.open_enums)
                    .unwrap_or(false);
                let mut package_builder = package_builder
                    .open_unions(open_unions)
                    .open_enums(open_enums)
                    .with(rust::sdk());
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(rust::operation_tests());
                }
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(rust::webhooks());
                }
                if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
                    package_builder = package_builder.with(rust::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "go" => {
                has_only_known_plugins(package, &["sdk", "operation-tests", "webhooks", "oauth"])?;
                let plugin = sdk_plugin(package)?;
                let mut sdk = go::sdk();
                if let Some(jobs) = plugin.jobs {
                    sdk = sdk.jobs(jobs);
                }
                let package_builder =
                    go::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let mut package_builder = package_builder.with(sdk);
                for consumer in &package.plugins {
                    package_builder = match consumer.name.as_str() {
                        "operation-tests" => package_builder.with(go::operation_tests()),
                        "webhooks" => package_builder.with(go::webhooks()),
                        "oauth" => package_builder.with(go::oauth()),
                        _ => package_builder,
                    };
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "python" => {
                has_only_known_plugins(
                    package,
                    &["sdk", "webhooks", "roundtrips", "operation-tests"],
                )?;
                let package_builder =
                    python::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let plugin = sdk_plugin(package)?;
                let mut package_builder = package_builder
                    .open_enums(plugin.open_enums.unwrap_or(false))
                    .with(python::sdk().async_client(plugin.async_client.unwrap_or(false)));
                for consumer in &package.plugins {
                    match consumer.name.as_str() {
                        "webhooks" => package_builder = package_builder.with(python::webhooks()),
                        "roundtrips" => {
                            package_builder = package_builder.with(python::roundtrips())
                        }
                        "operation-tests" => {
                            package_builder = package_builder.with(python::operation_tests())
                        }
                        _ => {}
                    }
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "php" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    php::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let mut package_builder = package_builder.with(php::sdk());
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(php::webhooks());
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(php::operation_tests());
                }
                if package.plugins.iter().any(|p| p.name == "oauth") {
                    package_builder = package_builder.with(php::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "symfony" => {
                has_only_known_plugins(package, &["sdk"])?;
                let plugin = sdk_plugin(package)?;
                let package_builder =
                    symfony::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let package_builder = if let Some(sdk_package) = &plugin.sdk_package {
                    package_builder.sdk_package(sdk_package)
                } else {
                    package_builder
                };
                profiles.package(with_configured_middleware(
                    package_builder.with(symfony::sdk()),
                    package,
                ))
            }
            "java" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    java::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let plugin = sdk_plugin(package)?;
                let mut package_builder = package_builder.with(
                    java::sdk()
                        .open_enums(plugin.open_enums.unwrap_or(false))
                        .preserve_presence(plugin.preserve_presence.unwrap_or(false)),
                );
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(java::webhooks());
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(java::operation_tests());
                }
                if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
                    package_builder = package_builder.with(java::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "csharp" | "dotnet" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    csharp::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let plugin = sdk_plugin(package)?;
                let mut package_builder = package_builder.with(
                    csharp::sdk()
                        .open_enums(plugin.open_enums.unwrap_or(false))
                        .preserve_presence(plugin.preserve_presence.unwrap_or(false)),
                );
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(csharp::webhooks());
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(csharp::operation_tests());
                }
                if package.plugins.iter().any(|plugin| plugin.name == "oauth") {
                    package_builder = package_builder.with(csharp::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "elixir" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    elixir::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let mut package_builder = package_builder.with(elixir::sdk());
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(elixir::webhooks());
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(elixir::operation_tests());
                }
                if package.plugins.iter().any(|p| p.name == "oauth") {
                    package_builder = package_builder.with(elixir::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "ruby" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    ruby::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let mut package_builder = package_builder.with(ruby::sdk());
                for consumer in &package.plugins {
                    if consumer.name == "webhooks" {
                        package_builder = package_builder.with(ruby::webhooks());
                    }
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(ruby::operation_tests());
                }
                if package.plugins.iter().any(|p| p.name == "oauth") {
                    package_builder = package_builder.with(ruby::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "swift" => {
                has_only_known_plugins(package, &["sdk", "webhooks", "operation-tests", "oauth"])?;
                let package_builder =
                    swift::package(&package.path).common(configured_common(style, package));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                let plugin = sdk_plugin(package)?;
                let mut package_builder = package_builder
                    .with(swift::sdk().open_enums(plugin.open_enums.unwrap_or(false)));
                if package.plugins.iter().any(|p| p.name == "webhooks") {
                    package_builder = package_builder.with(swift::webhooks());
                }
                if package.plugins.iter().any(|p| p.name == "operation-tests") {
                    package_builder = package_builder.with(swift::operation_tests());
                }
                if package.plugins.iter().any(|p| p.name == "oauth") {
                    package_builder = package_builder.with(swift::oauth());
                }
                profiles.package(with_configured_middleware(package_builder, package))
            }
            "postman" => {
                has_only_known_plugins(package, &["collection", "environment"])?;
                let mut builder =
                    postman::package(&package.path).common(configured_common(style, package));
                if let Some(name) = &package.name {
                    builder = builder.name(name);
                }
                ensure!(
                    package
                        .plugins
                        .iter()
                        .filter(|plugin| plugin.name == "collection")
                        .count()
                        == 1,
                    "postman package requires exactly one collection plugin"
                );
                ensure!(
                    package
                        .plugins
                        .iter()
                        .filter(|plugin| plugin.name == "environment")
                        .count()
                        <= 1,
                    "postman package accepts one environment plugin"
                );
                for plugin in &package.plugins {
                    ensure!(
                        plugin.id.is_none() && plugin.uses.is_empty(),
                        "Postman recipe handles are not exposed yet; use the native plugin API"
                    );
                    if plugin.name == "collection" {
                        let mut collection = postman::collection()
                            .strict(plugin.strict.unwrap_or(true))
                            .group_by_tag(plugin.group_by_tag.unwrap_or(true))
                            .split_by_group(plugin.split_by_group.unwrap_or(false));
                        if let Some(output) = &plugin.output {
                            collection = collection.output(output);
                        }
                        if let Some(base_url) = &plugin.base_url {
                            builder = builder.base_url(base_url);
                        }
                        builder = builder.with(collection);
                    } else {
                        let mut environment = postman::environment();
                        if let Some(output) = &plugin.output {
                            environment = environment.output(output);
                        }
                        builder = builder.with(environment);
                    }
                }
                profiles.package(with_configured_middleware(builder, package))
            }
            "terraform" => {
                has_only_known_plugins(package, &["provider", "release-scaffold"])?;
                let plugins = package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "provider")
                    .collect::<Vec<_>>();
                let [plugin] = plugins.as_slice() else {
                    bail!("terraform package requires exactly one provider plugin")
                };
                ensure!(
                    plugin.id.is_none() && plugin.uses.is_empty(),
                    "Terraform recipe handles are not exposed yet; use the native plugin API"
                );
                let mut builder =
                    terraform::package(&package.path).common(configured_common(style, package));
                if let Some(module) = &plugin.module {
                    builder = builder.module(module);
                }
                if let Some(name) = plugin.provider_name.as_ref().or(package.name.as_ref()) {
                    builder = builder.provider_name(name);
                }
                if let Some(namespace) = &plugin.registry_namespace {
                    builder = builder.registry_namespace(namespace);
                }
                let mut provider = terraform::provider()
                    .infer(plugin.infer.unwrap_or(true))
                    .data_sources(plugin.data_sources.unwrap_or(false));
                for resource in &plugin.resources {
                    provider = provider.resource(resource.clone());
                }
                builder = builder.with(provider);
                if package.plugins.iter().any(|p| p.name == "release-scaffold") {
                    builder = builder.with(terraform::release_scaffold());
                }
                profiles.package(with_configured_middleware(builder, package))
            }
            "mock" => {
                has_only_known_plugins(package, &["server"])?;
                let servers = package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "server")
                    .collect::<Vec<_>>();
                let [server] = servers.as_slice() else {
                    bail!(
                        "mock package {:?} requires exactly one server plugin",
                        package.path
                    )
                };
                let mut server_builder = mock::server();
                if let Some(image) = &server.image {
                    server_builder = server_builder.image(image);
                }
                if let Some(port) = server.port {
                    server_builder = server_builder.port(port);
                }
                profiles.package(with_configured_middleware(
                    mock::package(&package.path).with(server_builder),
                    package,
                ))
            }
            "artifacts" => {
                has_only_known_plugins(package, &["redoc", "mcp"])?;
                if package.plugins.is_empty() {
                    bail!(
                        "artifacts package {:?} must declare at least one plugin",
                        package.path
                    );
                }
                // This package is a typed output-root reservation. ReDoc and MCP files
                // are added by the artifact pass after SDK packages are composed.
                profiles
                    .package(ts::package(&package.path).common(configured_common(style, package)))
            }
            other => bail!(
                "unknown config language {other:?}; use typescript, typescript-cli, rust, rust-cli, go, python, php, symfony, java, csharp, dotnet (legacy alias), elixir, ruby, swift, postman, terraform, mock, or artifacts"
            ),
        };
    }
    Ok(profiles)
}

fn config_path(base: &Path, value: PathBuf) -> PathBuf {
    if value.is_absolute() {
        value
    } else {
        base.join(value)
    }
}

fn resolve_config_input(base: &Path, input: OpenApiInput) -> OpenApiInput {
    match input {
        OpenApiInput::Path(path) if remote_spec_url(&path).is_some() => OpenApiInput::Path(path),
        OpenApiInput::Path(path) => OpenApiInput::Path(config_path(base, path)),
        OpenApiInput::Remote(remote) => OpenApiInput::Remote(remote),
    }
}

fn generate_from_config(
    path: &Path,
    color: ColorChoice,
    check: bool,
    json_changes: bool,
) -> Result<()> {
    if path
        .extension()
        .is_some_and(|extension| extension == "yml" || extension == "yaml")
        || !path.exists() && path == Path::new("poolster.json")
    {
        return migration::generate(
            if path.exists() { path } else { Path::new(".") },
            color,
            check,
            json_changes,
        );
    }
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("cannot read Poolster config {}", path.display()))?;
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("read Poolster config {}", path.display()))?;
    let mut config: ProjectConfig = serde_json::from_str(&source)
        .with_context(|| format!("parse Poolster JSON config {}", path.display()))?;
    let base = path.parent().expect("config path has parent");
    if config.output.path.as_os_str().is_empty() {
        bail!("output.path cannot be empty");
    }
    let has_input = config.openapi.input.is_some();
    let has_artifacts = config.openapi.artifacts.is_some();
    if has_input == has_artifacts {
        bail!("openapi must set exactly one of input or artifacts");
    }
    validate_path_selection(&config.openapi.paths)?;
    config_defaults::apply(&mut config)?;
    let style = parse_style(config.defaults.client_style.as_deref())?;
    let output = config_path(base, config.output.path.clone());
    for package in &mut config.packages {
        GeneratedFile::new(&package.path, "")?;
        package.resolved_customizations = package
            .customizations
            .iter()
            .map(|code| code.load(base))
            .collect::<Result<Vec<_>>>()?;
        package.resolved_middleware = package
            .middleware
            .iter()
            .map(|middleware| middleware.load(base))
            .collect::<Result<Vec<_>>>()?;
        if package.release.is_some() && package.version.is_none() {
            let metadata_path = output
                .join(&package.path)
                .join(poolster_core::release::PACKAGE_METADATA_PATH);
            if metadata_path.exists() {
                let canonical_root = std::fs::canonicalize(&output)?;
                let canonical_metadata = std::fs::canonicalize(&metadata_path)?;
                if !canonical_metadata.starts_with(&canonical_root) {
                    bail!("release metadata escapes output root");
                }
                let previous: poolster_core::release::PackageMetadata =
                    serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
                previous.validate()?;
                package.version = Some(previous.version);
            } else if let Some(release) = &package.release {
                if !release.version.is_empty() {
                    package.version = Some(release.version.clone());
                }
            }
        }
    }
    let options = Generate {
        source: config
            .openapi
            .input
            .map(|input| resolve_config_input(base, input)),
        artifacts: config
            .openapi
            .artifacts
            .map(|artifacts| config_path(base, artifacts)),
        config: None,
        config_packages: Some(config.packages),
        output: config_path(base, config.output.path),
        languages: Vec::new(),
        name: config.openapi.name,
        version: config.openapi.version,
        style,
        raw: false,
        typescript_transport: None,
        client_name: None,
        compiler: config
            .openapi
            .compiler
            .map(|compiler| config_path(base, compiler)),
        path_selection: config.openapi.paths,
        config_sha256: Some(sha256(source.as_bytes())),
        source_sha256: None,
        jobs: 0,
        color,
        check,
        json_changes,
    };
    generate(options)
}

fn init_config(init: Init) -> Result<()> {
    if init.config.exists() {
        bail!(
            "refusing to overwrite {}; edit it or choose --config <new-file>",
            init.config.display()
        );
    }
    let document = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/poolster.schema.json",
        "openapi": {
            "input": init.input,
            "name": init.name,
            "version": init.version,
        },
        "output": { "path": init.output },
        "defaults": { "client_style": "namespaced" },
        "packages": [
            {
                "language": "typescript",
                "path": "typescript",
                "name": "@acme/api",
                "plugins": [
                    { "name": "sdk", "transport": "fetch", "client_name": "Api" },
                    { "name": "zod" },
                    { "name": "tanstack-react-query" },
                    { "name": "msw" }
                ]
            },
            {
                "language": "go",
                "path": "go",
                "plugins": [{ "name": "sdk", "jobs": 4 }]
            }
        ]
    });
    if let Some(parent) = init
        .config
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create config directory {}", parent.display()))?;
    }
    std::fs::write(
        &init.config,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )
    .with_context(|| format!("write {}", init.config.display()))?;
    println!(
        "Created {}. Edit its package plugins, then run poolster generate.",
        init.config.display()
    );
    Ok(())
}

fn artifact_options(package: &PackageConfig, plugin: &PluginConfig) -> ArtifactOptions {
    let output_dir = match plugin.output.as_deref() {
        Some(output) => Path::new(&package.path)
            .join(output)
            .to_string_lossy()
            .into_owned(),
        None => package.path.clone(),
    };
    ArtifactOptions {
        output_dir: Some(output_dir),
        max_file_bytes: plugin.max_file_bytes.unwrap_or(128 * 1024),
        clients_import: plugin
            .clients_import
            .clone()
            .unwrap_or_else(|| "./clients".into()),
        group_by_tag: plugin.group_by_tag.unwrap_or(true),
        openapi_spec: plugin.openapi_spec.clone(),
        title: plugin.title.clone(),
        ..Default::default()
    }
}

fn append_artifact_files(tree: &mut GeneratedTree, files: Vec<GeneratedFile>) -> Result<()> {
    for file in files {
        tree.insert(file)?;
    }
    Ok(())
}

fn append_config_artifacts(
    api: &Api,
    tree: &mut GeneratedTree,
    packages: &[PackageConfig],
) -> Result<()> {
    for package in packages {
        if package.language == "typescript" && has_typescript_provider(package) {
            continue;
        }
        if package.language != "typescript" && package.language != "artifacts" {
            continue;
        }
        for plugin in &package.plugins {
            if plugin.name == "sdk" || plugin.name == "server" || plugin.name == "cli" {
                continue;
            }
            let options = artifact_options(package, plugin);
            match (package.language.as_str(), plugin.name.as_str()) {
                ("typescript", "zod") => {
                    append_artifact_files(tree, TypeScriptZod.generate(api, &options)?)?
                }
                ("typescript", "tanstack-react-query") => {
                    append_artifact_files(tree, TypeScriptReactQuery.generate(api, &options)?)?
                }
                ("typescript", "tanstack-vue-query") => {
                    append_artifact_files(tree, TypeScriptVueQuery.generate(api, &options)?)?
                }
                ("typescript", "swr") => {
                    append_artifact_files(tree, TypeScriptSwr.generate(api, &options)?)?
                }
                ("typescript", "faker") => {
                    append_artifact_files(tree, TypeScriptFaker.generate(api, &options)?)?
                }
                ("typescript", "msw") => {
                    append_artifact_files(tree, TypeScriptMsw.generate(api, &options)?)?
                }
                ("typescript", "cypress") => {
                    append_artifact_files(tree, TypeScriptCypress.generate(api, &options)?)?
                }
                ("artifacts", "redoc") => {
                    append_artifact_files(tree, ReDoc.generate(api, &options)?)?
                }
                ("artifacts", "mcp") => {
                    append_artifact_files(tree, McpToolManifest.generate(api, &options)?)?
                }
                _ => bail!(
                    "plugin {:?} is not available for config language {:?}",
                    plugin.name,
                    package.language
                ),
            }
        }
    }
    Ok(())
}

fn add_typescript_artifact_dependencies(
    tree: &mut GeneratedTree,
    packages: &[PackageConfig],
) -> Result<()> {
    for package in packages {
        if package.language != "typescript" {
            continue;
        }
        if package.plugins.iter().any(|plugin| {
            matches!(
                plugin.name.as_str(),
                "sdk" | "models" | "transport" | "operations" | "client"
            )
        }) {
            continue;
        }
        let dependencies = package
            .plugins
            .iter()
            .filter_map(|plugin| match plugin.name.as_str() {
                "zod" => Some(("zod", "^4.0.0")),
                "tanstack-react-query" => Some(("@tanstack/react-query", "^5.0.0")),
                "tanstack-vue-query" => Some(("@tanstack/vue-query", "^5.0.0")),
                "swr" => Some(("swr", "^2.0.0")),
                "faker" => Some(("@faker-js/faker", "^9.0.0")),
                "msw" => Some(("msw", "^2.0.0")),
                _ => None,
            })
            .collect::<Vec<_>>();
        let dev_dependencies = package
            .plugins
            .iter()
            .filter_map(|plugin| match plugin.name.as_str() {
                // Cypress is test-only, but the generated `.cy.ts` file is
                // included by the package's strict TypeScript build. Owning
                // this type dependency makes an explicitly selected Cypress
                // plugin compile without asking consumers to guess it.
                "cypress" => Some(("cypress", "^15.0.0")),
                _ => None,
            })
            .collect::<Vec<_>>();
        if dependencies.is_empty() && dev_dependencies.is_empty() {
            continue;
        }
        let path = Path::new(".").join(&package.path).join("package.json");
        let Some(manifest) = tree.get(&path).map(str::to_owned) else {
            // Artifact-only output is supported for an existing project. In
            // that case Poolster does not own a package manifest to mutate.
            continue;
        };
        let mut manifest: serde_json::Value = serde_json::from_str(&manifest)
            .with_context(|| format!("parse generated TypeScript manifest {}", path.display()))?;
        let object = manifest
            .as_object_mut()
            .expect("Poolster TypeScript manifests are JSON objects");
        for (field, dependencies) in [
            ("dependencies", dependencies),
            ("devDependencies", dev_dependencies),
        ] {
            let entries = object
                .entry(field)
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .with_context(|| {
                    format!("generated TypeScript manifest {field} must be an object")
                })?;
            for (name, version) in dependencies {
                if let Some(existing) = entries.get(name).and_then(serde_json::Value::as_str) {
                    if existing != version {
                        bail!(
                            "generated TypeScript manifest {} already declares {name} as {existing}, not {version}",
                            path.display()
                        );
                    }
                }
                entries.insert(name.into(), serde_json::Value::String(version.into()));
            }
        }
        tree.replace(GeneratedFile::new(
            path,
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?)?;
    }
    Ok(())
}

fn generate(mut options: Generate) -> Result<()> {
    if let Some(config) = &options.config {
        return generate_from_config(config, options.color, options.check, options.json_changes);
    }
    let reporter = Reporter::new(options.color);
    reporter.started();
    let temporary;
    let artifacts = if let Some(artifacts) = &options.artifacts {
        artifacts.as_path()
    } else {
        let source = options.source.as_ref().expect("validated source");
        temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
        let remote = match source {
            OpenApiInput::Remote(remote) => Some(remote.clone()),
            OpenApiInput::Path(path) => remote_spec_url(path).map(|url| RemoteInput {
                url: url.to_owned(),
                headers: BTreeMap::new(),
                auth: None,
            }),
        };
        let compiler_source_url = remote.as_ref().map(compiler_source_origin).transpose()?;
        let source = if let Some(remote) = remote {
            let downloaded = temporary.path().join("openapi.downloaded.yaml");
            reporter.downloading(&remote.url);
            let started = Instant::now();
            download_openapi(&remote, &downloaded)?;
            reporter.phase("Download", started.elapsed());
            options.source_sha256 = Some(sha256_file(&downloaded)?);
            downloaded
        } else {
            let OpenApiInput::Path(source) = source else {
                unreachable!("remote inputs were handled above")
            };
            let source = std::fs::canonicalize(source)
                .with_context(|| format!("cannot read OpenAPI source {}", source.display()))?;
            if !source.is_file() {
                bail!("OpenAPI source must be a file")
            }
            options.source_sha256 = Some(sha256_file(&source)?);
            source
        };
        let helper = compiler_path(options.compiler.clone())?;
        reporter.compiling(&source);
        let started = Instant::now();
        let mut compiler = Command::new(&helper);
        compiler.arg("--out").arg(temporary.path());
        if let Some(source_url) = &compiler_source_url {
            compiler.arg("--source-url").arg(source_url);
        }
        let status = compiler
            .arg(&source)
            .status()
            .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
        if !status.success() {
            bail!("OpenAPI compiler failed ({status}); no SDK files were written")
        }
        reporter.phase("OpenAPI", started.elapsed());
        // The compiler hashes the complete local reference closure. A root-only
        // digest would miss changes in referenced files in generation provenance.
        let source_manifest = temporary.path().join("source.json");
        if source_manifest.is_file() {
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&source_manifest).context("read compiler source manifest")?,
            )
            .context("decode compiler source manifest")?;
            let digest = manifest
                .get("sha256")
                .and_then(serde_json::Value::as_str)
                .filter(|value| {
                    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .context("compiler source manifest must contain a SHA-256 digest")?;
            options.source_sha256 = Some(digest.to_owned());
        }
        temporary.path()
    };
    write_sdk(artifacts, &options, &reporter)
}

fn write_sdk(artifacts: &Path, options: &Generate, reporter: &Reporter) -> Result<()> {
    let started = Instant::now();
    let api = poolster_core::adapter::openapi_sidecar::load_operations(
        artifacts,
        options.name.clone(),
        options.version.clone(),
    )?;
    if let Some(report) = api.annotations.get("poolster.vendor.report") {
        if let Some(manual) = report.get("manual").and_then(serde_json::Value::as_array) {
            for diagnostic in manual {
                if let Some(message) = diagnostic.as_str() {
                    eprintln!("poolster compatibility: {message}");
                }
            }
        }
    }
    let api = slice_api_paths(api, &options.path_selection)?;
    let security_schemes =
        poolster_core::adapter::openapi_sidecar::load_security_schemes(artifacts)?;
    let mut tree = poolster::generate_with_security_catalog(
        &api,
        profiles(options)?,
        Some(&security_schemes),
    )?;
    if let Some(packages) = &options.config_packages {
        append_config_artifacts(&api, &mut tree, packages)?;
        add_typescript_artifact_dependencies(&mut tree, packages)?;
        for package in packages {
            if let Some(configured) = &package.release {
                let mut metadata = configured.clone();
                metadata.language = package.language.clone();
                if metadata.name.is_empty() {
                    metadata.name = package.name.clone().unwrap_or_else(|| package.path.clone());
                }
                metadata.version = package
                    .version
                    .clone()
                    .unwrap_or_else(|| options.version.clone());
                let path =
                    Path::new(&package.path).join(poolster_core::release::PACKAGE_METADATA_PATH);
                tree.insert(GeneratedFile::new(&path, metadata.to_json()?)?)?;
                tree.set_owner(&path, format!("package-metadata:{}", package.path))?;
            }
        }
        let customizations = packages
            .iter()
            .flat_map(|package| {
                package
                    .resolved_customizations
                    .iter()
                    .map(|code| code.prefixed(Path::new(&package.path)))
            })
            .collect::<Vec<_>>();
        poolster_core::customization::apply_code_customizations(&mut tree, &customizations)?;
    }
    reporter.generated(options, started.elapsed());
    let started = Instant::now();
    tree.insert(GeneratedFile::new(
        GENERATION_LOCK_PATH,
        generation_lock(artifacts, options, &api)?,
    )?)?;
    let changes = tree.check(&options.output)?;
    if options.json_changes {
        println!("{}", serde_json::to_string(&changes)?);
    } else if options.check {
        for path in &changes.added {
            println!("added {}", path.display());
        }
        for path in &changes.modified {
            println!("modified {}", path.display());
        }
        for path in &changes.removed {
            println!("removed {}", path.display());
        }
        if changes.is_empty() {
            println!("Generated output is up to date.");
        }
    }
    if options.check {
        if !changes.is_empty() {
            bail!("generated output has drift");
        }
        return Ok(());
    }
    tree.write_to(&options.output)?;
    reporter.phase("Writing files", started.elapsed());
    reporter.completed(
        configured_plugin_count(options),
        tree.iter().count() + 1,
        &options.output,
    );
    Ok(())
}

fn auth(options: Auth) -> Result<()> {
    match options {
        Auth::Login { profile, token_env } => {
            credentials::login(&profile, &token_env)?;
            println!("Saved auth profile {profile:?}; token stays in ${token_env}.");
        }
        Auth::Logout { profile } => {
            if credentials::logout(&profile)? {
                println!("Removed auth profile {profile:?}.");
            } else {
                println!("No auth profile named {profile:?} was configured.");
            }
        }
        Auth::Status => {
            let profiles = credentials::profiles()?;
            if profiles.is_empty() {
                println!("No Poolster auth profiles are configured.");
            } else {
                for (profile, token_env) in profiles {
                    println!("{profile}\t${token_env}");
                }
            }
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(sha256(&bytes))
}

fn sha256_directory(path: &Path) -> Result<String> {
    fn visit(root: &Path, directory: &Path, hasher: &mut Sha256) -> Result<()> {
        let mut entries = std::fs::read_dir(directory)
            .with_context(|| format!("read compiler artifacts {}", directory.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("read compiler artifacts {}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("inspect compiler artifact {}", path.display()))?;
            if file_type.is_dir() {
                visit(root, &path, hasher)?;
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("artifact child remains below artifact root");
                hasher.update(relative.to_string_lossy().as_bytes());
                hasher.update([0]);
                hasher.update(
                    std::fs::read(&path)
                        .with_context(|| format!("read compiler artifact {}", path.display()))?,
                );
                hasher.update([0]);
            }
        }
        Ok(())
    }

    let mut hasher = Sha256::new();
    visit(path, path, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_path_selection(selection: &PathSelection) -> Result<()> {
    for (kind, patterns) in [
        ("include path", &selection.include),
        ("exclude path", &selection.exclude),
    ] {
        for pattern in patterns {
            if pattern.trim().is_empty() || !pattern.starts_with('/') {
                bail!("{kind} pattern must start with '/' and cannot be empty, got {pattern:?}");
            }
        }
    }
    Ok(())
}

fn slice_api_paths(mut api: Api, selection: &PathSelection) -> Result<Api> {
    validate_path_selection(selection)?;
    api.operations.retain(|operation| {
        let included = selection.include.is_empty()
            || selection
                .include
                .iter()
                .any(|pattern| poolster_core::wildcard_matches(pattern, &operation.path));
        included
            && !selection
                .exclude
                .iter()
                .any(|pattern| poolster_core::wildcard_matches(pattern, &operation.path))
    });
    if api.operations.is_empty() {
        bail!("path selection matched no OpenAPI operations; adjust paths.include or paths.exclude")
    }
    Ok(api)
}

fn serve_mcp(options: Mcp) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let status = Command::new(&helper)
        .arg("--out")
        .arg(temporary.path())
        .arg(&source)
        // MCP reserves stdout for newline-delimited JSON-RPC messages. The
        // compiler's progress summary must never corrupt that transport.
        .stdout(Stdio::null())
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = poolster_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    mcp::serve(api, &options.base_url)
}

fn main() -> ExitCode {
    let action = match parse(env::args_os().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("poolster: {error:#}");
            return ExitCode::from(2);
        }
    };
    let result = match action {
        Action::Help => {
            print!("{HELP}");
            Ok(())
        }
        Action::Version => {
            println!("poolster {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Languages => {
            println!("{}", LANGUAGES.join("\n"));
            Ok(())
        }
        Action::Show(options) => show_command::show(options),
        Action::Update(options) => update(options),
        Action::Auth(options) => auth(options),
        Action::Discover(options) => openapi_sources::discover(options),
        Action::Download(options) => openapi_sources::download(options),
        Action::Init(init) => init_config(init),
        Action::Mcp(options) => serve_mcp(options),
        Action::McpGenerator => mcp::serve_generator(),
        Action::MockServe(options) => serve_mock(options),
        Action::Check(options) => check_command::check(options),
        Action::Generate(options) => generate(*options),
        Action::Sdk(options) => sdk_automation::run(options),
        Action::Eject(arguments) => eject::run(arguments),
        Action::Migrate(arguments) => migration::run(arguments),
        Action::Contract(arguments) => contract::run(arguments),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("poolster: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
