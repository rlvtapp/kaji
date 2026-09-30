use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use kaji::ts::artifacts::{
    ArtifactOptions, McpToolManifest, ReDoc, TypeScriptCypress, TypeScriptFaker, TypeScriptMsw,
    TypeScriptReactQuery, TypeScriptSwr, TypeScriptVueQuery, TypeScriptZod,
};
use kaji::{
    SdkClientStyle, csharp, dotnet, elixir, go, java, mock, php, prelude::*, python, ruby, rust,
    rust_cli, swift, ts, ts_cli,
};
use kaji_core::{Api, GeneratedFile, GeneratedTree};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod credentials;
mod mcp;
mod registry;

const HELP: &str = "Kaji — native multi-language OpenAPI SDK generator

Usage:
  kaji init [--config <file>] [--input <openapi-file>] [--output <directory>]
  kaji generate                         # reads ./kaji.json
  kaji generate --config <file>
  kaji generate <openapi-file> --output <directory> --language <target>...
  kaji generate --artifacts <directory> --output <directory> --language <target>...
  kaji mcp <openapi-file> --base-url <url>
  kaji mcp generator
  kaji mock serve <openapi-file> [--port <port>]
  kaji check <openapi-file> [--format human|json]
  kaji show <openapi-file> [--include-path <pattern>] [--exclude-path <pattern>]
  kaji update [--output <directory>] [--force]
  kaji auth <login|logout|status> ...
  kaji discover <query> [--limit <count>] [--format human|json]
  kaji download <api-id> --output <openapi-file> [--version <version>]
  kaji languages
  kaji --version

Config commands:
  init                                  Write a starter kaji.json; never overwrites it
  generate                              Read the config by default
      --config <file>                   Read a specific config file
      --color <mode>                    auto (default), always, or never

Generate options (direct mode):
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
      --openapi-compiler <file>       Override bundled kaji-openapi executable
      --include-path <pattern>        Generate only matching OpenAPI paths; repeatable
      --exclude-path <pattern>        Omit matching OpenAPI paths; repeatable
  -h, --help                          Show help

Targets: rust, rust-cli, typescript, typescript-cli, go, python, php, java, csharp, dotnet (legacy alias), elixir, ruby, swift

MCP commands:
  mcp                                   Serve an OpenAPI document as MCP tools over stdio
      --base-url <url>                  API origin used when a tool is called (required)
      --openapi-compiler <file>         Override bundled kaji-openapi executable
  mcp generator                         Serve Kaji generation controls as MCP tools over stdio

Mock commands:
  mock serve                            Serve OpenAPI-derived happy-path responses without Docker
      --port <port>                     Local port (default: 4010)
      --openapi-compiler <file>         Override bundled kaji-openapi executable

Contract commands:
  check                                 Find API-contract issues that make generated SDKs and CLIs awkward
      --openapi-compiler <file>         Override bundled kaji-openapi executable
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

Each target is written to its own subdirectory. Generated files are overwritten;
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
    "java",
    "csharp",
    "dotnet",
    "elixir",
    "ruby",
    "swift",
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
        eprintln!("\n{} {}", self.gold("$"), self.paint("1", "kaji generate"));
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

const MAX_OPENAPI_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
const MAX_API_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;

fn load_api_directory() -> Result<registry::Directory> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("kaji/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("configure API directory client")?;
    let response = client
        .get(registry::DIRECTORY_URL)
        .send()
        .context("download API directory")?
        .error_for_status()
        .context("download API directory")?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_API_DIRECTORY_BYTES)
    {
        bail!(
            "API directory is larger than the {} MiB download limit",
            MAX_API_DIRECTORY_BYTES / 1024 / 1024
        )
    }
    let mut document = Vec::new();
    response
        .take(MAX_API_DIRECTORY_BYTES + 1)
        .read_to_end(&mut document)
        .context("read API directory")?;
    if document.len() as u64 > MAX_API_DIRECTORY_BYTES {
        bail!(
            "API directory is larger than the {} MiB download limit",
            MAX_API_DIRECTORY_BYTES / 1024 / 1024
        )
    }
    let document = std::str::from_utf8(&document).context("API directory is not UTF-8")?;
    registry::Directory::parse(document)
}

fn discover(options: Discover) -> Result<()> {
    let directory = load_api_directory()?;
    let apis = directory.search(&options.query, options.limit);
    match options.format {
        DiscoverFormat::Json => println!("{}", serde_json::to_string_pretty(&apis)?),
        DiscoverFormat::Human => {
            if apis.is_empty() {
                println!("No OpenAPI directory entries matched {:?}.", options.query);
                return Ok(());
            }
            for api in apis {
                println!("{}  {}  {}", api.id, api.version, api.title);
                if let Some(description) = api.description {
                    println!("  {description}");
                }
                println!("  {}", api.openapi_url);
            }
        }
    }
    Ok(())
}

fn download(options: Download) -> Result<()> {
    if options.output.exists() {
        bail!(
            "refusing to overwrite existing file {}; choose a new --output path",
            options.output.display()
        )
    }
    let directory = load_api_directory()?;
    let api = directory.resolve(&options.id, options.version.as_deref())?;
    registry::validate_download(&api)?;
    let source = RemoteInput {
        url: api.openapi_url.clone(),
        headers: BTreeMap::new(),
        auth: None,
    };
    download_openapi(&source, &options.output)?;
    println!(
        "Downloaded {} version {} to {}",
        api.id,
        api.version,
        options.output.display()
    );
    Ok(())
}

fn remote_spec_url(source: &Path) -> Option<&str> {
    let source = source.to_str()?;
    (source.starts_with("https://") || source.starts_with("http://")).then_some(source)
}

fn download_openapi(source: &RemoteInput, destination: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("kaji/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("configure OpenAPI download client")?;
    let mut response = remote_request(&client, source)?
        .send()
        .with_context(|| format!("download OpenAPI document {}", source.url))?
        .error_for_status()
        .with_context(|| format!("download OpenAPI document {}", source.url))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_OPENAPI_DOWNLOAD_BYTES)
    {
        bail!(
            "OpenAPI document is larger than the {} MiB download limit",
            MAX_OPENAPI_DOWNLOAD_BYTES / 1024 / 1024
        );
    }
    let mut file = std::fs::File::create(destination)
        .with_context(|| format!("create downloaded OpenAPI file {}", destination.display()))?;
    let copied = std::io::copy(&mut response, &mut file)
        .with_context(|| format!("save downloaded OpenAPI document from {}", source.url))?;
    if copied > MAX_OPENAPI_DOWNLOAD_BYTES {
        bail!(
            "OpenAPI document is larger than the {} MiB download limit",
            MAX_OPENAPI_DOWNLOAD_BYTES / 1024 / 1024
        );
    }
    Ok(())
}

