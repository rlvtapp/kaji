//! Secret-free generation inventory and direct-run replay data.

use std::path::Path;

use anyhow::{Result, bail};
use poolster_core::Api;
use serde::{Deserialize, Serialize};

use super::{
    Generate, OpenApiInput, PathSelection, SdkClientStyle, TypeScriptTransport, configured_labels,
    remote_spec_url, sha256_directory, sha256_file,
};

pub(super) const GENERATION_LOCK_VERSION: u8 = 1;
pub(super) const GENERATION_LOCK_PATH: &str = ".poolster/generation.lock.json";

/// A deliberately small, secret-free account of exactly what Poolster rendered.
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
/// non-secret inputs so `poolster update` can faithfully replay it later.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct GenerationReplayLock {
    pub(super) source: Option<String>,
    pub(super) artifacts: Option<String>,
    pub(super) languages: Vec<String>,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) client_style: String,
    pub(super) typescript_transport: Option<String>,
    pub(super) typescript_surface: String,
    pub(super) typescript_client_name: Option<String>,
    pub(super) go_jobs: Option<usize>,
    pub(super) compiler: Option<String>,
    pub(super) paths: PathSelection,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateLock {
    pub(super) version: u8,
    pub(super) input: UpdateInputLock,
    #[serde(default)]
    pub(super) replay: Option<GenerationReplayLock>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateInputLock {
    #[serde(default)]
    pub(super) source_sha256: Option<String>,
    pub(super) artifacts_sha256: String,
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

pub(super) fn generation_lock(artifacts: &Path, options: &Generate, api: &Api) -> Result<String> {
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
            name: "poolster",
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
    Ok(format!("{}\n", serde_json::to_string_pretty(&lock)?))
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
