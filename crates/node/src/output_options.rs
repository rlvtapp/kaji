//! Apply options for the active bundled exporter while retaining independent configs.
use super::*;
use std::collections::BTreeMap;
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ContractOptions {
    style: Option<String>,
    raw: Option<bool>,
    transport: Option<String>,
    client_name: Option<String>,
    jobs: Option<usize>,
    #[serde(default)]
    scalars: BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
fn merge<T: PartialEq>(target: &mut Option<T>, scoped: Option<T>, field: &str) -> AnyResult<()> {
    if let Some(value) = scoped {
        if target.as_ref().is_some_and(|old| old != &value) {
            bail!("conflicting top-level and contract-scoped {field}");
        }
        *target = Some(value);
    }
    Ok(())
}
pub(super) fn apply(package: &mut SdkPackage, contract: &str) -> AnyResult<()> {
    for key in package.contracts.keys() {
        if !["http", "graphql"].contains(&key.as_str()) {
            bail!("unsupported bundled exporter contract {key}");
        }
    }
    let Some(mut scoped) = package.contracts.remove(contract) else {
        return Ok(());
    };
    if contract == "graphql" {
        for style in [&mut package.style, &mut scoped.style] {
            if style
                .as_deref()
                .is_some_and(|s| ["idiomatic", "namespaced", "grouped"].contains(&s))
            {
                *style = Some("idiomatic".into());
            }
        }
    }
    merge(&mut package.style, scoped.style, "style")?;
    merge(&mut package.raw, scoped.raw, "raw")?;
    merge(&mut package.transport, scoped.transport, "transport")?;
    merge(&mut package.client_name, scoped.client_name, "clientName")?;
    merge(&mut package.jobs, scoped.jobs, "jobs")?;
    for (name, mapping) in scoped.scalars {
        if package
            .scalars
            .get(&name)
            .is_some_and(|old| old != &mapping)
        {
            bail!("conflicting scalar mapping {name}");
        }
        package.scalars.insert(name, mapping);
    }
    for (group, methods) in scoped.groups {
        let target = package.groups.entry(group.clone()).or_default();
        for (method, operation) in methods {
            if target.get(&method).is_some_and(|old| old != &operation) {
                bail!("conflicting group method {group}.{method}");
            }
            target.insert(method, operation);
        }
    }
    Ok(())
}