fn remote_request(
    client: &reqwest::blocking::Client,
    source: &RemoteInput,
) -> Result<reqwest::blocking::RequestBuilder> {
    let mut request = client.get(&source.url);
    for (name, value) in &source.headers {
        let value = value.resolve(&format!("header {name:?}"))?;
        request = request.header(name, value);
    }
    if let Some(auth) = &source.auth {
        request = match auth {
            RemoteAuth::Basic { username, password } => request.basic_auth(
                username.resolve("basic authentication username")?,
                Some(password.resolve("basic authentication password")?),
            ),
            RemoteAuth::Bearer { token } => {
                request.bearer_auth(token.resolve("bearer authentication token")?)
            }
        };
    }
    Ok(request)
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

/// Path filters intentionally use the same small glob language as Kaji's
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
                .with_context(|| format!("resolve Kaji auth profile {profile:?} for {field}")),
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageConfig {
    language: String,
    path: String,
    name: Option<String>,
    client_style: Option<String>,
    #[serde(default)]
    plugins: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginConfig {
    name: String,
    transport: Option<String>,
    surface: Option<String>,
    client_name: Option<String>,
    group_by_tag: Option<bool>,
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

fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = arguments.into_iter();
    let Some(command) = args.next() else {
        return Ok(Action::Help);
    };
    if command == "--help" || command == "-h" || command == "help" {
        return Ok(Action::Help);
    }
    if command == "--version" || command == "-V" {
        return Ok(Action::Version);
    }
    if command == "init" {
        return parse_init(args);
    }
    if command == "mcp" {
        return parse_mcp(args);
    }
    if command == "mock" {
        return parse_mock(args);
    }
    if command == "check" {
        return parse_check(args);
    }
    if command == "show" {
        return parse_show(args);
    }
    if command == "update" {
        return parse_update(args);
    }
    if command == "auth" {
        return parse_auth(args);
    }
    if command == "discover" {
        return parse_discover(args);
    }
    if command == "download" {
        return parse_download(args);
    }
    if command == "languages" {
        if args.next().is_some() {
            bail!("languages does not accept arguments")
        }
        return Ok(Action::Languages);
    }
    if command != "generate" {
        bail!("unknown command {:?}; run kaji --help", command)
    }
    let mut options = Generate {
        source: None,
        config: None,
        config_packages: None,
        artifacts: None,
        output: PathBuf::new(),
        languages: Vec::new(),
        name: "API".into(),
        version: "0.1.0".into(),
        style: SdkClientStyle::Namespaced,
        raw: false,
        typescript_transport: None,
        client_name: None,
        compiler: None,
        path_selection: PathSelection::default(),
        config_sha256: None,
        source_sha256: None,
        jobs: 0,
        color: ColorChoice::Auto,
    };
    while let Some(argument) = args.next() {
        let text = argument.to_string_lossy();
        if text == "--help" || text == "-h" {
            return Ok(Action::Help);
        }
        if !text.starts_with('-') {
            if options
                .source
                .replace(OpenApiInput::Path(PathBuf::from(argument)))
                .is_some()
            {
                bail!("only one OpenAPI source may be supplied")
            }
            continue;
        }
        let flag = text.as_ref();
        if !matches!(
            flag,
            "--output"
                | "-o"
                | "--language"
                | "-l"
                | "--name"
                | "--sdk-version"
                | "--client-style"
                | "--typescript-transport"
                | "--typescript-surface"
                | "--typescript-client-name"
                | "--jobs"
                | "--artifacts"
                | "--openapi-compiler"
                | "--include-path"
                | "--exclude-path"
                | "--config"
                | "--color"
        ) {
            bail!("unknown option {flag}; run kaji --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        match flag {
            "--output" | "-o" => options.output = value.into(),
            "--artifacts" => options.artifacts = Some(value.into()),
            "--openapi-compiler" => options.compiler = Some(value.into()),
            "--config" => options.config = Some(value.into()),
            _ => {
                let value = value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("{flag} requires UTF-8 text"))?;
                if value.trim().is_empty() {
                    bail!("{flag} cannot be empty")
                }
                match flag {
                    "--language" | "-l" => {
                        for target in value.split(',') {
                            let targets = if target == "all" {
                                SDK_LANGUAGES
                            } else {
                                std::slice::from_ref(&target)
                            };
                            for target in targets {
                                if !LANGUAGES.contains(target) {
                                    bail!("unknown target {target:?}; run kaji languages")
                                }
                                if !options.languages.iter().any(|existing| existing == target) {
                                    options.languages.push((*target).into());
                                }
                            }
                        }
                    }
                    "--name" => options.name = value,
                    "--sdk-version" => options.version = value,
                    "--typescript-transport" => {
                        options.typescript_transport = Some(TypeScriptTransport::parse(&value)?);
                    }
                    "--typescript-client-name" => options.client_name = Some(value),
                    "--include-path" => options.path_selection.include.push(value),
                    "--exclude-path" => options.path_selection.exclude.push(value),
                    "--color" => options.color = ColorChoice::parse(&value)?,
                    "--jobs" => {
                        options.jobs =
                            value.parse().context("--jobs must be a positive integer")?;
                        if options.jobs == 0 {
                            bail!("--jobs must be a positive integer")
                        }
                    }
                    "--client-style" => {
                        options.style = match value.as_str() {
                            "flat" => SdkClientStyle::Flat,
                            "namespaced" => SdkClientStyle::Namespaced,
                            _ => bail!("--client-style must be flat or namespaced"),
                        }
                    }
                    "--typescript-surface" => {
                        options.raw = match value.as_str() {
                            "raw" => true,
                            "client" => false,
                            _ => bail!("--typescript-surface must be client or raw"),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    let direct_mode = options.source.is_some()
        || options.artifacts.is_some()
        || !options.output.as_os_str().is_empty()
        || !options.languages.is_empty()
        || options.raw
        || options.typescript_transport.is_some()
        || options.client_name.is_some()
        || options.compiler.is_some()
        || !options.path_selection.include.is_empty()
        || !options.path_selection.exclude.is_empty()
        || options.jobs != 0;
    if options.config.is_some() || !direct_mode {
        if direct_mode {
            bail!("--config cannot be combined with direct generation options")
        }
        options
            .config
            .get_or_insert_with(|| PathBuf::from("kaji.json"));
        return Ok(Action::Generate(Box::new(options)));
    }
    if options.output.as_os_str().is_empty() {
        bail!("--output is required")
    }
    if options.languages.is_empty() {
        bail!("at least one --language is required")
    }
    if options.source.is_some() == options.artifacts.is_some() {
        bail!("supply exactly one OpenAPI file or --artifacts directory")
    }
    if options.artifacts.is_some() && options.compiler.is_some() {
        bail!("--openapi-compiler cannot be used with --artifacts")
    }
    validate_path_selection(&options.path_selection)?;
    if (options.raw || options.typescript_transport.is_some() || options.client_name.is_some())
        && !options
            .languages
            .iter()
            .any(|target| target == "typescript")
    {
        bail!("TypeScript options require a TypeScript target")
    }
    Ok(Action::Generate(Box::new(options)))
}

fn parse_discover(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut query = None;
    let mut limit = 20;
    let mut format = DiscoverFormat::Human;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--limit" => {
                let value = args.next().context("--limit requires a positive integer")?;
                limit = value
                    .to_string_lossy()
                    .parse()
                    .context("--limit requires a positive integer")?;
                if limit == 0 {
                    bail!("--limit requires a positive integer")
                }
            }
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => DiscoverFormat::Human,
                    "json" => DiscoverFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                }
            }
            value if value.starts_with('-') => bail!("unknown option {value}; run kaji --help"),
            _ => {
                if query
                    .replace(
                        argument
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("discover query must be UTF-8 text"))?,
                    )
                    .is_some()
                {
                    bail!("discover accepts exactly one query")
                }
            }
        }
    }
    let query = query.context("discover requires a query")?;
    if query.trim().is_empty() {
        bail!("discover query cannot be empty")
    }
    Ok(Action::Discover(Discover {
        query,
        limit,
        format,
    }))
}

fn parse_download(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut id = None;
    let mut version = None;
    let mut output = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--version" => {
                version = Some(
                    args.next()
                        .context("--version requires a value")?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--version requires UTF-8 text"))?,
                );
            }
            "--output" | "-o" => {
                output = Some(args.next().context("--output requires a file path")?.into());
            }
            value if value.starts_with('-') => bail!("unknown option {value}; run kaji --help"),
            _ => {
                if id
                    .replace(
                        argument
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("API id must be UTF-8 text"))?,
                    )
                    .is_some()
                {
                    bail!("download accepts exactly one API id")
                }
            }
        }
    }
    let id = id.context("download requires an API id")?;
    if id.trim().is_empty() {
        bail!("API id cannot be empty")
    }
    let output = output.context("download requires --output <openapi-file>")?;
    Ok(Action::Download(Download {
        id,
        version,
        output,
    }))
}

fn parse_show(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut source = None;
    let mut compiler = None;
    let mut paths = PathSelection::default();
    let mut format = ShowFormat::Human;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if !flag.starts_with('-') {
            if source.replace(argument.into()).is_some() {
                bail!("show accepts exactly one OpenAPI file")
            }
            continue;
        }
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--openapi-compiler" => {
                compiler = Some(
                    args.next()
                        .context("--openapi-compiler requires a value")?
                        .into(),
                );
            }
            "--include-path" => paths.include.push(
                args.next()
                    .context("--include-path requires a pattern")?
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--include-path requires UTF-8 text"))?,
            ),
            "--exclude-path" => paths.exclude.push(
                args.next()
                    .context("--exclude-path requires a pattern")?
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--exclude-path requires UTF-8 text"))?,
            ),
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => ShowFormat::Human,
                    "json" => ShowFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                }
            }
            "--json" => format = ShowFormat::Json,
            value => bail!("unknown show option {value}; run kaji --help"),
        }
    }
    validate_path_selection(&paths)?;
    Ok(Action::Show(Show {
        source: source.context("show requires an OpenAPI file")?,
        compiler,
        paths,
        format,
    }))
}

fn parse_update(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut output = PathBuf::from(".");
    let mut force = false;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--output" | "-o" => {
                output = args.next().context("--output requires a directory")?.into();
            }
            "--force" => force = true,
            value if value.starts_with('-') => {
                bail!("unknown update option {value}; run kaji --help")
            }
            _ => bail!("update accepts only --output and --force"),
        }
    }
    Ok(Action::Update(Update { output, force }))
}

fn parse_auth(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = args.into_iter();
    let command = args
        .next()
        .context("auth requires login, logout, or status")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("auth command must be UTF-8 text"))?;
    match command.as_str() {
        "status" => {
            if args.next().is_some() {
                bail!("auth status does not accept arguments")
            }
            Ok(Action::Auth(Auth::Status))
        }
        "logout" => {
            let profile = args
                .next()
                .context("auth logout requires a profile name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("auth profile must be UTF-8 text"))?;
            if args.next().is_some() {
                bail!("auth logout accepts exactly one profile name")
            }
            Ok(Action::Auth(Auth::Logout { profile }))
        }
        "login" => {
            let profile = args
                .next()
                .context("auth login requires a profile name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("auth profile must be UTF-8 text"))?;
            let flag = args
                .next()
                .context("auth login requires --token-env <name>")?;
            if flag != "--token-env" {
                bail!("auth login requires --token-env <name>")
            }
            let token_env = args
                .next()
                .context("--token-env requires an environment variable name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("--token-env requires UTF-8 text"))?;
            if args.next().is_some() {
                bail!("auth login accepts only --token-env <name>")
            }
            Ok(Action::Auth(Auth::Login { profile, token_env }))
        }
        "--help" | "-h" | "help" => Ok(Action::Help),
        _ => bail!("auth requires login, logout, or status"),
    }
}

fn parse_check(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut source = None;
    let mut compiler = None;
    let mut format = CheckFormat::Human;
    let mut severity_overrides = BTreeMap::new();
    let mut fail_on = CheckFailureThreshold::Error;
    let mut baseline = None;
    let mut write_baseline = None;
    let mut ignored_rules = BTreeSet::new();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("check accepts exactly one OpenAPI file")
            }
            continue;
        }
        match flag.as_ref() {
            "--openapi-compiler" => {
                compiler = Some(
                    args.next()
                        .context("--openapi-compiler requires a value")?
                        .into(),
                );
            }
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => CheckFormat::Human,
                    "json" => CheckFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                };
            }
            "--json" => format = CheckFormat::Json,
            "--severity" => {
                let value = args
                    .next()
                    .context("--severity requires <rule=warning|error>")?
                    .to_string_lossy()
                    .into_owned();
                let (rule, level) = value.split_once('=').with_context(|| {
                    "--severity requires <rule=warning|error>, for example --severity missing-operation-id=warning"
                })?;
                if !CHECK_RULES.contains(&rule) {
                    bail!("unknown check rule {rule:?}")
                }
                severity_overrides.insert(rule.into(), CheckSeverity::parse(level)?);
            }
            "--fail-on" => {
                fail_on = CheckFailureThreshold::parse(
                    &args
                        .next()
                        .context("--fail-on requires warning, error, or none")?
                        .to_string_lossy(),
                )?;
            }
            "--baseline" => {
                baseline = Some(args.next().context("--baseline requires a file")?.into());
            }
            "--write-baseline" => {
                write_baseline = Some(
                    args.next()
                        .context("--write-baseline requires a file")?
                        .into(),
                );
            }
            "--ignore" => {
                let rule = args
                    .next()
                    .context("--ignore requires a rule name")?
                    .to_string_lossy()
                    .into_owned();
                if !CHECK_RULES.contains(&rule.as_str()) {
                    bail!("unknown check rule {rule:?}")
                }
                ignored_rules.insert(rule);
            }
            _ => bail!("unknown check option {flag}; run kaji --help"),
        }
    }
    Ok(Action::Check(Check {
        source: source.context("check requires an OpenAPI file")?,
        compiler,
        format,
        severity_overrides,
        fail_on,
        baseline,
        write_baseline,
        ignored_rules,
    }))
}

