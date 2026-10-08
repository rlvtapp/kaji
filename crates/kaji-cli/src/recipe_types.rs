use super::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectConfig {
    #[serde(rename = "$schema")]
    pub(super) _schema: Option<String>,
    pub(super) openapi: OpenApiConfig,
    pub(super) output: OutputConfig,
    #[serde(default)]
    pub(super) defaults: DefaultsConfig,
    #[serde(default)]
    pub(super) packages: Vec<PackageConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OpenApiConfig {
    pub(super) input: Option<OpenApiInput>,
    pub(super) artifacts: Option<PathBuf>,
    #[serde(default = "default_api_name")]
    pub(super) name: String,
    #[serde(default = "default_sdk_version")]
    pub(super) version: String,
    pub(super) compiler: Option<PathBuf>,
    #[serde(default)]
    pub(super) paths: PathSelection,
}

/// Path filters intentionally use the same small glob language as Poolster's
/// operation filters: `*` matches any sequence (including `/`) and `?` one
/// Unicode scalar. Includes form an OR-set; an exclusion always wins.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct PathSelection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum OpenApiInput {
    Path(PathBuf),
    Remote(RemoteInput),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RemoteInput {
    pub(super) url: String,
    #[serde(default)]
    pub(super) headers: BTreeMap<String, SecretValue>,
    pub(super) auth: Option<RemoteAuth>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum SecretValue {
    Literal(String),
    Environment { env: String },
    Profile { profile: String },
}

impl SecretValue {
    pub(super) fn resolve(&self, field: &str) -> Result<String> {
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
pub(super) enum RemoteAuth {
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
pub(super) struct OutputConfig {
    pub(super) path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefaultsConfig {
    pub(super) client_style: Option<String>,
    pub(super) layout: Option<poolster_core::SourceLayout>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PackageConfig {
    pub(super) language: String,
    pub(super) path: String,
    pub(super) name: Option<String>,
    pub(super) version: Option<String>,
    pub(super) release: Option<poolster_core::release::PackageMetadata>,
    pub(super) client_style: Option<String>,
    pub(super) layout: Option<poolster_core::SourceLayout>,
    #[serde(default)]
    pub(super) api_reference: bool,
    #[serde(default)]
    pub(super) idempotency: poolster_core::idempotency::IdempotencyConfig,
    #[serde(default)]
    pub(super) plugins: Vec<PluginConfig>,
    #[serde(default)]
    pub(super) customizations: Vec<CodeCustomizationConfig>,
    #[serde(skip)]
    pub(super) resolved_customizations: Vec<poolster_core::customization::CodeCustomization>,
    #[serde(default)]
    pub(super) middleware: Vec<BundledMiddlewareConfig>,
    #[serde(skip)]
    pub(super) resolved_middleware: Vec<poolster_core::customization::BundledMiddleware>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BundledMiddlewareConfig {
    pub(super) source: PathBuf,
    pub(super) path: PathBuf,
    pub(super) symbol: String,
    pub(super) async_symbol: Option<String>,
}
impl BundledMiddlewareConfig {
    pub(super) fn load(
        &self,
        base: &Path,
    ) -> Result<poolster_core::customization::BundledMiddleware> {
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
pub(super) enum CodeCustomizationConfig {
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
    pub(super) fn load(
        &self,
        base: &Path,
    ) -> Result<poolster_core::customization::CodeCustomization> {
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
pub(super) struct PluginConfig {
    pub(super) name: String,
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) uses: BTreeMap<String, String>,
    pub(super) integer_as_string: Option<bool>,
    pub(super) int64: Option<String>,
    pub(super) async_client: Option<bool>,
    pub(super) open_enums: Option<bool>,
    pub(super) open_unions: Option<bool>,
    pub(super) preserve_presence: Option<bool>,
    pub(super) transport: Option<String>,
    pub(super) surface: Option<String>,
    pub(super) client_name: Option<String>,
    pub(super) group_by_tag: Option<bool>,
    pub(super) split_by_group: Option<bool>,
    pub(super) max_operations_per_file: Option<usize>,
    pub(super) layout: Option<poolster_core::SourceLayout>,
    pub(super) include_operations: Option<Vec<String>>,
    pub(super) operation_kinds: Option<BTreeMap<String, String>>,
    pub(super) operation_names: Option<BTreeMap<String, String>>,
    pub(super) fixture_options: Option<poolster::ts::FixtureOptions>,
    pub(super) cypress_options: Option<poolster::ts::CypressOptions>,
    pub(super) max_file_bytes: Option<usize>,
    pub(super) throw_on_error: Option<bool>,
    pub(super) jobs: Option<usize>,
    pub(super) output: Option<String>,
    pub(super) clients_import: Option<String>,
    pub(super) openapi_spec: Option<String>,
    pub(super) title: Option<String>,
    pub(super) image: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) command_name: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) oauth: Option<CliOAuthConfig>,
    pub(super) sdk_package: Option<String>,
    pub(super) strict: Option<bool>,
    pub(super) infer: Option<bool>,
    pub(super) data_sources: Option<bool>,
    pub(super) module: Option<String>,
    pub(super) provider_name: Option<String>,
    pub(super) registry_namespace: Option<String>,
    #[serde(default)]
    pub(super) resources: Vec<terraform::ResourceBinding>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CliOAuthConfig {
    pub(super) client_id: String,
    pub(super) security_scheme: Option<String>,
    #[serde(default)]
    pub(super) scopes: Vec<String>,
    pub(super) preferred_flow: Option<String>,
    pub(super) authorization_url: Option<String>,
    pub(super) device_authorization_url: Option<String>,
    pub(super) token_url: Option<String>,
    pub(super) redirect_uri: Option<String>,
}

pub(super) fn default_api_name() -> String {
    "API".into()
}
pub(super) fn default_sdk_version() -> String {
    "0.1.0".into()
}
