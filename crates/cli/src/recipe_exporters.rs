//! Select the active bundled exporter without blending protocol options.
use super::*;
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExporterOptions {
    style: Option<String>,
    raw: Option<bool>,
    transport: Option<String>,
    client_name: Option<String>,
    #[serde(default)]
    scalars: BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
fn merge<T: PartialEq>(target: &mut Option<T>, scoped: Option<T>, field: &str) -> Result<()> {
    if let Some(value) = scoped {
        ensure!(
            target.as_ref().is_none_or(|old| old == &value),
            "conflicting top-level and contract-scoped {field}"
        );
        *target = Some(value);
    }
    Ok(())
}
pub(super) fn apply(config: &mut ProjectConfig) -> Result<()> {
    let active = config
        .input
        .as_ref()
        .map(|input| input.format.as_str())
        .unwrap_or("http");
    for package in &mut config.packages {
        if config
            .input
            .as_ref()
            .is_some_and(|input| !native_profiles::input_compatible(input, package))
        {
            continue;
        }
        for plugin in &mut package.plugins {
            for key in plugin.contracts.keys() {
                ensure!(
                    ["http", "graphql"].contains(&key.as_str()),
                    "unsupported bundled exporter contract {key}"
                );
            }
            let Some(mut scoped) = plugin.contracts.remove(active) else {
                continue;
            };
            merge(&mut plugin.transport, scoped.transport, "transport")?;
            merge(&mut plugin.client_name, scoped.client_name, "client_name")?;
            if active == "http" {
                ensure!(
                    scoped.scalars.is_empty() && scoped.groups.is_empty(),
                    "GraphQL scalar/group options require GraphQL input"
                );
                merge(&mut package.client_style, scoped.style, "client_style")?;
                if let Some(raw) = scoped.raw {
                    merge(
                        &mut plugin.surface,
                        Some(if raw {
                            "raw".to_owned()
                        } else {
                            "client".to_owned()
                        }),
                        "surface",
                    )?;
                }
            } else {
                for style in [&mut plugin.style, &mut scoped.style] {
                    if style
                        .as_deref()
                        .is_some_and(|s| ["idiomatic", "namespaced", "grouped"].contains(&s))
                    {
                        *style = Some("idiomatic".into());
                    }
                }
                merge(&mut plugin.style, scoped.style, "style")?;
                merge(&mut plugin.raw, scoped.raw, "raw")?;
                for (name, mapping) in scoped.scalars {
                    ensure!(
                        plugin.scalars.get(&name).is_none_or(|old| old == &mapping),
                        "conflicting scalar mapping {name}"
                    );
                    plugin.scalars.insert(name, mapping);
                }
                for (group, methods) in scoped.groups {
                    let target = plugin.groups.entry(group.clone()).or_default();
                    for (method, operation) in methods {
                        ensure!(
                            target.get(&method).is_none_or(|old| old == &operation),
                            "conflicting group method {group}.{method}"
                        );
                        target.insert(method, operation);
                    }
                }
            }
        }
    }
    Ok(())
}