fn parse_mcp(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let collected = args.into_iter().collect::<Vec<_>>();
    if collected.len() == 1 && collected[0] == "generator" {
        return Ok(Action::McpGenerator);
    }
    let mut source = None;
    let mut base_url = None;
    let mut compiler = None;
    let mut args = collected.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("mcp accepts exactly one OpenAPI file")
            }
            continue;
        }
        if !matches!(flag.as_ref(), "--base-url" | "--openapi-compiler") {
            bail!("unknown mcp option {flag}; run kaji --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        if flag == "--base-url" {
            let value = value
                .into_string()
                .map_err(|_| anyhow::anyhow!("--base-url requires UTF-8 text"))?;
            let parsed =
                reqwest::Url::parse(&value).context("--base-url must be an absolute URL")?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("--base-url must use http or https")
            }
            base_url = Some(value.trim_end_matches('/').to_owned());
        } else {
            compiler = Some(value.into());
        }
    }
    let source = source.context("mcp requires an OpenAPI file")?;
    let base_url = base_url.context("mcp requires --base-url")?;
    Ok(Action::Mcp(Mcp {
        source,
        base_url,
        compiler,
    }))
}

fn parse_mock(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        bail!("mock requires a subcommand; use kaji mock serve --help")
    };
    if command != "serve" {
        bail!("unknown mock subcommand {command:?}; use kaji mock serve")
    }
    let mut source = None;
    let mut port = 4010;
    let mut compiler = None;
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("mock serve accepts exactly one OpenAPI file")
            }
            continue;
        }
        if !matches!(flag.as_ref(), "--port" | "--openapi-compiler") {
            bail!("unknown mock serve option {flag}; run kaji --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        if flag == "--port" {
            port = value
                .into_string()
                .map_err(|_| anyhow::anyhow!("--port requires UTF-8 text"))?
                .parse()
                .context("--port must be a valid TCP port")?;
            if port == 0 {
                bail!("--port must be nonzero")
            }
        } else {
            compiler = Some(value.into());
        }
    }
    Ok(Action::MockServe(MockServe {
        source: source.context("mock serve requires an OpenAPI file")?,
        port,
        compiler,
    }))
}

fn parse_init(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut init = Init {
        config: PathBuf::from("kaji.json"),
        input: PathBuf::from("openapi.yaml"),
        output: PathBuf::from("generated"),
        name: default_api_name(),
        version: default_sdk_version(),
    };
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !matches!(
            flag.as_ref(),
            "--config" | "--input" | "--output" | "--name" | "--sdk-version"
        ) {
            bail!("unknown init option {flag}; run kaji --help");
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        let value = value
            .into_string()
            .map_err(|_| anyhow::anyhow!("{flag} requires UTF-8 text"))?;
        if value.trim().is_empty() {
            bail!("{flag} cannot be empty");
        }
        match flag.as_ref() {
            "--config" => init.config = value.into(),
            "--input" => init.input = value.into(),
            "--output" => init.output = value.into(),
            "--name" => init.name = value,
            "--sdk-version" => init.version = value,
            _ => unreachable!(),
        }
    }
    Ok(Action::Init(init))
}

fn compiler_path(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = override_path.or_else(|| env::var_os("KAJI_OPENAPI_BIN").map(PathBuf::from))
    {
        return std::fs::canonicalize(&path)
            .with_context(|| format!("cannot find OpenAPI compiler {}", path.display()));
    }
    let executable = env::current_exe().context("cannot locate the Kaji executable")?;
    let sibling = executable.with_file_name(if cfg!(windows) {
        "kaji-openapi.exe"
    } else {
        "kaji-openapi"
    });
    if !sibling.is_file() {
        bail!(
            "bundled OpenAPI compiler is missing: {}. Reinstall the platform package, or set --openapi-compiler /path/to/kaji-openapi (KAJI_OPENAPI_BIN is also supported). Source builds: build openapi/ with Go first",
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
            "java" => profiles.package(java::package("java").with(java::sdk())),
            "csharp" => profiles.package(csharp::package("csharp").with(csharp::sdk())),
            // Keep the established selector for existing scripts. New
            // configuration and direct commands should use `csharp`.
            "dotnet" => profiles.package(dotnet::package("dotnet").with(dotnet::sdk())),
            "elixir" => profiles.package(elixir::package("elixir").with(elixir::sdk())),
            "ruby" => profiles.package(ruby::package("ruby").with(ruby::sdk())),
            "swift" => profiles.package(swift::package("swift").with(swift::sdk())),
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
                "package {:?} uses plugin {:?}, which is not bundled by this Kaji binary",
                package.path,
                plugin.name
            );
        }
    }
    Ok(())
}

fn config_profiles(
    default_style: SdkClientStyle,
    packages: &[PackageConfig],
) -> Result<ProfileSet> {
    if packages.is_empty() {
        bail!("kaji.json must declare at least one package");
    }
    let mut profiles = ProfileSet::new(".").common(Common::default().client_style(default_style));
    for package in packages {
        let style = parse_style(package.client_style.as_deref())?;
        profiles = match package.language.as_str() {
            "typescript" => {
                has_only_known_plugins(
                    package,
                    &[
                        "sdk",
                        "zod",
                        "tanstack-react-query",
                        "tanstack-vue-query",
                        "swr",
                        "faker",
                        "msw",
                        "cypress",
                    ],
                )?;
                let sdk_plugins = package
                    .plugins
                    .iter()
                    .filter(|plugin| plugin.name == "sdk")
                    .collect::<Vec<_>>();
                if sdk_plugins.len() > 1 {
                    bail!("package {:?} declares sdk more than once", package.path);
                }
                if sdk_plugins.is_empty() {
                    if package.plugins.is_empty() {
                        bail!(
                            "TypeScript package {:?} must declare at least one plugin",
                            package.path
                        );
                    }
                    // Auxiliary TypeScript artifacts can intentionally be emitted without
                    // an SDK package. The empty typed package reserves the output root;
                    // the artifact pass below supplies the actual files.
                    let package_builder = ts::package(&package.path).common(package_common(style));
                    let package_builder = if let Some(name) = &package.name {
                        package_builder.name(name)
                    } else {
                        package_builder
                    };
                    profiles.package(package_builder)
                } else {
                    let plugin = sdk_plugins[0];
                    let transport = plugin.transport.as_deref().unwrap_or("fetch");
                    let mut sdk = match transport {
                        "fetch" => ts::sdk().fetch(),
                        "axios" => ts::sdk().axios(),
                        other => bail!(
                            "TypeScript sdk transport must be \"fetch\" or \"axios\", got {other:?}"
                        ),
                    };
                    if plugin.surface.as_deref().unwrap_or("client") == "raw" {
                        sdk = sdk.raw();
                    } else if plugin.surface.as_deref().unwrap_or("client") != "client" {
                        bail!("TypeScript sdk surface must be \"client\" or \"raw\"");
                    }
                    if let Some(name) = &plugin.client_name {
                        sdk = sdk.client_name(name);
                    }
                    if let Some(group_by_tag) = plugin.group_by_tag {
                        sdk = sdk.group_by_tag(group_by_tag);
                    }
                    if let Some(throw_on_error) = plugin.throw_on_error {
                        sdk = sdk.throw_on_error(throw_on_error);
                    }
                    let package_builder = ts::package(&package.path).common(package_common(style));
                    let package_builder = if let Some(name) = &package.name {
                        package_builder.name(name)
                    } else {
                        package_builder
                    };
                    profiles.package(package_builder.with(sdk))
                }
            }
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
                let package_builder = ts_cli::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(generator))
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
                    rust_cli::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(generator))
            }
            "rust" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = rust::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(rust::sdk()))
            }
            "go" => {
                has_only_known_plugins(package, &["sdk"])?;
                let plugin = sdk_plugin(package)?;
                let mut sdk = go::sdk();
                if let Some(jobs) = plugin.jobs {
                    sdk = sdk.jobs(jobs);
                }
                let package_builder = go::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(sdk))
            }
            "python" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = python::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(python::sdk()))
            }
            "php" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = php::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(php::sdk()))
            }
            "java" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = java::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(java::sdk()))
            }
            "csharp" | "dotnet" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = csharp::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(csharp::sdk()))
            }
            "elixir" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = elixir::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(elixir::sdk()))
            }
            "ruby" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = ruby::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(ruby::sdk()))
            }
            "swift" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = swift::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(swift::sdk()))
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
                profiles.package(mock::package(&package.path).with(server_builder))
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
                profiles.package(ts::package(&package.path).common(package_common(style)))
            }
            other => bail!(
                "unknown config language {other:?}; use typescript, typescript-cli, rust, rust-cli, go, python, php, java, csharp, dotnet (legacy alias), elixir, ruby, swift, mock, or artifacts"
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

fn generate_from_config(path: &Path, color: ColorChoice) -> Result<()> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("cannot read Kaji config {}", path.display()))?;
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("read Kaji config {}", path.display()))?;
    let config: ProjectConfig = serde_json::from_str(&source)
        .with_context(|| format!("parse Kaji JSON config {}", path.display()))?;
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
    let style = parse_style(config.defaults.client_style.as_deref())?;
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
        "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/kaji.schema.json",
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
        "Created {}. Edit its package plugins, then run kaji generate.",
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
            // that case Kaji does not own a package manifest to mutate.
            continue;
        };
        let mut manifest: serde_json::Value = serde_json::from_str(&manifest)
            .with_context(|| format!("parse generated TypeScript manifest {}", path.display()))?;
        let object = manifest
            .as_object_mut()
            .expect("Kaji TypeScript manifests are JSON objects");
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
        return generate_from_config(config, options.color);
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
        let status = Command::new(&helper)
            .arg("--out")
            .arg(temporary.path())
            .arg(&source)
            .status()
            .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
        if !status.success() {
            bail!("OpenAPI compiler failed ({status}); no SDK files were written")
        }
        reporter.phase("OpenAPI", started.elapsed());
        temporary.path()
    };
    write_sdk(artifacts, &options, &reporter)
}

