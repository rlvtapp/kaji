//! GraphQL package assembly from owned operation contracts.
use super::*;

pub(super) fn render_advanced(
    contract: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
    subscriptions: bool,
    incremental: bool,
) -> Result<(GeneratedTree, BTreeMap<String, String>)> {
    ensure!(
        package.split('/').count() == 2
            && package
                .split('/')
                .all(|part| part.as_bytes().first().is_some_and(u8::is_ascii_lowercase))
            && package
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/-_.".contains(c)),
        "invalid Composer package name"
    );
    ensure!(
        matches!(style, GraphqlStyle::Idiomatic) || groups.is_empty(),
        "custom GraphQL groups require idiomatic style"
    );
    let namespace = crate::namespace_for_package(package);
    for segment in namespace.split('\\') {
        ident(segment)?;
    }
    let mut models = Models {
        namespace: namespace.clone(),
        files: BTreeMap::new(),
        names: ["client", "presence", "graphqlresponse", "graphqlexception"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    for (name, fields) in &contract.input_objects {
        models.object(name, fields, true)?;
    }
    let mut symbols = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut methods = BTreeMap::new();
    let mut used = BTreeSet::new();
    for op in &contract.operations {
        ensure!(
            subscriptions || op.kind != GraphqlOperationKind::Subscription,
            "PHP GraphQL subscriptions require a separately supported transport"
        );
        let name = ident(&op.name)?;
        let method = {
            let mut v = name.clone();
            v[..1].make_ascii_lowercase();
            v
        };
        ensure!(
            used.insert(method.to_ascii_lowercase())
                && !matches!(
                    method.to_ascii_lowercase().as_str(),
                    "execute"
                        | "__construct"
                        | "query"
                        | "mutation"
                        | "subscribe"
                        | "incremental"
                        | "response"
                        | "stream"
                        | "decodevalue"
                        | "encodevalue"
                ),
            "GraphQL operation naming collision {method}"
        );
        models.object(&format!("{name}Variables"), &op.variables, true)?;
        let ModelKind::Object(fields) = &op.result.kind else {
            anyhow::bail!("GraphQL operation result must be an object")
        };
        models.object(&format!("{name}Result"), fields, false)?;
        let variables = if op.variables.iter().all(|v| v.optional) {
            format!("?{name}Variables $variables = null")
        } else {
            format!("{name}Variables $variables")
        };
        let mode = if op.kind == GraphqlOperationKind::Subscription {
            "subscribe"
        } else if incremental {
            "incremental"
        } else {
            "execute"
        };
        let annotation = if mode == "execute" {
            "GraphqlResponse"
        } else {
            "\\Generator"
        };
        let returns = match mode {
            "subscribe" => format!("\\Generator<int, GraphqlResponse<{name}Result>>"),
            "incremental" => "\\Generator<int, IncrementalFrame>".into(),
            _ => format!("GraphqlResponse<{name}Result>"),
        };
        let mut functions = String::new();
        writeln!(
            functions,
            "/** @return {returns} */\nfunction {method}(Client $client, {variables}): {annotation}\n{{\n    return $client->{mode}(\n        {},\n        {},\n        $variables ?? new {name}Variables(),\n        {name}Result::class,\n    );\n}}",
            quote(&name),
            quote(&op.document)
        )?;
        if matches!(style, GraphqlStyle::Flat) {
            methods.insert(name.clone(), format!("    /** @return {returns} */\n    public function {method}({variables}): {annotation}\n    {{\n        return {method}($this, $variables);\n    }}"));
        }
        files.insert(format!("Operations/{name}.php"), functions);
        symbols.insert(name, method);
    }
    if matches!(style, GraphqlStyle::Idiomatic) {
        let mut groups = groups.clone();
        if groups.is_empty() {
            for op in &contract.operations {
                groups
                    .entry(
                        match op.kind {
                            GraphqlOperationKind::Query => "query",
                            GraphqlOperationKind::Mutation => "mutation",
                            GraphqlOperationKind::Subscription => "subscription",
                        }
                        .into(),
                    )
                    .or_default()
                    .insert(symbols[&op.name].clone(), op.name.clone());
            }
        }
        let mut group_names = BTreeSet::new();
        for (group, entries) in groups {
            let group = ident(&group)?;
            ensure!(
                group_names.insert(group.to_ascii_lowercase())
                    && !matches!(
                        group.to_ascii_lowercase().as_str(),
                        "execute" | "__construct"
                    ),
                "GraphQL group naming collision"
            );
            let class = format!("{}Group", upper(&group));
            ensure!(
                models.names.insert(class.to_ascii_lowercase()),
                "GraphQL group type collision"
            );
            methods.insert(
                class.clone(),
                format!("    public function {group}(): {class}\n    {{\n        return new {class}($this);\n    }}"),
            );
            let mut group_methods = BTreeMap::new();
            let mut names = BTreeSet::new();
            for (method, operation) in entries {
                let method = ident(&method)?;
                ensure!(
                    names.insert(method.to_ascii_lowercase()) && method != "__construct",
                    "GraphQL grouped method collision"
                );
                let function = symbols
                    .get(&operation)
                    .ok_or_else(|| anyhow::anyhow!("unknown GraphQL operation {operation}"))?;
                let op = contract
                    .operations
                    .iter()
                    .find(|o| o.name == operation)
                    .unwrap();
                let vars = if op.variables.iter().all(|f| f.optional) {
                    format!("?{operation}Variables $variables = null")
                } else {
                    format!("{operation}Variables $variables")
                };
                let annotation = if op.kind == GraphqlOperationKind::Subscription || incremental {
                    "\\Generator"
                } else {
                    "GraphqlResponse"
                };
                let returns = if op.kind == GraphqlOperationKind::Subscription {
                    format!("\\Generator<int, GraphqlResponse<{operation}Result>>")
                } else if incremental {
                    "\\Generator<int, IncrementalFrame>".into()
                } else {
                    format!("GraphqlResponse<{operation}Result>")
                };
                group_methods.insert(upper(&method), format!("    /** @return {returns} */\n    public function {method}({vars}): {annotation}\n    {{\n        return {function}($this->client, $variables);\n    }}"));
            }
            let inherited = method_traits(&class, &group_methods, &mut files, &mut models.names)?;
            files.insert(format!("Groups/{class}.php"),format!("final readonly class {class}\n{{\n    public function __construct(private Client $client)\n    {{\n    }}\n    {inherited}\n}}"));
        }
    }
    let mut tree = GeneratedTree::default();
    let inherited = method_traits("Client", &methods, &mut files, &mut models.names)?;
    let runtime = include_str!("runtime/runtime.php.tmpl").replace("__NAMESPACE__", &namespace);
    let (runtime, client) = runtime.split_once("class Client\n{").unwrap();
    tree.insert(GeneratedFile::new(
        "src/Runtime.php",
        format!("{runtime}\nrequire_once __DIR__ . '/Streaming.php';\n"),
    )?)?;
    files.insert(
        "Client.php".into(),
        format!(
            "class Client\n{{{}",
            client.replace("__CLIENT_METHODS__", &inherited)
        ),
    );
    files.insert(
        "Streaming.php".into(),
        include_str!("runtime/streaming.php.tmpl").into(),
    );
    for (name, source) in models.files {
        files.insert(format!("Models/{name}.php"), source);
    }
    for (file, mut source) in files {
        if !source.ends_with('\n') {
            source.push('\n');
        }
        tree.insert(GeneratedFile::new(
            format!("src/{file}"),
            format!("<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace};\n\n{source}"),
        )?)?;
    }
    tree.insert(GeneratedFile::new("src/Graphql.php","<?php\ndeclare(strict_types=1);\nrequire_once __DIR__.'/Runtime.php';\nforeach(['Models','Operations','Methods','Groups'] as $dir) foreach(glob(__DIR__.'/'. $dir .'/*.php') ?: [] as $file) require_once $file;\nrequire_once __DIR__.'/Client.php';\n")?)?;
    tree.insert(GeneratedFile::new("composer.json",serde_json::to_string_pretty(&serde_json::json!({"name":package,"version":"0.0.0","require":{"php":">=8.2"},"autoload":{"files":["src/Graphql.php"]}}))?)?)?;
    tree.insert(GeneratedFile::new("README.md","# GraphQL client\n\nPHP 8.2+, Composer autoload. Query/mutation fixed-operation clients use selection-specific immutable models. Optional fields use Presence::missing() versus Presence::of(null). Responses preserve partial data and errors; requireData() rejects errors. Raw functions, flat Client methods or grouped accessors. HTTP streams by default, injectable callable transport for PSR-18/Symfony. Opt-in subscriptions use distinct-connection graphql-sse. Incremental inputs use multipart/mixed deferSpec=20220824 with raw partial snapshots and final typed envelopes. Per-client scalarCodecs encode/decode callbacks walk model shapes; null and absence are retained. No reconnect, multiplexing or alternate incremental dialect.\n")?)?;
    source_layout::diagnostics(&mut tree)?;
    Ok((tree, symbols))
}
