use super::*;

#[derive(Debug)]
pub(super) struct Generate {
    pub(super) source: Option<OpenApiInput>,
    pub(super) native_input: Option<NativeInputConfig>,
    pub(super) native_output: NativeOutputConfig,
    pub(super) config: Option<PathBuf>,
    pub(super) config_packages: Option<Vec<PackageConfig>>,
    pub(super) artifacts: Option<PathBuf>,
    pub(super) output: PathBuf,
    pub(super) languages: Vec<String>,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) style: SdkClientStyle,
    pub(super) raw: bool,
    pub(super) typescript_transport: Option<TypeScriptTransport>,
    pub(super) client_name: Option<String>,
    pub(super) compiler: Option<PathBuf>,
    pub(super) path_selection: PathSelection,
    pub(super) config_sha256: Option<String>,
    pub(super) source_sha256: Option<String>,
    pub(super) jobs: usize,
    pub(super) color: ColorChoice,
    pub(super) check: bool,
    pub(super) json_changes: bool,
}

#[derive(Debug)]
pub(super) struct Mcp {
    pub(super) source: PathBuf,
    pub(super) base_url: String,
    pub(super) compiler: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorChoice {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TypeScriptTransport {
    Fetch,
    Axios,
}

impl TypeScriptTransport {
    pub(super) fn parse(value: &str) -> Result<Self> {
        match value {
            "fetch" => Ok(Self::Fetch),
            "axios" => Ok(Self::Axios),
            _ => bail!("--typescript-transport must be fetch or axios"),
        }
    }
}

impl ColorChoice {
    pub(super) fn parse(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => bail!("--color must be auto, always, or never"),
        }
    }

    pub(super) fn enabled(self) -> bool {
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

pub(super) struct Reporter {
    pub(super) color: bool,
    pub(super) started: Instant,
}

impl Reporter {
    pub(super) fn new(choice: ColorChoice) -> Self {
        Self {
            color: choice.enabled(),
            started: Instant::now(),
        }
    }

    pub(super) fn paint(&self, code: &str, value: impl std::fmt::Display) -> String {
        if self.color {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    pub(super) fn gold(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;208", value)
    }

    pub(super) fn blue(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;117", value)
    }

    pub(super) fn green(&self, value: impl std::fmt::Display) -> String {
        self.paint("38;5;77", value)
    }

    pub(super) fn dim(&self, value: impl std::fmt::Display) -> String {
        self.paint("2", value)
    }

    pub(super) fn started(&self) {
        eprintln!(
            "\n{} {}",
            self.gold("$"),
            self.paint("1", "poolster generate")
        );
        eprintln!("  {} Generation started", self.gold("◆"));
    }

    pub(super) fn compiling(&self, source: &Path) {
        eprintln!(
            "  {} Compiling {}",
            self.dim("◇"),
            self.dim(source.display())
        );
    }

    pub(super) fn downloading(&self, url: &str) {
        eprintln!("  {} Downloading {}", self.dim("◇"), self.blue(url));
    }

    pub(super) fn phase(&self, label: &str, elapsed: std::time::Duration) {
        eprintln!(
            "  {} {} {}",
            self.green("◇"),
            self.blue(label),
            self.dim(format!("completed in {}", duration(elapsed)))
        );
    }

    pub(super) fn generated(&self, options: &Generate, elapsed: std::time::Duration) {
        for label in configured_labels(options) {
            eprintln!("  {} {}", self.green("◇"), self.blue(label));
        }
        self.phase("Generation", elapsed);
    }

    pub(super) fn completed(&self, plugins: usize, files: usize, output: &Path) {
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

pub(super) fn duration(value: std::time::Duration) -> String {
    if value.as_secs_f64() >= 1.0 {
        format!("{:.2}s", value.as_secs_f64())
    } else {
        format!("{}ms", value.as_millis())
    }
}

pub(super) fn configured_labels(options: &Generate) -> Vec<String> {
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

pub(super) fn configured_plugin_count(options: &Generate) -> usize {
    options
        .config_packages
        .as_ref()
        .map(|packages| packages.iter().map(|package| package.plugins.len()).sum())
        .unwrap_or(options.languages.len())
}

#[derive(Debug)]
pub(super) struct Init {
    pub(super) config: PathBuf,
    pub(super) input: PathBuf,
    pub(super) output: PathBuf,
    pub(super) name: String,
    pub(super) version: String,
}

#[derive(Debug)]
pub(super) struct MockServe {
    pub(super) source: PathBuf,
    pub(super) port: u16,
    pub(super) compiler: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct Check {
    pub(super) source: PathBuf,
    pub(super) compiler: Option<PathBuf>,
    pub(super) format: CheckFormat,
    pub(super) severity_overrides: BTreeMap<String, CheckSeverity>,
    pub(super) fail_on: CheckFailureThreshold,
    pub(super) baseline: Option<PathBuf>,
    pub(super) write_baseline: Option<PathBuf>,
    pub(super) ignored_rules: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CheckFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum CheckSeverity {
    Warning,
    Error,
}

impl CheckSeverity {
    pub(super) fn parse(value: &str) -> Result<Self> {
        match value {
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            _ => bail!("severity must be warning or error, got {value:?}"),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CheckFailureThreshold {
    Warning,
    Error,
    None,
}

impl CheckFailureThreshold {
    pub(super) fn parse(value: &str) -> Result<Self> {
        match value {
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "none" => Ok(Self::None),
            _ => bail!("--fail-on must be warning, error, or none, got {value:?}"),
        }
    }

    pub(super) fn fails(self, severity: CheckSeverity) -> bool {
        match self {
            Self::Warning => true,
            Self::Error => severity == CheckSeverity::Error,
            Self::None => false,
        }
    }
}
