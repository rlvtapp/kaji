use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use kaji::ts::artifacts::{
    ArtifactOptions, McpToolManifest, ReDoc, TypeScriptCypress, TypeScriptFaker, TypeScriptMsw,
    TypeScriptReactQuery, TypeScriptSwr, TypeScriptVueQuery, TypeScriptZod,
};
use kaji::{SdkClientStyle, dotnet, elixir, go, java, mock, php, prelude::*, python, rust, ts};
use kaji_core::{Api, GeneratedFile, GeneratedTree};
use serde::Deserialize;

const HELP: &str = "Kaji — native multi-language OpenAPI SDK generator

Usage:
  kaji init [--config <file>] [--input <openapi-file>] [--output <directory>]
  kaji generate                         # reads ./kaji.json
  kaji generate --config <file>
  kaji generate <openapi-file> --output <directory> --language <target>...
  kaji generate --artifacts <directory> --output <directory> --language <target>...
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
  -h, --help                          Show help

Targets: rust, typescript, go, python, php, java, dotnet, elixir

Each target is written to its own subdirectory. Generated files are overwritten;
custom starter files and unrelated files are preserved. Input must be a local file.
The npm distribution bundles both native executables; Rust and Go are not required.
";

const LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "go",
    "python",
    "php",
    "java",
    "dotnet",
    "elixir",
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
    jobs: usize,
    color: ColorChoice,
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
}

impl SecretValue {
    fn resolve(&self, field: &str) -> Result<String> {
        match self {
            Self::Literal(value) => Ok(value.clone()),
            Self::Environment { env: variable } => env::var(variable)
                .with_context(|| format!("read environment variable {variable:?} for {field}")),
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
    Init(Init),
    Generate(Box<Generate>),
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
                                LANGUAGES
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
            "go" => profiles.package(go::package("go").with(go::sdk().jobs(options.jobs))),
            "python" => profiles.package(python::package("python").with(python::sdk())),
            "php" => profiles.package(php::package("php").with(php::sdk())),
            "java" => profiles.package(java::package("java").with(java::sdk())),
            "dotnet" => profiles.package(dotnet::package("dotnet").with(dotnet::sdk())),
            "elixir" => profiles.package(elixir::package("elixir").with(elixir::sdk())),
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
            "dotnet" => {
                has_only_known_plugins(package, &["sdk"])?;
                let package_builder = dotnet::package(&package.path).common(package_common(style));
                let package_builder = if let Some(name) = &package.name {
                    package_builder.name(name)
                } else {
                    package_builder
                };
                profiles.package(package_builder.with(dotnet::sdk()))
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
                "unknown config language {other:?}; use typescript, rust, go, python, php, java, dotnet, elixir, mock, or artifacts"
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
        "$schema": "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/kaji.schema.json",
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
            if plugin.name == "sdk" || plugin.name == "server" {
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
                "zod" => Some(("zod", "^3.0.0")),
                "tanstack-react-query" => Some(("@tanstack/react-query", "^5.0.0")),
                "tanstack-vue-query" => Some(("@tanstack/vue-query", "^5.0.0")),
                "swr" => Some(("swr", "^2.0.0")),
                "faker" => Some(("@faker-js/faker", "^9.0.0")),
                "msw" => Some(("msw", "^2.0.0")),
                _ => None,
            })
            .collect::<Vec<_>>();
        if dependencies.is_empty() {
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
        let entries = object
            .entry("dependencies")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .context("generated TypeScript manifest dependencies must be an object")?;
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
        tree.replace(GeneratedFile::new(
            path,
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?)?;
    }
    Ok(())
}

fn generate(options: Generate) -> Result<()> {
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
    reporter.phase("Writing files", started.elapsed());
    reporter.completed(
        configured_plugin_count(options),
        tree.iter().count(),
        &options.output,
    );
    Ok(())
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
        Action::Init(init) => init_config(init),
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
    fn validates_flags_sources_and_language_specific_options() {
        for invalid in [
            "generate api.yaml -o sdk",
            "generate -o sdk -l go",
            "generate api.yaml -l go",
            "generate api.yaml --artifacts cache -o sdk -l go",
            "generate api.yaml -o sdk -l ruby",
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
        assert_eq!(options.languages.len(), LANGUAGES.len());
        assert_eq!(options.artifacts, Some(PathBuf::from("cache")));
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
}
