//! Bound clients reuse their configured transport across selected operations.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphqlStyle {
    Raw,
    Flat,
    #[default]
    #[serde(alias = "namespaced")]
    Idiomatic,
}
pub(super) struct Layout {
    pub methods: BTreeMap<String, String>,
    groups: BTreeMap<String, (String, Vec<(String, String)>)>,
    without_variables: BTreeSet<String>,
}
fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value
                .chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit())),
        "invalid Rust GraphQL client name {value:?}"
    );
    Ok(())
}
fn checked_name(value: &str) -> Result<String> {
    identifier(value)?;
    let name = crate::render::rust_field_name(value);
    ensure!(
        !matches!(name.as_str(), "new" | "from_transport"),
        "reserved Rust GraphQL client name {value:?}"
    );
    Ok(name)
}
pub(super) fn layout(
    style: GraphqlStyle,
    contract: &GraphqlOperations,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<Layout> {
    ensure!(
        custom.is_empty() || style == GraphqlStyle::Idiomatic,
        "GraphQL custom groups require idiomatic/namespaced style"
    );
    let mut assignments = BTreeMap::new();
    let mut group_names = BTreeSet::new();
    for (group, members) in custom {
        let group = checked_name(group)?;
        ensure!(
            group_names.insert(group.clone()),
            "Rust GraphQL group name collision: {group}"
        );
        for (method, operation) in members {
            let method = checked_name(method)?;
            ensure!(
                contract.operations.iter().any(|op| &op.name == operation),
                "GraphQL group references unknown operation {operation:?}"
            );
            ensure!(
                assignments
                    .insert(operation.as_str(), (group.clone(), method))
                    .is_none(),
                "GraphQL operation {operation:?} assigned to multiple groups"
            );
        }
    }
    let mut groups = BTreeMap::<String, (String, Vec<(String, String)>)>::new();
    let mut methods = BTreeMap::new();
    let mut allocated = BTreeSet::new();
    let mut types = BTreeSet::from(["Client".to_string()]);
    let mut ordered_operations: Vec<_> = contract.operations.iter().collect();
    ordered_operations.sort_by_key(|op| &op.name);
    for op in ordered_operations {
        if style == GraphqlStyle::Raw {
            continue;
        }
        let (group, method) = assignments
            .get(op.name.as_str())
            .cloned()
            .unwrap_or_else(|| {
                (
                    if style == GraphqlStyle::Flat {
                        String::new()
                    } else {
                        match op.kind {
                            GraphqlOperationKind::Query => "query",
                            GraphqlOperationKind::Mutation => "mutation",
                            GraphqlOperationKind::Subscription => "subscription",
                        }
                        .into()
                    },
                    crate::render::rust_field_name(&op.name),
                )
            });
        ensure!(
            !matches!(method.as_str(), "new" | "from_transport"),
            "reserved Rust GraphQL client method {method}"
        );
        ensure!(
            allocated.insert((group.clone(), method.clone())),
            "Rust GraphQL client method collision at {group}.{method}"
        );
        if !groups.contains_key(&group) {
            let ty = if group.is_empty() {
                "Client".into()
            } else {
                format!("{}Operations", crate::render::type_name(&group))
            };
            ensure!(
                group.is_empty() || types.insert(ty.clone()),
                "Rust GraphQL group type collision: {ty}"
            );
            groups.insert(group.clone(), (ty, vec![]));
        }
        groups
            .get_mut(&group)
            .unwrap()
            .1
            .push((op.name.clone(), method.clone()));
        methods.insert(
            op.name.clone(),
            if group.is_empty() {
                method
            } else {
                format!("{group}().{method}")
            },
        );
    }
    let without_variables = contract
        .operations
        .iter()
        .filter(|op| op.variables.is_empty())
        .map(|op| op.name.clone())
        .collect();
    Ok(Layout {
        methods,
        groups,
        without_variables,
    })
}
impl Layout {
    pub fn reserved(&self) -> Vec<String> {
        std::iter::once("Client".into())
            .chain(self.groups.values().map(|(ty, _)| ty.clone()))
            .collect()
    }
    pub fn render_files(
        &self,
        style: GraphqlStyle,
        operations: &BTreeMap<String, GraphqlOperationSymbols>,
    ) -> Result<BTreeMap<String, String>> {
        let mut files = BTreeMap::new();
        if style == GraphqlStyle::Raw {
            return Ok(files);
        }
        files.insert("client".into(), "pub struct Client { transport: crate::graphql_runtime::GraphqlHttpTransport }\nimpl Client {\n    pub fn new(endpoint: impl Into<String>, client: reqwest::Client) -> Self { Self::from_transport(crate::graphql_runtime::GraphqlHttpTransport::new(endpoint,client)) }\n    pub fn from_transport(transport: crate::graphql_runtime::GraphqlHttpTransport) -> Self { Self { transport } }\n}\n".into());
        for (group, (ty, members)) in &self.groups {
            if !group.is_empty() {
                files.insert(format!("group_{group}"), format!("pub struct {ty}<'a> {{ transport: &'a crate::graphql_runtime::GraphqlHttpTransport }}\nimpl Client {{\n    pub fn {group}(&self) -> {ty}<'_> {{ {ty} {{ transport: &self.transport }} }}\n}}\n"));
            }
            for member in members {
                let mut source = if group.is_empty() {
                    "impl Client {\n".into()
                } else {
                    format!("impl {ty}<'_> {{\n")
                };
                self.render_methods(
                    &mut source,
                    std::slice::from_ref(member),
                    operations,
                    if group.is_empty() {
                        "&self.transport"
                    } else {
                        "self.transport"
                    },
                )?;
                source.push_str("}\n");
                files.insert(
                    format!(
                        "methods/{}/{}",
                        poolster_core::files::source_file_stem(if group.is_empty() {
                            "flat"
                        } else {
                            group
                        }),
                        poolster_core::files::source_file_stem(&member.1)
                    ),
                    source,
                );
            }
        }
        Ok(files)
    }
    fn render_methods(
        &self,
        source: &mut String,
        members: &[(String, String)],
        operations: &BTreeMap<String, GraphqlOperationSymbols>,
        transport: &str,
    ) -> Result<()> {
        for (operation, method) in members {
            let symbols = &operations[operation];
            if self.without_variables.contains(operation) {
                writeln!(
                    source,
                    "    pub async fn {method}(&self) -> std::result::Result<crate::graphql_runtime::GraphqlResponse<{}>, crate::graphql_runtime::GraphqlTransportError> {{ {}({transport}, &{} {{}}).await }}",
                    symbols.result, symbols.function, symbols.variables
                )?;
                continue;
            }
            writeln!(
                source,
                "    pub async fn {method}(&self, variables: &{}) -> std::result::Result<crate::graphql_runtime::GraphqlResponse<{}>, crate::graphql_runtime::GraphqlTransportError> {{ {}({transport}, variables).await }}",
                symbols.variables, symbols.result, symbols.function
            )?;
        }
        Ok(())
    }
}