fn write_sdk(artifacts: &Path, options: &Generate, reporter: &Reporter) -> Result<()> {
    let started = Instant::now();
    let api = kaji_core::adapter::openapi_sidecar::load_operations(
        artifacts,
        options.name.clone(),
        options.version.clone(),
    )?;
    let api = slice_api_paths(api, &options.path_selection)?;
    let security_schemes = kaji_core::adapter::openapi_sidecar::load_security_schemes(artifacts)?;
    let mut tree =
        kaji::generate_with_security_catalog(&api, profiles(options)?, Some(&security_schemes))?;
    if let Some(packages) = &options.config_packages {
        append_config_artifacts(&api, &mut tree, packages)?;
        add_typescript_artifact_dependencies(&mut tree, packages)?;
    }
    reporter.generated(options, started.elapsed());
    let started = Instant::now();
    tree.write_to(&options.output)?;
    write_generation_lock(&options.output, artifacts, options, &api)?;
    reporter.phase("Writing files", started.elapsed());
    reporter.completed(
        configured_plugin_count(options),
        tree.iter().count() + 1,
        &options.output,
    );
    Ok(())
}

const GENERATION_LOCK_VERSION: u8 = 1;
const GENERATION_LOCK_PATH: &str = ".kaji/generation.lock.json";

/// A deliberately small, secret-free account of exactly what Kaji rendered.
/// It is an output artifact rather than an input lock: regenerate it whenever
/// the contract or selected generator settings change, then review it in the
/// same change as generated code.
#[derive(Debug, Serialize)]
struct GenerationLock {
    version: u8,
    generator: GeneratorLock,
    input: GenerationInputLock,
    api: GeneratedApiLock,
    paths: PathSelection,
    targets: Vec<String>,
    settings: GenerationSettingsLock,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay: Option<GenerationReplayLock>,
}

/// Direct generation has no separate recipe to re-run. Preserve its
/// non-secret inputs so `kaji update` can faithfully replay it later.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct GenerationReplayLock {
    source: Option<String>,
    artifacts: Option<String>,
    languages: Vec<String>,
    name: String,
    version: String,
    client_style: String,
    typescript_transport: Option<String>,
    typescript_surface: String,
    typescript_client_name: Option<String>,
    go_jobs: Option<usize>,
    compiler: Option<String>,
    paths: PathSelection,
}

#[derive(Debug, Deserialize)]
struct UpdateLock {
    version: u8,
    input: UpdateInputLock,
    #[serde(default)]
    replay: Option<GenerationReplayLock>,
}

#[derive(Debug, Deserialize)]
struct UpdateInputLock {
    #[serde(default)]
    source_sha256: Option<String>,
    artifacts_sha256: String,
}

#[derive(Debug, Serialize)]
struct GeneratorLock {
    name: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct GenerationInputLock {
    kind: &'static str,
    locator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_sha256: Option<String>,
    artifacts_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
struct GeneratedApiLock {
    name: String,
    version: String,
    operations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GenerationSettingsLock {
    client_style: &'static str,
    typescript_transport: Option<&'static str>,
    typescript_surface: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    typescript_client_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    go_jobs: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compiler: Option<String>,
}

fn write_generation_lock(
    output: &Path,
    artifacts: &Path,
    options: &Generate,
    api: &Api,
) -> Result<()> {
    let (kind, locator, source_sha256) = match (&options.artifacts, &options.source) {
        (Some(path), _) => ("artifacts", path.to_string_lossy().into_owned(), None),
        (None, Some(OpenApiInput::Path(path))) if remote_spec_url(path).is_none() => (
            "openapi",
            path.to_string_lossy().into_owned(),
            options.source_sha256.clone().or(Some(sha256_file(path)?)),
        ),
        (None, Some(OpenApiInput::Path(path))) => (
            "openapi",
            path.to_string_lossy().into_owned(),
            options.source_sha256.clone(),
        ),
        (None, Some(OpenApiInput::Remote(remote))) => {
            ("openapi", remote.url.clone(), options.source_sha256.clone())
        }
        (None, None) => bail!("generation input is missing"),
    };
    let lock = GenerationLock {
        version: GENERATION_LOCK_VERSION,
        generator: GeneratorLock {
            name: "kaji",
            version: env!("CARGO_PKG_VERSION"),
        },
        input: GenerationInputLock {
            kind,
            locator,
            source_sha256,
            artifacts_sha256: sha256_directory(artifacts)?,
            config_sha256: options.config_sha256.clone(),
        },
        api: GeneratedApiLock {
            name: api.name.clone(),
            version: api.version.clone(),
            operations: api
                .operations
                .iter()
                .map(|operation| format!("{} {}", operation.method.as_str(), operation.path))
                .collect(),
        },
        paths: options.path_selection.clone(),
        targets: configured_labels(options),
        settings: GenerationSettingsLock {
            client_style: match options.style {
                SdkClientStyle::Namespaced => "namespaced",
                SdkClientStyle::Flat => "flat",
            },
            typescript_transport: options
                .typescript_transport
                .map(|transport| match transport {
                    TypeScriptTransport::Fetch => "fetch",
                    TypeScriptTransport::Axios => "axios",
                }),
            typescript_surface: if options.raw { "raw" } else { "client" },
            typescript_client_name: options.client_name.clone(),
            go_jobs: (options.jobs != 0).then_some(options.jobs),
            compiler: options
                .compiler
                .as_ref()
                .map(|compiler| compiler.to_string_lossy().into_owned()),
        },
        replay: direct_generation_replay(options),
    };
    let destination = output.join(GENERATION_LOCK_PATH);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("create generation metadata directory {}", parent.display())
        })?;
    }
    std::fs::write(
        &destination,
        format!("{}\n", serde_json::to_string_pretty(&lock)?),
    )
    .with_context(|| format!("write generation metadata {}", destination.display()))
}

fn direct_generation_replay(options: &Generate) -> Option<GenerationReplayLock> {
    options
        .config_packages
        .is_none()
        .then(|| GenerationReplayLock {
            source: options.source.as_ref().map(|source| match source {
                OpenApiInput::Path(path) => path.to_string_lossy().into_owned(),
                OpenApiInput::Remote(remote) => remote.url.clone(),
            }),
            artifacts: options
                .artifacts
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            languages: options.languages.clone(),
            name: options.name.clone(),
            version: options.version.clone(),
            client_style: match options.style {
                SdkClientStyle::Namespaced => "namespaced".into(),
                SdkClientStyle::Flat => "flat".into(),
            },
            typescript_transport: options
                .typescript_transport
                .map(|transport| match transport {
                    TypeScriptTransport::Fetch => "fetch".into(),
                    TypeScriptTransport::Axios => "axios".into(),
                }),
            typescript_surface: if options.raw { "raw" } else { "client" }.into(),
            typescript_client_name: options.client_name.clone(),
            go_jobs: (options.jobs != 0).then_some(options.jobs),
            compiler: options
                .compiler
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            paths: options.path_selection.clone(),
        })
}

fn update(options: Update) -> Result<()> {
    let root = std::fs::canonicalize(&options.output)
        .with_context(|| format!("cannot read update output {}", options.output.display()))?;
    if !root.is_dir() {
        bail!("update output must be a directory")
    }
    let mut locks = Vec::new();
    find_generation_locks(&root, &mut locks)?;
    if locks.is_empty() {
        bail!(
            "no {GENERATION_LOCK_PATH} files were found below {}; run kaji generate first",
            root.display()
        )
    }
    let mut updated = 0;
    let mut unchanged = 0;
    let mut unsupported = 0;
    for path in locks {
        let document = std::fs::read_to_string(&path)
            .with_context(|| format!("read generation lock {}", path.display()))?;
        let lock: UpdateLock = serde_json::from_str(&document)
            .with_context(|| format!("parse generation lock {}", path.display()))?;
        if lock.version != GENERATION_LOCK_VERSION {
            bail!(
                "generation lock {} has unsupported version {}; expected {}",
                path.display(),
                lock.version,
                GENERATION_LOCK_VERSION
            )
        }
        let Some(replay) = lock.replay else {
            eprintln!(
                "kaji update: skipping {} (created by config generation; run kaji generate --config instead)",
                path.display()
            );
            unsupported += 1;
            continue;
        };
        let output = path
            .parent()
            .and_then(Path::parent)
            .expect("generation lock is always nested below .kaji")
            .to_path_buf();
        if !options.force && replay_input_is_unchanged(&replay, &lock.input)? {
            println!("Unchanged: {}", output.display());
            unchanged += 1;
            continue;
        }
        println!("Updating: {}", output.display());
        generate(replay_generate_options(replay, output)?)?;
        updated += 1;
    }
    println!(
        "Update complete: {updated} regenerated, {unchanged} unchanged, {unsupported} need their config recipe"
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
                println!("No Kaji auth profiles are configured.");
            } else {
                for (profile, token_env) in profiles {
                    println!("{profile}\t${token_env}");
                }
            }
        }
    }
    Ok(())
}

fn find_generation_locks(directory: &Path, locks: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("read update output {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("read update output {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect update path {}", path.display()))?;
        if file_type.is_dir() {
            if entry.file_name() != ".git" {
                find_generation_locks(&path, locks)?;
            }
        } else if file_type.is_file() && path.ends_with(GENERATION_LOCK_PATH) {
            locks.push(path);
        }
    }
    Ok(())
}

fn replay_input_is_unchanged(
    replay: &GenerationReplayLock,
    input: &UpdateInputLock,
) -> Result<bool> {
    if let Some(source) = &replay.source {
        let source = Path::new(source);
        // Remote URLs are intentionally re-fetched. Their lock records the
        // downloaded hash, but a local check cannot establish freshness.
        return Ok(source.is_file()
            && input
                .source_sha256
                .as_ref()
                .is_some_and(|hash| sha256_file(source).is_ok_and(|actual| actual == *hash)));
    }
    if let Some(artifacts) = &replay.artifacts {
        let artifacts = Path::new(artifacts);
        return Ok(artifacts.is_dir() && sha256_directory(artifacts)? == input.artifacts_sha256);
    }
    Ok(false)
}

fn replay_generate_options(replay: GenerationReplayLock, output: PathBuf) -> Result<Generate> {
    let source = replay
        .source
        .map(|source| OpenApiInput::Path(PathBuf::from(source)));
    let artifacts = replay.artifacts.map(PathBuf::from);
    if source.is_some() == artifacts.is_some() {
        bail!("generation replay must contain exactly one source or artifact directory")
    }
    let style = parse_style(Some(&replay.client_style))?;
    let typescript_transport = replay
        .typescript_transport
        .as_deref()
        .map(TypeScriptTransport::parse)
        .transpose()?;
    if replay.typescript_surface != "client" && replay.typescript_surface != "raw" {
        bail!("generation replay has invalid TypeScript surface")
    }
    Ok(Generate {
        source,
        config: None,
        config_packages: None,
        artifacts,
        output,
        languages: replay.languages,
        name: replay.name,
        version: replay.version,
        style,
        raw: replay.typescript_surface == "raw",
        typescript_transport,
        client_name: replay.typescript_client_name,
        compiler: replay.compiler.map(PathBuf::from),
        path_selection: replay.paths,
        config_sha256: None,
        source_sha256: None,
        jobs: replay.go_jobs.unwrap_or_default(),
        color: ColorChoice::Auto,
    })
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
                .any(|pattern| kaji_core::wildcard_matches(pattern, &operation.path));
        included
            && !selection
                .exclude
                .iter()
                .any(|pattern| kaji_core::wildcard_matches(pattern, &operation.path))
    });
    if api.operations.is_empty() {
        bail!("path selection matched no OpenAPI operations; adjust paths.include or paths.exclude")
    }
    Ok(api)
}

