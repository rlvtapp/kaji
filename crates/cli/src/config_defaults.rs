//! Resolve shared project settings before constructing package plugins.
use super::ProjectConfig;
use anyhow::Result;

pub(super) fn apply(config: &mut ProjectConfig) -> Result<()> {
    if let Some(layout) = &config.defaults.layout {
        layout.groups(&[], 0)?;
    }
    for package in &mut config.packages {
        if package.layout.is_none() {
            package.layout = config.defaults.layout.clone();
        }
        if let Some(layout) = &package.layout {
            layout.groups(&[], 0)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::SourceLayout;

    #[test]
    fn layout_defaults_inherit_and_explicit_overrides_survive() {
        let mut config: ProjectConfig = serde_json::from_value(serde_json::json!({
            "openapi":{"input":"api.yaml"}, "output":{"path":"generated"},
            "defaults":{"layout":{"mode":"per-operation"}},
            "packages":[
                {"language":"typescript","path":"one","plugins":[{"name":"sdk"},{"name":"swr","layout":{"mode":"single-file"}}]},
                {"language":"typescript","path":"two","layout":{"mode":"per-resource"},"plugins":[{"name":"sdk"}]},
                {"language":"go","path":"go","plugins":[{"name":"sdk"}]}
            ]
        })).unwrap();
        apply(&mut config).unwrap();
        assert_eq!(config.packages[0].layout, Some(SourceLayout::PerOperation));
        assert_eq!(
            config.packages[0].plugins[1].layout,
            Some(SourceLayout::SingleFile)
        );
        assert!(matches!(
            config.packages[1].layout,
            Some(SourceLayout::PerResource { .. })
        ));
        assert!(config.packages[0].plugins[0].layout.is_none());
        apply(&mut config).unwrap();
        assert_eq!(config.packages[0].layout, Some(SourceLayout::PerOperation));
    }

    #[test]
    fn invalid_unused_shared_budget_is_rejected() {
        let mut config: ProjectConfig = serde_json::from_value(serde_json::json!({
            "openapi":{"input":"api.yaml"}, "output":{"path":"generated"},
            "defaults":{"layout":{"mode":"chunked","max_file_bytes":0}},
            "packages":[]
        }))
        .unwrap();
        assert!(apply(&mut config).is_err());
    }

    #[test]
    fn shared_layout_reaches_query_generation_and_plugin_can_opt_out() {
        let mut config: ProjectConfig = serde_json::from_value(serde_json::json!({
            "openapi":{"input":"api.yaml"}, "output":{"path":"generated"},
            "defaults":{"layout":{"mode":"per-operation"}},
            "packages":[{"language":"typescript","path":"ts","plugins":[
                {"name":"sdk"}, {"name":"tanstack-react-query"},
                {"name":"swr","layout":{"mode":"single-file"}}
            ]}]
        }))
        .unwrap();
        apply(&mut config).unwrap();
        let api = poolster_core::Api {
            name: "Example".into(),
            version: "1.0.0".into(),
            operations: vec![poolster_core::Operation {
                id: "readItem".into(),
                method: poolster_core::HttpMethod::Get,
                path: "/items".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let profiles = super::super::config_profiles(
            poolster_core::SdkClientStyle::Namespaced,
            &config.packages,
        )
        .unwrap();
        let tree = poolster::generate(&api, profiles).unwrap();
        assert!(tree.iter().any(|(path, _)| {
            path.ends_with(std::path::Path::new("react-query_operations").join("readItem.ts"))
        }));
        assert!(
            tree.iter()
                .any(|(path, _)| path.ends_with(std::path::Path::new("ts").join("swr.ts")))
        );
        assert!(!tree.iter().any(|(path, _)| {
            path.components()
                .any(|part| part.as_os_str() == "swr_operations")
        }));
    }
}
