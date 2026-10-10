//! Native SDK package settings shared across protocol entry points.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SdkPackage {
    #[serde(default)]
    pub(super) source_quality: Option<poolster_core::SourceQuality>,
    pub(super) language: String,
    #[serde(default)]
    pub(super) command_name: Option<String>,
    #[serde(default)]
    pub(super) endpoint: Option<String>,
    pub(super) path: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) style: Option<String>,
    #[serde(default)]
    pub(super) scalars: std::collections::BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    pub(super) groups:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub(super) contracts: std::collections::BTreeMap<String, output_options::ContractOptions>,
    #[serde(default)]
    pub(super) transport: Option<String>,
    #[serde(default)]
    pub(super) client_name: Option<String>,
    #[serde(default)]
    pub(super) raw: Option<bool>,
    #[serde(default)]
    pub(super) subscriptions: Option<bool>,
    #[serde(default)]
    pub(super) jobs: Option<usize>,
    #[serde(default)]
    pub(super) plugins: Vec<NativePlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativePlugin {
    pub(super) name: String,
    #[serde(default)]
    pub(super) fixture_options: Option<ts::FixtureOptions>,
    #[serde(default)]
    pub(super) cypress_options: Option<ts::CypressOptions>,
    #[serde(default)]
    pub(super) output: Option<String>,
}

pub(super) fn package_common(package: &SdkPackage) -> AnyResult<Common> {
    let mut common = Common {
        source_quality: package.source_quality.clone(),
        ..Default::default()
    };
    if let Some(style) = package.style.as_deref() {
        common = common.client_style(match style {
            "flat" => SdkClientStyle::Flat,
            "namespaced" => SdkClientStyle::Namespaced,
            _ => bail!("SDK style must be flat or namespaced"),
        });
    }
    if let Some(version) = &package.version {
        common = common.package_version(version);
    }
    Ok(common)
}

pub(super) fn validate_options(package: &SdkPackage) -> AnyResult<()> {
    if package.command_name.is_some() || package.endpoint.is_some() {
        bail!("commandName and endpoint require GraphQL tool output");
    }
    if !package.scalars.is_empty() || !package.groups.is_empty() {
        bail!("scalar mappings and groups require GraphQL input");
    }
    if package.path.is_empty() {
        bail!("SDK package path must not be empty");
    }
    if package.language != "typescript"
        && (package.transport.is_some()
            || package.client_name.is_some()
            || package.raw.unwrap_or(false))
    {
        bail!("transport, clientName and raw are TypeScript-only SDK options");
    }
    if package.language != "go" && package.jobs.is_some() {
        bail!("jobs is a Go-only SDK option");
    }
    if package.jobs == Some(0) {
        bail!("jobs must be at least 1");
    }
    if package.language != "typescript" && !package.plugins.is_empty() {
        bail!("registered native auxiliaries currently require a TypeScript package");
    }
    Ok(())
}