#[derive(Debug, Deserialize)]
struct CheckSidecarOperation {
    #[serde(default)]
    operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckDiagnostic {
    code: &'static str,
    severity: CheckSeverity,
    method: String,
    path: String,
    message: String,
    hint: String,
}

impl CheckDiagnostic {
    fn fingerprint(&self) -> String {
        format!("{}:{}:{}", self.code, self.method, self.path)
    }

    fn render(&self) {
        eprintln!(
            "{}[{}] {} {}: {}\n  help: {}",
            self.severity.label(),
            self.code,
            self.method,
            self.path,
            self.message,
            self.hint
        );
    }
}

const CHECK_RULES: &[&str] = &[
    "missing-operation-id",
    "duplicate-operation-id",
    "ambiguous-operation-id",
    "unsafe-path",
    "ambiguous-path",
    "missing-path-parameter",
    "optional-path-parameter",
    "missing-success-response",
];

#[derive(Debug, Deserialize)]
struct CheckBaseline {
    diagnostics: Vec<CheckBaselineDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct CheckBaselineDiagnostic {
    fingerprint: String,
}

#[derive(Serialize)]
struct CheckBaselineOutput {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: u8,
    diagnostics: Vec<CheckBaselineDiagnosticOutput>,
}

#[derive(Serialize)]
struct CheckBaselineDiagnosticOutput {
    fingerprint: String,
}

#[derive(Serialize)]
struct CheckJsonReport<'a> {
    version: u8,
    operations: usize,
    diagnostics: Vec<CheckJsonDiagnostic<'a>>,
    suppressed: Vec<CheckJsonDiagnostic<'a>>,
    summary: CheckJsonSummary,
}

#[derive(Serialize)]
struct CheckJsonDiagnostic<'a> {
    code: &'a str,
    severity: CheckSeverity,
    method: &'a str,
    path: &'a str,
    message: &'a str,
    hint: &'a str,
    fingerprint: String,
}

impl<'a> From<&'a CheckDiagnostic> for CheckJsonDiagnostic<'a> {
    fn from(diagnostic: &'a CheckDiagnostic) -> Self {
        Self {
            code: diagnostic.code,
            severity: diagnostic.severity,
            method: &diagnostic.method,
            path: &diagnostic.path,
            message: &diagnostic.message,
            hint: &diagnostic.hint,
            fingerprint: diagnostic.fingerprint(),
        }
    }
}

#[derive(Serialize)]
struct CheckJsonSummary {
    errors: usize,
    warnings: usize,
    suppressed: usize,
    failed: bool,
}

fn read_check_operations(artifacts: &Path) -> Result<Vec<CheckSidecarOperation>> {
    let index: BTreeMap<String, String> = serde_json::from_reader(
        std::fs::File::open(artifacts.join("operations.json"))
            .context("read compiled operation index")?,
    )
    .context("parse compiled operation index")?;
    let order: Vec<String> = serde_json::from_reader(
        std::fs::File::open(artifacts.join("operations-order.json"))
            .context("read compiled operation order")?,
    )
    .context("parse compiled operation order")?;
    order
        .into_iter()
        .map(|key| {
            let file = index
                .get(&key)
                .with_context(|| format!("compiled operation index is missing {key:?}"))?;
            serde_json::from_reader(
                std::fs::File::open(artifacts.join("operations").join(file))
                    .with_context(|| format!("read compiled operation {key:?}"))?,
            )
            .with_context(|| format!("parse compiled operation {key:?}"))
        })
        .collect()
}

fn operation_symbol(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn path_template_parameters(path: &str) -> Vec<&str> {
    path.split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .filter(|name| !name.is_empty())
        .collect()
}

fn check_api(api: &Api, source: &[CheckSidecarOperation]) -> Vec<CheckDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut ids = BTreeMap::<String, (String, String)>::new();
    let mut symbols = BTreeMap::<String, (String, String, String)>::new();

    for (index, operation) in api.operations.iter().enumerate() {
        let source_operation = source.get(index);
        let method = operation.method.as_str().to_owned();
        let path = operation.path.clone();
        let operation_id = source_operation
            .map(|operation| operation.operation_id.trim())
            .unwrap_or("");

        if operation_id.is_empty() {
            diagnostics.push(CheckDiagnostic {
                code: "missing-operation-id",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "operationId is missing; Kaji can infer a name, but the inferred public SDK and CLI name may change when the path changes".into(),
                hint: "add a stable, unique operationId such as listUsers".into(),
            });
        } else if let Some((previous_method, previous_path)) =
            ids.insert(operation_id.to_owned(), (method.clone(), path.clone()))
        {
            diagnostics.push(CheckDiagnostic {
                code: "duplicate-operation-id",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: format!(
                    "operationId {operation_id:?} is also used by {previous_method} {previous_path}"
                ),
                hint: "give every operation a unique operationId".into(),
            });
        } else {
            let symbol = operation_symbol(operation_id);
            if !symbol.is_empty() {
                if let Some((previous_id, previous_method, previous_path)) = symbols.insert(
                    symbol,
                    (operation_id.to_owned(), method.clone(), path.clone()),
                ) {
                    if previous_id != operation_id {
                        diagnostics.push(CheckDiagnostic {
                            code: "ambiguous-operation-id",
                            severity: CheckSeverity::Error,
                            method: method.clone(),
                            path: path.clone(),
                            message: format!(
                                "operationId {operation_id:?} normalizes to the same generated symbol as {previous_id:?} on {previous_method} {previous_path}"
                            ),
                            hint: "rename one operationId so it stays distinct after case and punctuation normalization".into(),
                        });
                    }
                }
            }
        }

        if !path.starts_with('/') {
            diagnostics.push(CheckDiagnostic {
                code: "unsafe-path",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "path is not absolute".into(),
                hint: "start every OpenAPI path with `/`".into(),
            });
        }
        if path.is_empty() || path.contains("//") {
            diagnostics.push(CheckDiagnostic {
                code: "ambiguous-path",
                severity: CheckSeverity::Error,
                method: method.clone(),
                path: path.clone(),
                message: "path contains an empty segment, which makes generated command grouping ambiguous".into(),
                hint: "use a single slash between path segments".into(),
            });
        }

        let path_parameters = path_template_parameters(&path)
            .into_iter()
            .collect::<BTreeSet<_>>();
        for name in &path_parameters {
            match operation
                .parameters
                .iter()
                .find(|parameter| parameter.location == "path" && parameter.name == *name)
            {
                None => diagnostics.push(CheckDiagnostic {
                    code: "missing-path-parameter",
                    severity: CheckSeverity::Error,
                    method: method.clone(),
                    path: path.clone(),
                    message: format!("path parameter {{{name}}} is not declared as a parameter"),
                    hint: format!("declare `{name}` with `in: path` and `required: true`"),
                }),
                Some(parameter) if !parameter.required => diagnostics.push(CheckDiagnostic {
                    code: "optional-path-parameter",
                    severity: CheckSeverity::Error,
                    method: method.clone(),
                    path: path.clone(),
                    message: format!("path parameter {{{name}}} is not required"),
                    hint: "OpenAPI path parameters must set `required: true`".into(),
                }),
                Some(_) => {}
            }
        }

        if !operation
            .responses
            .iter()
            .any(|response| response.status.starts_with('2'))
        {
            diagnostics.push(CheckDiagnostic {
                code: "missing-success-response",
                severity: CheckSeverity::Error,
                method,
                path,
                message: "operation declares no 2xx success response".into(),
                hint: "declare the successful status code, for example `200`, `201`, or `204`"
                    .into(),
            });
        }
    }
    diagnostics
}

