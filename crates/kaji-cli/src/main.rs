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
use generation_artifacts::{add_typescript_artifact_dependencies, append_config_artifacts};
use generation_config::{config_path, generate_from_config, init_config};
use generation_direct::generate;
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
use profile_config::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use typescript_profile::typescript_profile;
#[cfg(test)]
use update_command::replay_input_is_unchanged;
use update_command::update;

mod check_command;
mod check_rules;
mod cli_args;
mod config_defaults;
mod config_profile_cli;
mod config_profile_other;
mod config_profile_sdk;
mod contract;
mod credentials;
mod eject;
mod generation_artifacts;
mod generation_config;
mod generation_direct;
mod generation_lock;
mod mcp;
mod migration;
mod mock_http;
mod mock_server;
mod openapi_sources;
mod profile_config;
mod registry;
mod sdk_automation;
mod sdk_doctor;
mod sdk_install;
mod sdk_status;
mod show_command;
mod typescript_profile;
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
