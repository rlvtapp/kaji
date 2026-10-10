//! Bound GraphQL clients keep configuration separate from individual requests.
use super::*;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphqlStyle {
    Raw,
    Flat,
    #[default]
    #[serde(alias = "namespaced")]
    Idiomatic,
}
fn group(kind: GraphqlOperationKind) -> &'static str {
    match kind {
        GraphqlOperationKind::Query => "query",
        GraphqlOperationKind::Mutation => "mutation",
        GraphqlOperationKind::Subscription => "subscription",
    }
}
#[cfg(test)]
pub(super) fn render(
    out: &mut String,
    style: GraphqlStyle,
    contract: &GraphqlOperations,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<BTreeMap<String, String>> {
    render_internal(out, style, contract, custom, None)
}
pub(super) fn render_files(
    out: &mut String,
    style: GraphqlStyle,
    contract: &GraphqlOperations,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
    files: &mut BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    render_internal(out, style, contract, custom, Some(files))
}
fn render_internal(
    out: &mut String,
    style: GraphqlStyle,
    contract: &GraphqlOperations,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
    mut files: Option<&mut BTreeMap<String, String>>,
) -> Result<BTreeMap<String, String>> {
    ensure!(
        custom.is_empty() || style == GraphqlStyle::Idiomatic,
        "GraphQL custom groups require idiomatic/namespaced style"
    );
    let mut assignments = BTreeMap::new();
    for (namespace, members) in custom {
        identifier(namespace)?;
        ensure!(
            !matches!(
                namespace.as_str(),
                "then" | "constructor" | "prototype" | "__proto__"
            ),
            "reserved GraphQL client group {namespace:?}"
        );
        for (method, operation) in members {
            identifier(method)?;
            ensure!(
                contract.operations.iter().any(|op| &op.name == operation),
                "GraphQL group {namespace}.{method} references unknown operation {operation:?}"
            );
            ensure!(
                assignments
                    .insert(operation.as_str(), (namespace.as_str(), method.clone()))
                    .is_none(),
                "GraphQL operation {operation:?} assigned to multiple client groups"
            );
        }
    }
    if style == GraphqlStyle::Raw {
        return Ok(contract
            .operations
            .iter()
            .map(|op| (op.name.clone(), crate::symbols::camel(&op.name)))
            .collect());
    }
    let mut allocated = BTreeSet::new();
    let mut methods = BTreeMap::new();
    let mut groups = BTreeMap::<&str, Vec<(String, String)>>::new();
    let model_prefix = if files.is_some() { "models." } else { "" };
    let operation_prefix = if files.is_some() { "operations." } else { "" };
    let mut ordered_operations: Vec<_> = contract.operations.iter().collect();
    ordered_operations.sort_by_key(|op| &op.name);
    for op in ordered_operations {
        let (namespace, name) = if let Some((namespace, method)) = assignments.get(op.name.as_str())
        {
            (*namespace, method.clone())
        } else {
            (
                if style == GraphqlStyle::Flat {
                    ""
                } else {
                    group(op.kind)
                },
                crate::symbols::camel(&op.name),
            )
        };
        ensure!(
            !matches!(
                name.as_str(),
                "then" | "constructor" | "prototype" | "__proto__"
            ),
            "GraphQL client method {name:?} is reserved; rename operation {}",
            op.name
        );
        ensure!(
            allocated.insert((namespace, name.clone())),
            "GraphQL client method collision at {namespace}.{name}; rename operation {}",
            op.name
        );
        let signature = format!(
            "(variables: {model_prefix}{}Variables, options?: RequestOptions) => {}<GraphqlResult<{model_prefix}{}Result>>",
            op.name,
            if op.kind == GraphqlOperationKind::Subscription {
                "AsyncIterable"
            } else {
                "Promise"
            },
            op.name
        );
        let transport = if op.kind == GraphqlOperationKind::Subscription {
            "subscriptionTransport!"
        } else {
            "transport!"
        };
        let binding = format!(
            "(variables: {model_prefix}{}Variables, options?: RequestOptions) => {operation_prefix}{}({transport}, variables, options)",
            op.name, op.name
        );
        groups
            .entry(namespace)
            .or_default()
            .push((format!("{name}: {signature}"), format!("{name}: {binding}")));
        methods.insert(
            op.name.clone(),
            if namespace.is_empty() {
                name
            } else {
                format!("{namespace}.{name}")
            },
        );
    }
    let http = contract
        .operations
        .iter()
        .any(|op| op.kind != GraphqlOperationKind::Subscription);
    let subscriptions = contract
        .operations
        .iter()
        .any(|op| op.kind == GraphqlOperationKind::Subscription);
    writeln!(
        out,
        "\nimport {{ createGraphqlHttpTransport }} from './graphql-runtime.js';\nimport {{ withGraphqlScalarCodecs }} from './graphql-scalar-runtime.js';\nimport type {{ GraphqlScalarCodecs }} from './graphql-codecs.js';"
    )?;
    let connection = if http {
        "({ transport: GraphqlTransport; endpoint?: never } | { endpoint: string; transport?: never })"
    } else {
        "{ endpoint?: string; transport?: GraphqlTransport }"
    };
    writeln!(
        out,
        "export type GraphqlClientOptions = {connection} & {{ headers?: HeadersInit; fetch?: typeof globalThis.fetch; scalarCodecs?: GraphqlScalarCodecs; subscriptionTransport{}: SubscriptionTransport }};",
        if subscriptions { "" } else { "?" }
    )?;
    let mut modular_bindings = Vec::new();
    if let Some(files) = files.as_deref_mut() {
        let mut declarations = Vec::new();
        for (group, members) in &groups {
            let label = poolster_core::files::source_file_stem(if group.is_empty() {
                "flat"
            } else {
                group
            });
            let mut types = Vec::new();
            let mut calls = Vec::new();
            for (index, chunk) in members.chunks(64).enumerate() {
                let name = format!("ClientPart{label}_{index}");
                let create = format!("createPart{label}_{index}");
                let path = format!("graphql/client/{label}_{index}.ts");
                let fields = chunk
                    .iter()
                    .map(|(sig, _)| sig.as_str())
                    .collect::<Vec<_>>()
                    .join(";\n  ");
                let bindings = chunk
                    .iter()
                    .map(|(_, binding)| binding.as_str())
                    .collect::<Vec<_>>()
                    .join(",\n    ");
                files.insert(path.clone(), format!("/** Generated by Poolster. */\nimport type {{ GraphqlTransport, SubscriptionTransport, GraphqlResult, RequestOptions }} from '../../graphql-runtime.js';\nimport type * as models from '../../graphql-models.js';\nimport * as operations from '../../graphql-operations.js';\nexport type {name} = {{\n  {fields};\n}};\nexport function {create}(transport: GraphqlTransport | undefined, subscriptionTransport: SubscriptionTransport | undefined): {name} {{\n  return {{\n    {bindings},\n  }};\n}}\n"));
                writeln!(
                    out,
                    "import {{ {create}, type {name} }} from './{}.js';",
                    path.trim_end_matches(".ts")
                )?;
                types.push(name);
                calls.push(format!("...{create}(transport, subscriptionTransport)"));
            }
            if group.is_empty() {
                declarations.push(types.join(" & "));
                modular_bindings.push(calls.join(",\n    "));
            } else {
                declarations.push(format!("{{ {group}: {} }}", types.join(" & ")));
                modular_bindings.push(format!(
                    "{group}: {{\n      {},\n    }}",
                    calls.join(",\n      ")
                ));
            }
        }
        writeln!(
            out,
            "export type GraphqlBoundClient = {};",
            declarations.join(" & ")
        )?;
    } else {
        let declarations = groups
            .iter()
            .map(|(group, members)| {
                let fields = members
                    .iter()
                    .map(|(signature, _)| signature.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
                if group.is_empty() {
                    fields
                } else {
                    format!("{group}: {{ {fields} }}")
                }
            })
            .collect::<Vec<_>>();
        writeln!(
            out,
            "export type GraphqlBoundClient = {{ {} }};",
            declarations.join("; ")
        )?;
    }
    writeln!(
        out,
        "export function createClient(config: GraphqlClientOptions): GraphqlBoundClient {{\n  const transport = withGraphqlScalarCodecs(config.transport ?? (config.endpoint !== undefined ? createGraphqlHttpTransport(config.endpoint, config) : undefined), config.scalarCodecs);\n  const subscriptionTransport = withGraphqlScalarCodecs(config.subscriptionTransport, config.scalarCodecs);"
    )?;
    if http {
        writeln!(
            out,
            "  if (!transport) throw new Error('GraphQL client requires an endpoint or transport');"
        )?;
    }
    if subscriptions {
        writeln!(
            out,
            "  if (!subscriptionTransport) throw new Error('GraphQL client requires a separate subscription transport');"
        )?;
    }
    let bindings: Vec<_> = groups
        .iter()
        .map(|(group, members)| {
            let fields = members
                .iter()
                .map(|(_, binding)| binding.clone())
                .collect::<Vec<_>>()
                .join(", ");
            if group.is_empty() {
                fields
            } else {
                format!("{group}: {{ {fields} }}")
            }
        })
        .collect();
    let bindings = if files.is_some() {
        modular_bindings.join(",\n    ")
    } else {
        bindings.join(", ")
    };
    writeln!(out, "  return {{\n    {bindings},\n  }};\n}}")?;
    Ok(methods)
}

#[cfg(test)]
#[path = "facade_tests.rs"]
mod tests;