fn check(options: Check) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let mut compiler = Command::new(&helper);
    compiler.arg("--out").arg(temporary.path()).arg(&source);
    // JSON is an API, so keep compiler progress off stdout where CI parsers and
    // agents expect exactly one document.
    if options.format == CheckFormat::Json {
        compiler.stdout(Stdio::null());
    }
    let status = compiler
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = kaji_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    let source_operations = read_check_operations(temporary.path())?;
    let mut diagnostics = check_api(&api, &source_operations);
    for diagnostic in &mut diagnostics {
        if let Some(severity) = options.severity_overrides.get(diagnostic.code) {
            diagnostic.severity = *severity;
        }
    }

    if let Some(path) = &options.write_baseline {
        write_check_baseline(path, &diagnostics)?;
    }

    let baseline = match &options.baseline {
        Some(path) => load_check_baseline(path)?,
        None => BTreeSet::new(),
    };
    let (suppressed, reported): (Vec<_>, Vec<_>) =
        diagnostics.into_iter().partition(|diagnostic| {
            options.ignored_rules.contains(diagnostic.code)
                || baseline.contains(&diagnostic.fingerprint())
        });
    let errors = reported
        .iter()
        .filter(|diagnostic| diagnostic.severity == CheckSeverity::Error)
        .count();
    let warnings = reported.len() - errors;
    let failures = reported
        .iter()
        .filter(|diagnostic| options.fail_on.fails(diagnostic.severity))
        .count();

    match options.format {
        CheckFormat::Human => {
            if reported.is_empty() {
                print_check_passed(api.operations.len(), suppressed.len());
            } else {
                for diagnostic in &reported {
                    diagnostic.render();
                }
            }
        }
        CheckFormat::Json => {
            let report = CheckJsonReport {
                version: 1,
                operations: api.operations.len(),
                diagnostics: reported.iter().map(CheckJsonDiagnostic::from).collect(),
                suppressed: suppressed.iter().map(CheckJsonDiagnostic::from).collect(),
                summary: CheckJsonSummary {
                    errors,
                    warnings,
                    suppressed: suppressed.len(),
                    failed: failures > 0,
                },
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    if failures > 0 {
        bail!("check failed with {failures} issue(s)")
    }
    Ok(())
}

#[derive(Serialize)]
struct ShownOperation {
    method: String,
    path: String,
    operation_id: String,
}

#[derive(Default)]
struct ShowTree {
    operations: Vec<String>,
    children: BTreeMap<String, ShowTree>,
}

fn show(options: Show) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let mut compiler = Command::new(&helper);
    compiler.arg("--out").arg(temporary.path()).arg(&source);
    // JSON is an API for agent callers; compiler progress must not corrupt it.
    if options.format == ShowFormat::Json {
        compiler.stdout(Stdio::null());
    }
    let status = compiler
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = kaji_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    let api = slice_api_paths(api, &options.paths)?;
    match options.format {
        ShowFormat::Json => {
            let operations = api
                .operations
                .iter()
                .map(|operation| ShownOperation {
                    method: operation.method.as_str().into(),
                    path: operation.path.clone(),
                    operation_id: operation.id.clone(),
                })
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&operations)?);
        }
        ShowFormat::Human => print_show_tree(&api),
    }
    Ok(())
}

fn print_show_tree(api: &Api) {
    let mut root = ShowTree::default();
    for operation in &api.operations {
        let mut node = &mut root;
        for segment in operation
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
        {
            node = node.children.entry(segment.into()).or_default();
        }
        node.operations
            .push(format!("{} {}", operation.method.as_str(), operation.id));
    }
    println!("/");
    print_show_children(&root, "");
}

fn print_show_children(node: &ShowTree, prefix: &str) {
    let entries = node.children.iter().collect::<Vec<_>>();
    for (index, (segment, child)) in entries.iter().enumerate() {
        let last = index + 1 == entries.len();
        let branch = if last { "└─" } else { "├─" };
        let operations = if child.operations.is_empty() {
            String::new()
        } else {
            format!(" [{}]", child.operations.join(", "))
        };
        println!("{prefix}{branch}{segment}{operations}");
        let next_prefix = format!("{prefix}{}", if last { "  " } else { "│ " });
        print_show_children(child, &next_prefix);
    }
}

fn print_check_passed(operations: usize, suppressed: usize) {
    if suppressed == 0 {
        println!("check passed: {operations} operations are ready for generation");
    } else {
        println!(
            "check passed: {operations} operations are ready for generation ({suppressed} known issue(s) suppressed)"
        );
    }
}

fn load_check_baseline(path: &Path) -> Result<BTreeSet<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot read check baseline {}", path.display()))?;
    let baseline: CheckBaseline = serde_json::from_reader(file)
        .with_context(|| format!("parse check baseline {}", path.display()))?;
    Ok(baseline
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.fingerprint)
        .collect())
}

fn write_check_baseline(path: &Path, diagnostics: &[CheckDiagnostic]) -> Result<()> {
    let document = CheckBaselineOutput {
        schema: "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/check-baseline.schema.json",
        version: 1,
        diagnostics: diagnostics
            .iter()
            .map(|diagnostic| CheckBaselineDiagnosticOutput {
                fingerprint: diagnostic.fingerprint(),
            })
            .collect(),
    };
    let document = serde_json::to_string_pretty(&document)?;
    std::fs::write(path, format!("{document}\n"))
        .with_context(|| format!("write check baseline {}", path.display()))
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
    let api = kaji_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    mcp::serve(api, &options.base_url)
}

#[derive(Clone, Serialize)]
struct MockRequest {
    id: u64,
    method: String,
    path: String,
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario: Option<String>,
    status: u16,
    body: Option<String>,
}

#[derive(Default)]
struct MockRequests {
    next_id: u64,
    entries: Vec<MockRequest>,
}

fn serve_mock(options: MockServe) -> Result<()> {
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
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = Arc::new(kaji_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?);
    // Fail before listening when a scenario is malformed. Falling back to a
    // happy-path response would hide a broken test contract.
    kaji_core::extract_mock_scenarios(&api)?;
    let listener = TcpListener::bind(("127.0.0.1", options.port))
        .with_context(|| format!("cannot listen on http://127.0.0.1:{}", options.port))?;
    let requests = Arc::new(Mutex::new(MockRequests::default()));
    println!("Kaji dynamic mock: http://127.0.0.1:{}", options.port);
    println!(
        "Request log: http://127.0.0.1:{}/_kaji/requests",
        options.port
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_mock_request(stream, &api, &requests) {
                    eprintln!("kaji mock: {error:#}");
                }
            }
            Err(error) => eprintln!("kaji mock: accept connection: {error}"),
        }
    }
    Ok(())
}

fn handle_mock_request(
    mut stream: TcpStream,
    api: &Api,
    requests: &Arc<Mutex<MockRequests>>,
) -> Result<()> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .context("configure mock request timeout")?;
    let mut reader = BufReader::new(&mut stream);
    let mut start = String::new();
    if reader.read_line(&mut start)? == 0 {
        return Ok(());
    }
    let mut parts = start.split_whitespace();
    let method = parts
        .next()
        .context("invalid HTTP request method")?
        .to_owned();
    let target = parts
        .next()
        .context("invalid HTTP request target")?
        .to_owned();
    let mut content_length = 0usize;
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_owned();
            let value = value.trim().to_owned();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(name, value);
        }
    }
    let mut bytes = vec![0; content_length.min(1024 * 1024)];
    reader.read_exact(&mut bytes)?;
    drop(reader);
    let (path, query) = mock_target_parts(&target);

    if method == "GET" && path == "/_kaji/requests" {
        let entries = requests
            .lock()
            .expect("mock request log poisoned")
            .entries
            .clone();
        return write_mock_response(
            &mut stream,
            200,
            "application/json",
            &serde_json::to_vec(&entries)?,
            &BTreeMap::new(),
        );
    }
    if method == "GET" && path == "/_kaji/health" {
        return write_mock_response(
            &mut stream,
            200,
            "application/json",
            br#"{"ok":true}"#,
            &BTreeMap::new(),
        );
    }
    if method == "OPTIONS" {
        return write_mock_response(&mut stream, 204, "text/plain", b"", &BTreeMap::new());
    }

    // OpenAPI permits a literal route alongside a parameterized sibling, for
    // example `/users/me` and `/users/{id}`. Prefer the most literal match so
    // the result does not accidentally depend on declaration order.
    let operation = api
        .operations
        .iter()
        .filter(|operation| {
            operation.method.as_str().eq_ignore_ascii_case(&method)
                && mock_path_matches(&operation.path, path)
        })
        .max_by_key(|operation| mock_path_specificity(&operation.path));
    let request_id = next_mock_id(requests);
    let path_parameters = operation
        .map(|operation| mock_path_parameters(&operation.path, path))
        .unwrap_or_default();
    let json_body = serde_json::from_slice(&bytes).ok();
    let scenario = operation
        .map(kaji_core::extract_operation_mock_scenarios)
        .transpose()?
        .and_then(|scenarios| {
            scenarios.into_iter().find(|scenario| {
                kaji_core::mock_scenario_matches(
                    scenario,
                    &headers,
                    &query,
                    &path_parameters,
                    json_body.as_ref(),
                )
            })
        });
    let scenario_name = scenario.as_ref().map(|scenario| scenario.name.clone());
    let (status, content_type, response, response_headers) = match (operation, scenario) {
        (_, Some(scenario)) => {
            if let Some(delay_ms) = scenario.response.delay_ms {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            let content_type = scenario_content_type(&scenario.response.headers)
                .unwrap_or_else(|| "application/json".into());
            (
                scenario.response.status,
                content_type,
                scenario.response.body,
                scenario.response.headers,
            )
        }
        (Some(operation), None) => {
            let (status, content_type, response) =
                kaji_core::httpmock::mock_dynamic_response(api, operation, request_id);
            (
                status,
                content_type.unwrap_or_else(|| "application/json".into()),
                Some(response),
                BTreeMap::new(),
            )
        }
        (None, None) => (
            404,
            "application/json".into(),
            Some(serde_json::json!({ "error": "No OpenAPI operation matches this request" })),
            BTreeMap::new(),
        ),
    };
    let response = mock_response_body(&content_type, response)?;
    let body =
        (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).chars().take(4096).collect());
    let entry = MockRequest {
        id: request_id,
        method,
        path: target,
        operation: operation.map(|operation| operation.id.clone()),
        scenario: scenario_name,
        status,
        body,
    };
    let mut log = requests.lock().expect("mock request log poisoned");
    log.entries.push(entry);
    if log.entries.len() > 200 {
        log.entries.remove(0);
    }
    write_mock_response(
        &mut stream,
        status,
        &content_type,
        &response,
        &response_headers,
    )
}

fn next_mock_id(requests: &Arc<Mutex<MockRequests>>) -> u64 {
    let mut log = requests.lock().expect("mock request log poisoned");
    log.next_id += 1;
    log.next_id
}

fn mock_path_matches(template: &str, path: &str) -> bool {
    template
        .split('/')
        .zip(path.split('/'))
        .all(|(template, actual)| {
            if template.starts_with('{') && template.ends_with('}') {
                !actual.is_empty()
            } else {
                template == actual
            }
        })
        && template.split('/').count() == path.split('/').count()
}

fn mock_path_specificity(template: &str) -> usize {
    template
        .split('/')
        .filter(|segment| {
            !(segment.is_empty() || segment.starts_with('{') && segment.ends_with('}'))
        })
        .count()
}

fn mock_target_parts(target: &str) -> (&str, BTreeMap<String, String>) {
    let (path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    (path, mock_query_parameters(raw_query))
}

fn mock_query_parameters(raw_query: &str) -> BTreeMap<String, String> {
    raw_query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (mock_percent_decode(name), mock_percent_decode(value))
        })
        .collect()
}

