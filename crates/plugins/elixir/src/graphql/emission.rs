//! Selection-specific model, operation and facade emission.
use super::*;

#[cfg(test)]
pub(super) fn render(
    c: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    render_capabilities(c, package, style, custom, false, false, None)
}
pub(super) fn render_capabilities(
    c: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    incremental: bool,
    selections: Option<&BTreeMap<String, Vec<poolster_core::native::GraphqlIncrementalSelection>>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        !c.operations.is_empty(),
        "Elixir GraphQL requires operation documents"
    );
    let app = crate::elixir_identifier(package);
    let module = crate::pascal_case(package);
    ensure!(
        !app.is_empty() && !module.is_empty() && app.len() <= 128 && module.len() <= 80,
        "Invalid Elixir GraphQL package name"
    );
    fn validate_names(t: &poolster_core::native::ModelType, c: &GraphqlOperations) -> Result<()> {
        use poolster_core::native::ModelKind;
        match &t.kind {
            ModelKind::Named(n) => ensure!(
                c.input_objects.contains_key(n),
                "Unknown GraphQL input object {n}"
            ),
            ModelKind::List(inner) => validate_names(inner, c)?,
            ModelKind::Object(fields) => {
                for f in fields {
                    validate_names(&f.ty, c)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    for fields in c
        .input_objects
        .values()
        .chain(c.operations.iter().map(|o| &o.variables))
    {
        for f in fields {
            validate_names(&f.ty, c)?;
        }
    }
    let mut models = models::Models::new(&module);
    let mut methods = BTreeMap::new();
    let mut calls = String::new();
    let mut operation_files = Vec::new();
    let mut declarations = Vec::new();
    let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (name, fields) in &c.input_objects {
        models.input(name, fields)?;
    }
    for op in &c.operations {
        ensure!(
            op.kind != GraphqlOperationKind::Subscription || subscriptions,
            "Elixir GraphQL subscriptions need a separately supported transport"
        );
        let name = crate::pascal_case(&op.name);
        ensure!(!name.is_empty(), "Invalid Elixir GraphQL operation name");
        let method = crate::elixir_identifier(&op.name);
        ensure!(
            method.len() <= 240,
            "Elixir GraphQL method exceeds VM atom name limit"
        );
        ensure!(
            models::identifier(&method)
                && ![
                    "new",
                    "client",
                    "execute",
                    "require_data",
                    "__info__",
                    "module_info"
                ]
                .contains(&method.as_str()),
            "Invalid Elixir GraphQL operation method"
        );
        ensure!(
            !methods.values().any(|m| m == &method),
            "Elixir GraphQL operation method collision"
        );
        methods.insert(op.name.clone(), method.clone());
        ensure!(
            matches!(op.result.kind, poolster_core::native::ModelKind::Object(_)),
            "GraphQL result must be an object selection"
        );
        let vars = models.input(&format!("{name}Variables"), &op.variables)?;
        let result = models.ty(&op.result, &format!("{name}Result"))?;
        let optional = op.variables.iter().all(|v| v.optional);
        let default = if optional {
            format!(" \\\\ %{vars}{{}}")
        } else {
            String::new()
        };
        let start = calls.len();
        let execution = if incremental {
            "incremental"
        } else if op.kind == GraphqlOperationKind::Subscription {
            "subscribe"
        } else {
            "execute"
        };
        let descriptor = serde_json::to_string(
            &serde_json::json!({"variables":poolster_core::native::graphql_scalar_fields(&op.variables),"result":poolster_core::native::graphql_scalar_shape(&op.result),"selections":selections.and_then(|s|s.get(&op.name)),"inputs":c.input_objects.iter().map(|(n,f)|(n.clone(),poolster_core::native::graphql_scalar_fields(f))).collect::<BTreeMap<_,_>>() }),
        )?;
        let returns = if execution == "execute" {
            format!("{{:ok, {module}.Envelope.t({result})}} | {{:error, term()}}")
        } else {
            "Enumerable.t()".into()
        };
        writeln!(
            calls,
            "  @spec {method}({module}.Client.t(), {vars}.t()) :: {returns}\n  def {method}(client, variables{default}) do\n    try do\n      {module}.Runtime.{execution}(client, {}, {}, {vars}.to_wire(variables), &{module}.Models.{name}Result.from_wire/1, Jason.decode!({}))\n    rescue e in ArgumentError -> {{:error, {{:variables, Exception.message(e)}}}}\n    end\n  end",
            elixir_string(&op.document),
            elixir_string(&op.name),
            elixir_string(&descriptor)
        )?;
        operation_files.push((
            method.clone(),
            format!(
                "defmodule {module}.Operations.{name} do\n{}\nend\n",
                &calls[start..]
            ),
        ));
        declarations.push(format!("{}\n  def {method}(client, variables{default}), do: {module}.Operations.{name}.{method}(client, variables)",calls[start..].lines().next().unwrap()));
        if style == GraphqlStyle::Idiomatic {
            groups
                .entry(
                    match op.kind {
                        GraphqlOperationKind::Query => "Query",
                        GraphqlOperationKind::Mutation => "Mutation",
                        GraphqlOperationKind::Subscription => "Subscription",
                    }
                    .into(),
                )
                .or_default()
                .insert(method, op.name.clone());
        }
    }
    if !custom.is_empty() {
        ensure!(
            style == GraphqlStyle::Idiomatic,
            "Custom groups require idiomatic style"
        );
        groups = custom.clone();
    }
    let surface = if style == GraphqlStyle::Raw {
        format!("{module}.Operations")
    } else {
        module.clone()
    };
    let mut tree = GeneratedTree::default();
    layout::facade(
        &mut tree,
        &app,
        &surface,
        &declarations,
        &format!("lib/{app}/operations.ex"),
    )?;
    let mut group_names = std::collections::BTreeSet::new();
    for (group, members) in groups {
        let group_name = crate::pascal_case(&group);
        ensure!(
            !group_name.is_empty()
                && group_name.len() + module.len() <= 230
                && group_names.insert(group_name.to_ascii_lowercase())
                && ![
                    "Client",
                    "Models",
                    "Runtime",
                    "Envelope",
                    "Application",
                    "MixProject"
                ]
                .contains(&group_name.as_str()),
            "Invalid Elixir group"
        );
        let mut group_declarations = Vec::new();
        let mut member_names = std::collections::BTreeSet::new();
        for (method, op) in members {
            let method = crate::elixir_identifier(&method);
            ensure!(
                models::identifier(&method)
                    && method.len() <= 240
                    && member_names.insert(method.clone())
                    && !["__info__", "module_info"].contains(&method.as_str()),
                "Invalid Elixir group method"
            );
            let target = methods
                .get(&op)
                .ok_or_else(|| anyhow::anyhow!("Unknown GraphQL operation {op}"))?;
            let name = crate::pascal_case(&op);
            let optional = c
                .operations
                .iter()
                .find(|v| v.name == op)
                .unwrap()
                .variables
                .iter()
                .all(|v| v.optional);
            let default = if optional {
                format!(" \\\\ %{module}.Models.{name}Variables{{}}")
            } else {
                String::new()
            };
            group_declarations.push(format!("  def {method}(client, variables{default}), do: {surface}.{target}(client, variables)"));
        }
        layout::facade(
            &mut tree,
            &app,
            &format!("{module}.{group_name}"),
            &group_declarations,
            &format!("lib/{app}/groups/{}.ex", layout::stem(&group_name)),
        )?;
    }
    for (name, source) in layout::modules(&models.source)? {
        let name = name.rsplit('.').next().unwrap();
        tree.insert(GeneratedFile::new(
            format!("lib/{app}/models/{}.ex", layout::stem(name)),
            source,
        )?)?;
    }
    for (name, source) in
        layout::modules(&include_str!("runtime/runtime.ex.tmpl").replace("__POOLSTER__", &module))?
    {
        let name = name.rsplit('.').next().unwrap();
        tree.insert(GeneratedFile::new(
            format!("lib/{app}/{}.ex", layout::stem(name)),
            source,
        )?)?;
    }
    for template in [
        include_str!("runtime/codecs.ex.tmpl"),
        include_str!("runtime/streaming.ex.tmpl"),
    ] {
        for (name, source) in layout::modules(&template.replace("__POOLSTER__", &module))? {
            let name = name.rsplit('.').next().unwrap();
            tree.insert(GeneratedFile::new(
                format!("lib/{app}/{}.ex", layout::stem(name)),
                source,
            )?)?;
        }
    }
    for (name, source) in operation_files {
        tree.insert(GeneratedFile::new(
            format!("lib/{app}/operations/{}.ex", layout::stem(&name)),
            source,
        )?)?;
    }
    tree.insert(GeneratedFile::new("mix.exs",format!("defmodule {module}.MixProject do\n use Mix.Project\n def project, do: [app: :{app},version: \"0.0.0\",elixir: \"~> 1.15\",deps: [{{:finch, \"== 0.24.0\"}},{{:jason, \"== 1.4.5\"}}]]\n def application, do: [extra_applications: [:logger,:inets,:crypto],mod: {{{module}.Application,[]}}]\nend\n"))?)?;
    tree.insert(GeneratedFile::new("README.md",format!("# GraphQL Elixir client\n\nFixed operation clients in `{style:?}` style. Construct `{module}.Client.new(endpoint)`. Raw functions live in `{module}.Operations`, flat functions in `{module}`, grouped functions in Query/Mutation or your custom modules. Variables and selected results retain their typed struct names below `{module}.Models`, with one module per model file. Execution bodies and documents live in one file per operation; API and group facades use bounded export modules. Optional variables default to `:poolster_absent`; nil means explicit null.\n\nReturns `{{:ok, Envelope}}` even for GraphQL errors/partial results; inspect data_present, data, errors, extensions and status. `{module}.Runtime.require_data/1` returns error for GraphQL errors or absent/null data. HTTP/protocol/variable failures return `{{:error, reason}}`. Finch0.24.0 and Jason1.4.5 are pinned. Custom transport receives a Finch request. Subscriptions use opt-in distinct-connection GraphQL SSE; incremental packages negotiate multipart deferSpec=20220824. Runtime scalar_codecs maps scalar names to encode/decode callbacks, preserving null and omission; custom scalars retain term() types. Abstract selections require selected __typename.\n"))?)?;
    tree.insert(GeneratedFile::new(
        "graphql/schema.graphql",
        c.schema_source.clone(),
    )?)?;
    tree.insert(GeneratedFile::new(
        "graphql/operations.graphql",
        c.operation_source.clone(),
    )?)?;
    let oversized=tree.iter().filter(|(path,source)|path.extension().is_some_and(|ext|ext=="ex")&&source.len()>128*1024).map(|(path,source)|serde_json::json!({"path":path,"bytes":source.len(),"max_file_bytes":128*1024,"reason":"Atomic declaration exceeds the source budget; retained intact."})).collect::<Vec<_>>();
    if !oversized.is_empty() {
        tree.insert(GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    Ok((tree, methods))
}
