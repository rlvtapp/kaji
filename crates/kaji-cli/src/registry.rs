use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const DIRECTORY_URL: &str = "https://api.apis.guru/v2/list.json";

#[derive(Debug, Deserialize)]
pub struct Directory(BTreeMap<String, DirectoryEntry>);

#[derive(Debug, Deserialize)]
struct DirectoryEntry {
    preferred: Option<String>,
    versions: BTreeMap<String, DirectoryVersion>,
}

#[derive(Debug, Deserialize)]
struct DirectoryVersion {
    info: DirectoryInfo,
    #[serde(rename = "swaggerUrl")]
    swagger_url: Option<String>,
    #[serde(rename = "swaggerYamlUrl")]
    swagger_yaml_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DirectoryInfo {
    title: Option<String>,
    description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Api {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: String,
    pub openapi_url: String,
}

impl Directory {
    pub fn parse(document: &str) -> Result<Self> {
        serde_json::from_str(document).context("parse API directory")
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Api> {
        let query = query.trim().to_ascii_lowercase();
        let mut matches = self
            .0
            .iter()
            .filter_map(|(id, entry)| {
                let version_name = entry.preferred.as_deref()?;
                let version = entry.versions.get(version_name)?;
                let title = version.info.title.as_deref().unwrap_or(id);
                let description = version.info.description.as_deref();
                let id_match = id.to_ascii_lowercase();
                let title_match = title.to_ascii_lowercase();
                let rank = if query.is_empty() || id_match == query {
                    Some(0)
                } else if id_match.contains(&query) {
                    Some(1)
                } else if title_match.contains(&query) {
                    Some(2)
                } else if description
                    .is_some_and(|value| value.to_ascii_lowercase().contains(&query))
                {
                    Some(3)
                } else {
                    None
                };
                rank.map(|rank| (rank, Self::api(id, version_name, version)))
            })
            .collect::<Vec<_>>();
        matches.sort_by(|(left_rank, left), (right_rank, right)| {
            left_rank
                .cmp(right_rank)
                .then_with(|| left.id.cmp(&right.id))
        });
        matches
            .into_iter()
            .map(|(_, api)| api)
            .take(limit)
            .collect()
    }

    pub fn resolve(&self, id: &str, requested_version: Option<&str>) -> Result<Api> {
        let entry = self
            .0
            .get(id)
            .with_context(|| format!("API {id:?} was not found in the directory"))?;
        let version_name = match requested_version {
            Some(version) => version,
            None => entry
                .preferred
                .as_deref()
                .with_context(|| format!("API {id:?} has no preferred version"))?,
        };
        let version = entry.versions.get(version_name).with_context(|| {
            format!("API {id:?} does not have version {version_name:?} in the directory")
        })?;
        Ok(Self::api(id, version_name, version))
    }

    fn api(id: &str, version_name: &str, version: &DirectoryVersion) -> Api {
        // YAML makes a downloaded spec easy to inspect and edit. Some entries
        // only publish JSON, so retain that as a safe fallback.
        let openapi_url = version
            .swagger_yaml_url
            .as_ref()
            .or(version.swagger_url.as_ref())
            .cloned()
            .unwrap_or_default();
        Api {
            id: id.into(),
            title: version.info.title.clone().unwrap_or_else(|| id.into()),
            description: version.info.description.clone(),
            version: version_name.into(),
            openapi_url,
        }
    }
}

pub fn validate_download(api: &Api) -> Result<()> {
    if api.openapi_url.is_empty() {
        bail!(
            "API {:?} version {:?} does not publish an OpenAPI download URL",
            api.id,
            api.version
        )
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECTORY: &str = r#"{
      "example.com": {
        "preferred": "2.0.0",
        "versions": {
          "1.0.0": {"info": {"title": "Example v1"}, "swaggerUrl": "https://example.test/v1.json"},
          "2.0.0": {"info": {"title": "Example API", "description": "Searchable contract"}, "swaggerUrl": "https://example.test/v2.json", "swaggerYamlUrl": "https://example.test/v2.yaml"}
        }
      }
    }"#;

    #[test]
    fn search_uses_preferred_version_and_matches_metadata() {
        let directory = Directory::parse(DIRECTORY).unwrap();
        assert_eq!(
            directory.search("CONTRACT", 10),
            vec![Api {
                id: "example.com".into(),
                title: "Example API".into(),
                description: Some("Searchable contract".into()),
                version: "2.0.0".into(),
                openapi_url: "https://example.test/v2.yaml".into(),
            }]
        );
    }

    #[test]
    fn search_ranks_an_identifier_match_before_a_description_match() {
        let directory = Directory::parse(
            r#"{
              "another.example": {"preferred":"1", "versions":{"1":{"info":{"title":"Another", "description":"GitHub is mentioned here"},"swaggerUrl":"https://example.test/another"}}},
              "github.com": {"preferred":"1", "versions":{"1":{"info":{"title":"GitHub API"},"swaggerUrl":"https://example.test/github"}}}
            }"#,
        )
        .unwrap();
        assert_eq!(directory.search("github", 1)[0].id, "github.com");
    }

    #[test]
    fn resolve_can_select_a_non_preferred_version() {
        let directory = Directory::parse(DIRECTORY).unwrap();
        let api = directory.resolve("example.com", Some("1.0.0")).unwrap();
        assert_eq!(api.title, "Example v1");
        assert_eq!(api.openapi_url, "https://example.test/v1.json");
    }
}