fn mock_percent_decode(value: &str) -> String {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hex = |value: u8| match value {
                    b'0'..=b'9' => Some(value - b'0'),
                    b'a'..=b'f' => Some(value - b'a' + 10),
                    b'A'..=b'F' => Some(value - b'A' + 10),
                    _ => None,
                };
                if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    decoded.push(high * 16 + low);
                    index += 2;
                } else {
                    decoded.push(bytes[index]);
                }
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn mock_path_parameters(template: &str, path: &str) -> BTreeMap<String, String> {
    template
        .split('/')
        .zip(path.split('/'))
        .filter_map(|(template, value)| {
            template
                .strip_prefix('{')
                .and_then(|name| name.strip_suffix('}'))
                .map(|name| (name.to_owned(), mock_percent_decode(value)))
        })
        .collect()
}

fn scenario_content_type(headers: &BTreeMap<String, String>) -> Option<String> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone())
}

fn mock_response_body(content_type: &str, body: Option<serde_json::Value>) -> Result<Vec<u8>> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    if content_type.to_ascii_lowercase().contains("json") {
        return Ok(serde_json::to_vec(&body)?);
    }
    if let Some(text) = body.as_str() {
        return Ok(text.as_bytes().to_vec());
    }
    Ok(serde_json::to_vec(&body)?)
}

fn write_mock_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    headers: &BTreeMap<String, String>,
) -> Result<()> {
    let mut extra_headers = String::new();
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("content-type") {
            use std::fmt::Write as _;
            writeln!(extra_headers, "{name}: {value}\r")?;
        }
    }
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra_headers}Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type, authorization\r\nConnection: close\r\n\r\n",
        mock_status_text(status),
        body.len(),
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn mock_status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Mock Response",
    }
}

#[cfg(any())]
fn mock_dashboard_removed() -> &'static str {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Kaji Mock Inspector</title><style>
:root{--ink:#e8e1d5;--muted:#989185;--line:#36342e;--panel:#161714;--void:#0b0c0a;--signal:#d7ff4f;--warning:#ff7557}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 90% 0,#273016 0,transparent 32rem),var(--void);color:var(--ink);font:14px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace}.shell{max-width:1120px;margin:auto;padding:48px 28px}header{display:flex;justify-content:space-between;gap:24px;align-items:end;border-bottom:1px solid var(--line);padding-bottom:25px}h1{font:700 clamp(34px,6vw,68px)/.88 Georgia,serif;letter-spacing:-.06em;margin:0}h1 b{color:var(--signal);font-weight:inherit}.live{color:var(--signal);font-size:11px;letter-spacing:.14em;text-transform:uppercase}.live:before{content:'';display:inline-block;width:8px;height:8px;margin-right:8px;border-radius:99px;background:var(--signal);box-shadow:0 0 18px var(--signal);animation:pulse 1.5s infinite}@keyframes pulse{50%{opacity:.35;transform:scale(.7)}}.summary{display:flex;gap:12px;margin:25px 0}.card{background:var(--panel);border:1px solid var(--line);padding:12px 15px;min-width:130px}.card strong{display:block;font-size:25px;color:var(--signal)}.card span{color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.08em}table{width:100%;border-collapse:collapse;background:rgba(22,23,20,.82);border:1px solid var(--line)}th{text-align:left;padding:11px 14px;color:var(--muted);font-weight:400;font-size:11px;letter-spacing:.08em;text-transform:uppercase;border-bottom:1px solid var(--line)}td{padding:13px 14px;border-bottom:1px solid #292a26;vertical-align:top}tr:last-child td{border:0}.method{color:var(--signal);font-weight:bold}.status{color:var(--warning)}code{white-space:pre-wrap;word-break:break-word;color:#d5d1c8}.empty{padding:48px 14px;color:var(--muted);text-align:center}footer{color:var(--muted);font-size:12px;margin-top:18px}a{color:var(--signal)}</style></head><body><main class="shell"><header><div><div class="live">Local contract emulator</div><h1>Kaji <b>Mock</b></h1></div><div class="live" id="state">watching requests</div></header><section class="summary"><div class="card"><strong id="count">0</strong><span>requests observed</span></div><div class="card"><strong id="latest">—</strong><span>latest status</span></div></section><table><thead><tr><th>#</th><th>method</th><th>route</th><th>operation</th><th>status</th><th>body</th></tr></thead><tbody id="rows"><tr><td class="empty" colspan="6">Waiting for your app to call the mock API.</td></tr></tbody></table><footer>Schema-shaped responses are regenerated for every request. <a href="/_kaji/requests">Raw request log JSON</a></footer></main><script>const rows=document.querySelector('#rows'),count=document.querySelector('#count'),latest=document.querySelector('#latest');const esc=v=>{const e=document.createElement('span');e.textContent=v??'';return e.innerHTML};async function refresh(){try{const data=await fetch('/_kaji/requests').then(r=>r.json());count.textContent=data.length;latest.textContent=data.length?data[data.length-1].status:'—';rows.innerHTML=data.length?[...data].reverse().map(r=>`<tr><td>${r.id}</td><td class="method">${esc(r.method)}</td><td><code>${esc(r.path)}</code></td><td>${esc(r.operation??'unmatched')}</td><td class="status">${r.status}</td><td><code>${esc(r.body??'')}</code></td></tr>`).join(''):'<tr><td class="empty" colspan="6">Waiting for your app to call the mock API.</td></tr>'}catch{document.querySelector('#state').textContent='reconnecting…'}}refresh();setInterval(refresh,900)</script></body></html>"#
}

