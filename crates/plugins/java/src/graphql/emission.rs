//! Selection-specific model, operation and facade emission.
use super::*;

#[cfg(test)]
pub(super) fn render(
    c: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    render_advanced(c, package, style, custom, false, false)
}
pub(super) fn render_advanced(
    c: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    incremental: bool,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    let mut canonical = c.clone();
    canonical
        .operations
        .sort_by(|left, right| left.name.cmp(&right.name));
    let c = &canonical;
    ensure!(
        !c.operations.is_empty(),
        "Java GraphQL requires operation documents"
    );
    ensure!(
        package.len() <= 512
            && package
                .split('.')
                .all(|segment| models::identifier(segment) && segment.len() <= 128),
        "Invalid Java package name"
    );
    fn validate_names(ty: &poolster_core::native::ModelType, c: &GraphqlOperations) -> Result<()> {
        use poolster_core::native::ModelKind;
        match &ty.kind {
            ModelKind::Named(name) => ensure!(
                c.input_objects.contains_key(name),
                "Unknown Java GraphQL input object {name}"
            ),
            ModelKind::List(inner) => validate_names(inner, c)?,
            ModelKind::Object(fields) => {
                for field in fields {
                    validate_names(&field.ty, c)?;
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
        for field in fields {
            validate_names(&field.ty, c)?;
        }
    }
    let mut models = models::Models::default();
    for (name, fields) in &c.input_objects {
        models.input(name, fields)?;
    }
    let mut methods = BTreeMap::new();
    let mut calls = Vec::new();
    let mut operation_files = Vec::new();
    let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for op in &c.operations {
        let mut operation_calls = String::new();
        ensure!(
            op.kind != GraphqlOperationKind::Subscription || subscriptions,
            "Java GraphQL subscriptions require a separately supported transport"
        );
        ensure!(
            models::identifier(&op.name),
            "Invalid Java operation identifier: {}",
            op.name
        );
        let method = crate::method_name(&op.name);
        ensure!(
            ![
                "execute",
                "query",
                "mutation",
                "transport",
                "getClass",
                "wait",
                "notify",
                "notifyAll",
                "toString",
                "hashCode",
                "equals"
            ]
            .contains(&method.as_str()),
            "Reserved Java operation name"
        );
        ensure!(
            !methods.values().any(|v| v == &method),
            "Java GraphQL operation name collision"
        );
        methods.insert(op.name.clone(), method.clone());
        models.input(&format!("{}Variables", op.name), &op.variables)?;
        ensure!(
            matches!(op.result.kind, poolster_core::native::ModelKind::Object(_)),
            "GraphQL operation result must be an object selection"
        );
        let result = models.ty(&op.result, &format!("{}Result", op.name))?;
        let vars = format!("{}Variables", op.name);
        let chunks = op
            .document
            .chars()
            .collect::<Vec<_>>()
            .chunks(4000)
            .map(|chars| serde_json::to_string(&chars.iter().collect::<String>()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let document = if chunks.len() <= 1 {
            chunks.first().cloned().unwrap_or("\"\"".into())
        } else {
            format!("String.join(\"\",{})", chunks.join(","))
        };
        let name = serde_json::to_string(&op.name)?;
        let return_type = if incremental {
            format!("java.util.stream.Stream<IncrementalSnapshot<{result}>>")
        } else if op.kind == GraphqlOperationKind::Subscription {
            format!("java.util.stream.Stream<Envelope<{result}>>")
        } else {
            format!("Envelope<{result}>")
        };
        let execution = if incremental {
            "incremental"
        } else if op.kind == GraphqlOperationKind::Subscription {
            "subscribeWithScalars"
        } else {
            "executeWithScalars"
        };
        let variable_shape = serde_json::to_string(&serde_json::to_string(
            &poolster_core::native::graphql_scalar_fields(&op.variables),
        )?)?;
        let result_shape = serde_json::to_string(&serde_json::to_string(
            &poolster_core::native::graphql_scalar_shape(&op.result),
        )?)?;
        let input_shapes: BTreeMap<_, _> = c
            .input_objects
            .iter()
            .map(|(name, fields)| (name, poolster_core::native::graphql_scalar_fields(fields)))
            .collect();
        let inputs = serde_json::to_string(&serde_json::to_string(&input_shapes)?)?;
        let (receiver, param) = if style == GraphqlStyle::Raw {
            ("client.transport", "Client client, ")
        } else {
            ("transport", "")
        };
        writeln!(
            operation_calls,
            "public {} {return_type} {method}({param}{vars} variables) throws java.io.IOException, InterruptedException {{\n  return {}Operation.execute({receiver}, variables);\n\n}}",
            if style == GraphqlStyle::Raw {
                "static"
            } else {
                ""
            },
            op.name
        )?;
        operation_files.push((format!("{}Operation",op.name),format!("package {package}.operations;\n\nimport {package}.models.*;\n\nimport static {package}.GraphqlRuntime.*;\n\npublic final class {}Operation {{\n  private {}Operation() {{}} public static {return_type} execute(Transport transport,{vars} variables) throws java.io.IOException,InterruptedException {{\n    return transport.{execution}({document},{name},variables.toJson(),{result}::fromJson,shape({variable_shape}),shape({result_shape}),shape({inputs}));\n\n  }}\n\n}}\n\n",op.name,op.name)));
        if op.variables.iter().all(|v| v.optional) {
            writeln!(
                operation_calls,
                "public {} {return_type} {method}({}) throws java.io.IOException, InterruptedException {{\n  return {method}({}new {vars}({}));\n\n}}",
                if style == GraphqlStyle::Raw {
                    "static"
                } else {
                    ""
                },
                if style == GraphqlStyle::Raw {
                    "Client client"
                } else {
                    ""
                },
                if style == GraphqlStyle::Raw {
                    "client, "
                } else {
                    ""
                },
                op.variables
                    .iter()
                    .map(|_| "Field.absent()")
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        calls.push(operation_calls);
        if style == GraphqlStyle::Idiomatic {
            groups
                .entry(
                    match op.kind {
                        GraphqlOperationKind::Query => "query",
                        GraphqlOperationKind::Subscription => "subscription",
                        _ => "mutation",
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
    let mut group_code = Vec::new();
    let mut group_files = Vec::new();
    for (group, members) in groups {
        ensure!(
            models::identifier(&group)
                && !methods.values().any(|m| m == &group)
                && ![
                    "transport",
                    "execute",
                    "getClass",
                    "wait",
                    "notify",
                    "notifyAll",
                    "toString",
                    "hashCode",
                    "equals"
                ]
                .contains(&group.as_str()),
            "Invalid or colliding Java group: {group}"
        );
        let class = format!("{}Group", crate::type_name(&group));
        models.reserve(&class)?;
        let mut accessor = String::new();
        writeln!(
            accessor,
            "public {package}.groups.{class} {group}() {{\n  return new {package}.groups.{class}(this);\n\n}}"
        )?;
        group_code.push(accessor);
        let mut declarations = Vec::new();
        let mut group_source = format!(
            "package {package}.groups;\n\nimport {package}.Client;\n\nimport {package}.models.*;\n\nimport static {package}.GraphqlRuntime.*;\n\npublic final class {class} {{\n  private final Client client;\n  public {class}(Client client) {{\n    this.client=client;\n\n  }}\n  \n"
        );
        for (method, op) in members {
            let mut declaration = String::new();
            ensure!(
                models::identifier(&method)
                    && ![
                        "getClass",
                        "wait",
                        "notify",
                        "notifyAll",
                        "toString",
                        "hashCode",
                        "equals"
                    ]
                    .contains(&method.as_str()),
                "Invalid Java grouped method"
            );
            let target = methods
                .get(&op)
                .ok_or_else(|| anyhow::anyhow!("Unknown GraphQL operation {op}"))?;
            let return_type = if incremental {
                format!("java.util.stream.Stream<IncrementalSnapshot<{op}Result>>")
            } else if c
                .operations
                .iter()
                .any(|item| item.name == op && item.kind == GraphqlOperationKind::Subscription)
            {
                format!("java.util.stream.Stream<Envelope<{op}Result>>")
            } else {
                format!("Envelope<{op}Result>")
            };
            writeln!(
                declaration,
                "public {return_type} {method}({op}Variables variables) throws java.io.IOException, InterruptedException {{\n  return client.{target}(variables);\n\n}}"
            )?;
            if c.operations
                .iter()
                .find(|v| v.name == op)
                .unwrap()
                .variables
                .iter()
                .all(|v| v.optional)
            {
                writeln!(
                    declaration,
                    "public {return_type} {method}() throws java.io.IOException, InterruptedException {{\n  return client.{target}();\n\n}}"
                )?;
            }
            declarations.push(declaration);
        }
        group_source.push_str(&declarations.join("\n"));
        group_source.push_str("}\n");
        group_files.push((class, group_source, declarations));
    }
    let root = format!("src/main/java/{}", package.replace('.', "/"));
    let mut tree = GeneratedTree::default();
    for file in facades::client(package, &calls, &group_code)? {
        tree.insert(file)?;
    }
    let runtime = (include_str!("runtime/runtime.java.tmpl").to_owned()
        + include_str!("runtime/scalars.java.tmpl")
        + include_str!("runtime/incremental.java.tmpl"))
    .replace("private static ", "public static ")
    .replace(
        "public static final ObjectMapper JSON",
        "private static final ObjectMapper JSON",
    );
    tree.insert(GeneratedFile::new(format!("{root}/GraphqlRuntime.java"),format!("package {package};\n\nimport com.fasterxml.jackson.databind.*;\n\nimport com.fasterxml.jackson.databind.node.*;\n\nimport java.util.*;\n\npublic class GraphqlRuntime {{\n  public final Transport transport;\n  protected GraphqlRuntime(Transport transport){{\n    this.transport=Objects.requireNonNull(transport);\n\n  }}\n  public static ObjectNode object(){{\n    return JSON.createObjectNode();\n\n  }}\n  \n{runtime}\n}}\n\n"))?)?;
    for (name, source) in layout::declarations(&models.source)? {
        let source = source.replace("JSON.createObjectNode()", "object()");
        tree.insert(GeneratedFile::new(format!("{root}/models/{name}.java"),format!("package {package}.models;\nimport com.fasterxml.jackson.databind.*;\nimport com.fasterxml.jackson.databind.node.*;\nimport static {package}.GraphqlRuntime.*;\npublic {}\n",source.strip_prefix("public ").unwrap()))?)?;
    }
    for (name, source) in operation_files {
        tree.insert(GeneratedFile::new(
            format!("{root}/operations/{name}.java"),
            source,
        )?)?;
    }
    for (name, source, declarations) in group_files {
        for file in facades::group(package, &name, &source, &declarations)? {
            tree.insert(file)?;
        }
    }
    tree.insert(GeneratedFile::new(
        "pom.xml",
        crate::docs::pom_xml(package, "graphql-client", "0.0.0"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "build.gradle",
        crate::docs::build_gradle(package, "graphql-client", "0.0.0"),
    )?)?;
    tree.insert(GeneratedFile::new(
        "settings.gradle",
        crate::docs::settings_gradle("graphql-client"),
    )?)?;
    tree.insert(GeneratedFile::new("README.md",format!("# GraphQL Java client\n\nJava 17+; Maven or Gradle; pinned Jackson 2.18.3. Import `{package}.Client`.\n\nSelected style: `{style:?}`. Raw exposes static operation functions taking a client, flat exposes methods on the client, and idiomatic exposes query/mutation or explicitly configured resource groups.\n\nVariables and selected results are individual records in `{package}.models`; import `{package}.models.ReadUserVariables` instead of the previous `Client.ReadUserVariables`. Client call paths and inherited Field/Envelope/Transport helper types remain compatible. Each operation document and execution body lives in a separate operations file; Client is a thin facade. Facade methods are split into inherited declaration parts. Atomic model declarations exceeding128KiB are reported in .poolster/source-layout-diagnostics.json rather than silently split across incompatible APIs. Optional fields use `Field.absent()` or `Field.of(value)`; explicit null is distinct from absence. HTTP uses JDK HttpClient. Supply `HttpTransport(endpoint, http, headers)` or implement `Transport`.\n\nCalls return `Envelope<T>` with data presence, typed partial data, errors, extensions and HTTP status. `requireData()` rejects GraphQL errors and missing/null data. Transport/protocol failures throw IOException; interruption remains explicit. Abstract selections require selected __typename. Custom scalars remain JsonNode. Subscriptions are opt-in via .subscriptions(true), using distinct POST text/event-stream connections. Calls return a closeable Java Stream of typed envelopes; use try-with-resources.\n"))?)?;
    tree.insert(GeneratedFile::new(
        "graphql/schema.graphql",
        c.schema_source.clone(),
    )?)?;
    tree.insert(GeneratedFile::new(
        "graphql/operations.graphql",
        c.operation_source.clone(),
    )?)?;
    let oversized=tree.iter().filter(|(path,source)|path.extension().is_some_and(|ext|ext=="java")&&source.len()>128*1024).map(|(path,source)|serde_json::json!({"path":path,"bytes":source.len(),"max_file_bytes":128*1024,"reason":"Atomic declaration exceeds the source budget; retained intact to preserve public API."})).collect::<Vec<_>>();
    if !oversized.is_empty() {
        tree.insert(GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    Ok((tree, methods))
}