fn main() -> ExitCode {
    let action = match parse(env::args_os().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("kaji: {error:#}");
            return ExitCode::from(2);
        }
    };
    let result = match action {
        Action::Help => {
            print!("{HELP}");
            Ok(())
        }
        Action::Version => {
            println!("kaji {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Languages => {
            println!("{}", LANGUAGES.join("\n"));
            Ok(())
        }
        Action::Show(options) => show(options),
        Action::Update(options) => update(options),
        Action::Auth(options) => auth(options),
        Action::Discover(options) => discover(options),
        Action::Download(options) => download(options),
        Action::Init(init) => init_config(init),
        Action::Mcp(options) => serve_mcp(options),
        Action::McpGenerator => mcp::serve_generator(),
        Action::MockServe(options) => serve_mock(options),
        Action::Check(options) => check(options),
        Action::Generate(options) => generate(*options),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kaji: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{Operation, OperationParameter, OperationResponse};
    fn arguments(value: &str) -> Vec<OsString> {
        value.split_whitespace().map(Into::into).collect()
    }

    #[test]
    fn selects_multiple_targets_without_duplicates() {
        let Action::Generate(options) =
            parse(arguments("generate api.yaml -o sdk -l go,rust -l go")).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.languages, ["go", "rust"]);
        assert!(matches!(options.style, SdkClientStyle::Namespaced));
    }

    #[test]
    fn accepts_csharp_and_the_legacy_dotnet_selector() {
        let Action::Generate(options) =
            parse(arguments("generate api.yaml -o sdk -l csharp,dotnet")).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.languages, ["csharp", "dotnet"]);
    }

    #[test]
    fn parses_repeatable_direct_path_selectors() {
        let Action::Generate(options) = parse(arguments(
            "generate --artifacts cache -o sdk -l go --include-path /messages* --include-path /admin* --exclude-path /admin/audit*",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.path_selection.include, ["/messages*", "/admin*"]);
        assert_eq!(options.path_selection.exclude, ["/admin/audit*"]);
    }

    #[test]
    fn validates_flags_sources_and_language_specific_options() {
        for invalid in [
            "generate api.yaml -o sdk",
            "generate -o sdk -l go",
            "generate api.yaml -l go",
            "generate api.yaml --artifacts cache -o sdk -l go",
            "generate api.yaml -o sdk -l go --typescript-surface raw",
            "generate api.yaml -o sdk -l go --client-style bad",
            "generate api.yaml -o sdk -l go --unknown nope",
            "generate api.yaml second.yaml -o sdk -l go",
            "generate --artifacts cache -o sdk -l go --openapi-compiler helper",
            "generate api.yaml -o sdk -l go --jobs 0",
            "generate api.yaml -o sdk -l go --jobs -1",
            "generate api.yaml -o sdk -l go --jobs many",
        ] {
            assert!(parse(arguments(invalid)).is_err(), "{invalid}");
        }
    }

    #[test]
    fn supports_all_targets_and_artifact_reuse() {
        let Action::Generate(options) = parse(arguments(
            "generate --artifacts cache -o sdk -l all --client-style flat",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.languages.len(), SDK_LANGUAGES.len());
        assert_eq!(options.artifacts, Some(PathBuf::from("cache")));
    }

    #[test]
    fn path_selectors_are_validated_and_exclusions_win() {
        let selection = PathSelection {
            include: vec!["/messages*".into(), "/admin/users*".into()],
            exclude: vec!["/admin/users/audit*".into()],
        };
        let api = Api {
            name: "Example".into(),
            version: "1".into(),
            operations: vec![
                Operation {
                    path: "/messages/send".into(),
                    ..Default::default()
                },
                Operation {
                    path: "/admin/users".into(),
                    ..Default::default()
                },
                Operation {
                    path: "/admin/users/audit-log".into(),
                    ..Default::default()
                },
                Operation {
                    path: "/health".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let sliced = slice_api_paths(api, &selection).unwrap();
        assert_eq!(
            sliced
                .operations
                .iter()
                .map(|operation| operation.path.as_str())
                .collect::<Vec<_>>(),
            ["/messages/send", "/admin/users"]
        );
        assert!(
            validate_path_selection(&PathSelection {
                include: vec!["messages".into()],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            slice_api_paths(
                Api {
                    operations: vec![Operation {
                        path: "/health".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                &PathSelection {
                    include: vec!["/messages*".into()],
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn color_mode_is_global_and_does_not_force_direct_generation() {
        let Action::Generate(options) = parse(arguments("generate --color always")).unwrap() else {
            panic!()
        };
        assert_eq!(options.config, Some(PathBuf::from("kaji.json")));
        assert_eq!(options.color, ColorChoice::Always);
        assert!(ColorChoice::Always.enabled());
        assert!(!ColorChoice::Never.enabled());
    }

    #[test]
    fn selects_the_generator_control_mcp_server() {
        assert!(matches!(
            parse(arguments("mcp generator")).unwrap(),
            Action::McpGenerator
        ));
        assert!(parse(arguments("mcp generator extra")).is_err());
    }

    #[test]
    fn parses_native_mock_server_options() {
        let Action::MockServe(options) = parse(arguments(
            "mock serve openapi.yaml --port 4011 --openapi-compiler compiler",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.source, PathBuf::from("openapi.yaml"));
        assert_eq!(options.port, 4011);
        assert_eq!(options.compiler, Some(PathBuf::from("compiler")));
        assert!(parse(arguments("mock serve openapi.yaml --port 0")).is_err());
    }

    #[test]
    fn parses_contract_check_options() {
        let Action::Check(options) =
            parse(arguments("check openapi.yaml --openapi-compiler compiler")).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.source, PathBuf::from("openapi.yaml"));
        assert_eq!(options.compiler, Some(PathBuf::from("compiler")));
        assert!(parse(arguments("check one.yaml two.yaml")).is_err());
        assert!(parse(arguments("check openapi.yaml --wat")).is_err());
    }

    #[test]
    fn parses_show_update_and_auth_commands() {
        let Action::Show(options) = parse(arguments(
            "show openapi.yaml --include-path /messages* --exclude-path /messages/audit* --json",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.source, PathBuf::from("openapi.yaml"));
        assert_eq!(options.paths.include, ["/messages*"]);
        assert_eq!(options.paths.exclude, ["/messages/audit*"]);
        assert_eq!(options.format, ShowFormat::Json);

        let Action::Update(options) =
            parse(arguments("update --output generated --force")).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.output, PathBuf::from("generated"));
        assert!(options.force);

        let Action::Auth(Auth::Login { profile, token_env }) =
            parse(arguments("auth login github --token-env GITHUB_TOKEN")).unwrap()
        else {
            panic!()
        };
        assert_eq!(profile, "github");
        assert_eq!(token_env, "GITHUB_TOKEN");
        assert!(parse(arguments("auth logout github extra")).is_err());
    }

    #[test]
    fn update_detects_unchanged_local_direct_input() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("openapi.yaml");
        std::fs::write(&source, "openapi: 3.1.0\n").unwrap();
        let replay = GenerationReplayLock {
            source: Some(source.to_string_lossy().into_owned()),
            artifacts: None,
            languages: vec!["go".into()],
            name: "API".into(),
            version: "1".into(),
            client_style: "namespaced".into(),
            typescript_transport: None,
            typescript_surface: "client".into(),
            typescript_client_name: None,
            go_jobs: None,
            compiler: None,
            paths: PathSelection::default(),
        };
        let input = UpdateInputLock {
            source_sha256: Some(sha256_file(&source).unwrap()),
            artifacts_sha256: "irrelevant".into(),
        };
        assert!(replay_input_is_unchanged(&replay, &input).unwrap());
        std::fs::write(&source, "openapi: 3.1.1\n").unwrap();
        assert!(!replay_input_is_unchanged(&replay, &input).unwrap());
    }

    #[test]
    fn parses_openapi_directory_commands() {
        let Action::Discover(options) =
            parse(arguments("discover github --limit 5 --format json")).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.query, "github");
        assert_eq!(options.limit, 5);
        assert_eq!(options.format, DiscoverFormat::Json);
        assert!(parse(arguments("discover --limit 0 github")).is_err());

        let Action::Download(options) = parse(arguments(
            "download github.com --version 1.1.4 --output contract.yaml",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.id, "github.com");
        assert_eq!(options.version.as_deref(), Some("1.1.4"));
        assert_eq!(options.output, PathBuf::from("contract.yaml"));
        assert!(parse(arguments("download github.com")).is_err());
    }

    #[test]
    fn parses_machine_readable_gradual_check_options() {
        let Action::Check(options) = parse(arguments(
            "check openapi.yaml --json --severity missing-operation-id=warning --fail-on none --baseline old.json --write-baseline next.json --ignore ambiguous-path",
        ))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.format, CheckFormat::Json);
        assert_eq!(
            options.severity_overrides["missing-operation-id"],
            CheckSeverity::Warning
        );
        assert_eq!(options.fail_on, CheckFailureThreshold::None);
        assert_eq!(options.baseline, Some(PathBuf::from("old.json")));
        assert_eq!(options.write_baseline, Some(PathBuf::from("next.json")));
        assert!(options.ignored_rules.contains("ambiguous-path"));
        assert!(parse(arguments("check openapi.yaml --severity unknown=warning")).is_err());
        assert!(
            parse(arguments(
                "check openapi.yaml --severity missing-operation-id=notice"
            ))
            .is_err()
        );
    }

    #[test]
    fn check_baselines_use_stable_fingerprints() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("baseline.json");
        let diagnostics = vec![CheckDiagnostic {
            code: "missing-operation-id",
            severity: CheckSeverity::Error,
            method: "GET".into(),
            path: "/users".into(),
            message: "irrelevant to the baseline".into(),
            hint: "irrelevant to the baseline".into(),
        }];
        write_check_baseline(&path, &diagnostics).unwrap();
        assert_eq!(
            load_check_baseline(&path).unwrap(),
            BTreeSet::from(["missing-operation-id:GET:/users".into()])
        );
        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(document["version"], 1);
        assert_eq!(
            document["$schema"],
            "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/check-baseline.schema.json"
        );
    }

    #[test]
    fn contract_check_reports_generator_facing_issues() {
        let api = Api {
            operations: vec![
                Operation {
                    id: "inferred".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/admin//users/{userId}".into(),
                    parameters: vec![OperationParameter {
                        name: "userId".into(),
                        location: "path".into(),
                        required: false,
                        schema: None,
                        description: None,
                        annotations: BTreeMap::new(),
                    }],
                    responses: vec![],
                    ..Operation::default()
                },
                Operation {
                    id: "get-user".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/users/{id}".into(),
                    responses: vec![OperationResponse::json(
                        "200",
                        kaji_core::SchemaValue::unknown(),
                    )],
                    ..Operation::default()
                },
                Operation {
                    id: "get_user".into(),
                    method: kaji_core::HttpMethod::Get,
                    path: "/people".into(),
                    responses: vec![OperationResponse::json(
                        "200",
                        kaji_core::SchemaValue::unknown(),
                    )],
                    ..Operation::default()
                },
            ],
            ..Api::default()
        };
        let source = vec![
            CheckSidecarOperation {
                operation_id: String::new(),
            },
            CheckSidecarOperation {
                operation_id: "get-user".into(),
            },
            CheckSidecarOperation {
                operation_id: "get_user".into(),
            },
        ];
        let codes = check_api(&api, &source)
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"missing-operation-id"));
        assert!(codes.contains(&"ambiguous-path"));
        assert!(codes.contains(&"optional-path-parameter"));
        assert!(codes.contains(&"missing-success-response"));
        assert!(codes.contains(&"missing-path-parameter"));
        assert!(codes.contains(&"ambiguous-operation-id"));
    }

    #[test]
    fn native_mock_paths_require_values_and_prefer_literal_routes() {
        assert!(mock_path_matches("/users/{id}", "/users/me"));
        assert!(!mock_path_matches("/users/{id}", "/users/"));
        assert!(mock_path_specificity("/users/me") > mock_path_specificity("/users/{id}"));
    }

    #[test]
    fn native_mock_decodes_query_and_path_values_for_scenarios() {
        let (path, query) = mock_target_parts("/notes/note%201?expand=full%20body&tag=a%2Bb");
        assert_eq!(path, "/notes/note%201");
        assert_eq!(query["expand"], "full body");
        assert_eq!(query["tag"], "a+b");
        let parameters = mock_path_parameters("/notes/{noteId}", path);
        assert_eq!(parameters["noteId"], "note 1");
        assert_eq!(
            mock_response_body("text/plain", Some(serde_json::json!("rate limited"))).unwrap(),
            b"rate limited"
        );
    }

    #[test]
    fn remote_input_applies_headers_and_basic_auth_before_downloading() {
        let source = RemoteInput {
            url: "https://example.com/openapi.yaml".into(),
            headers: BTreeMap::from([("X-OpenAPI-Key".into(), SecretValue::Literal("key".into()))]),
            auth: Some(RemoteAuth::Basic {
                username: SecretValue::Literal("alice".into()),
                password: SecretValue::Literal("secret".into()),
            }),
        };
        let client = reqwest::blocking::Client::builder().build().unwrap();
        let request = remote_request(&client, &source).unwrap().build().unwrap();
        assert_eq!(request.url().as_str(), source.url);
        assert_eq!(request.headers()["x-openapi-key"], "key");
        assert_eq!(request.headers()["authorization"], "Basic YWxpY2U6c2VjcmV0");
    }

    #[test]
    fn config_profiles_emit_a_typescript_cli_with_oauth() {
        let packages: Vec<PackageConfig> = serde_json::from_value(serde_json::json!([
            {
                "language": "typescript-cli",
                "path": "cli",
                "name": "@acme/cli",
                "plugins": [{
                    "name": "cli",
                    "command_name": "acme",
                    "base_url": "https://api.acme.test/v1",
                    "oauth": {
                        "security_scheme": "OAuth",
                        "client_id": "acme-cli",
                        "preferred_flow": "device",
                        "device_authorization_url": "https://auth.acme.test/device",
                        "token_url": "https://auth.acme.test/token",
                        "scopes": ["projects:read"]
                    }
                }]
            }
        ]))
        .unwrap();
        let profiles = config_profiles(SdkClientStyle::Namespaced, &packages).unwrap();
        let api = Api {
            name: "Acme".into(),
            version: "1.0.0".into(),
            operations: vec![kaji_core::Operation {
                id: "listProjects".into(),
                method: kaji_core::HttpMethod::Get,
                path: "/projects".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tree = kaji::generate(&api, profiles).unwrap();
        assert!(
            tree.get("./cli/package.json")
                .unwrap()
                .contains("@acme/cli")
        );
        let index = tree.get("./cli/src/index.ts").unwrap();
        assert!(index.contains("\"projects\",\n      \"list\""));
        assert!(index.contains("deviceAuthorizationUrl"));
        assert!(index.contains("ACME_TOKEN"));
        assert!(
            tree.get("./cli/src/runtime.ts")
                .unwrap()
                .contains("browserLogin")
        );
    }
}
