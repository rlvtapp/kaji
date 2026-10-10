//! Selection-specific source and artifact emission.
use super::*;

pub(super) fn generate(generator: &Graphql, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
    let mut oversized = Vec::new();
    let incremental = if generator.incremental_mode {
        Some(cx.inputs.get::<GraphqlIncrementalOperations>()?)
    } else {
        None
    };
    let contract = if let Some(incremental) = incremental {
        &incremental.definition
    } else {
        cx.inputs.get::<GraphqlOperations>()?
    };
    ensure!(
        !contract.operations.is_empty(),
        "Rust GraphQL generation requires operations"
    );
    ensure!(
        contract
            .operations
            .iter()
            .all(|op| op.kind != GraphqlOperationKind::Subscription
                || (generator.subscriptions && !generator.incremental_mode)),
        "Rust GraphQL subscriptions require .subscriptions(); incremental subscriptions are unsupported"
    );
    ensure!(
        cx.workspace.graphql_package.is_none(),
        "Rust package already contains a GraphQL generator"
    );
    ensure!(
        !cx.workspace.models && !cx.workspace.operations && cx.workspace.http_api.is_none(),
        "HTTP and GraphQL generators require separate Rust packages"
    );
    let name = cx
        .settings
        .package_name
        .clone()
        .unwrap_or_else(|| "poolster-graphql-client".into());
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()),
        "invalid Rust GraphQL package name"
    );
    let version = cx
        .common
        .package_version
        .clone()
        .unwrap_or_else(|| "0.0.0".into());
    let layout = styles::layout(generator.style, contract, &generator.groups)?;
    let mappings = scalars::validate_mappings(&generator.scalars, contract)?;
    let mut models = models::Models::with_reserved(contract, &mappings, &layout.reserved());
    models.input_objects()?;
    let mut operation_files = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut wire_names = BTreeSet::new();
    let mut operations = BTreeMap::new();
    let mut ordered_operations: Vec<_> = contract.operations.iter().collect();
    ordered_operations.sort_by_key(|op| &op.name);
    for op in ordered_operations {
        ensure!(
            wire_names.insert(&op.name),
            "duplicate GraphQL operation {}",
            op.name
        );
        let variables = models.variables(
            &format!("{}Variables", crate::render::type_name(&op.name)),
            &op.variables,
        )?;
        let result = models.result(
            &format!("{}Result", crate::render::type_name(&op.name)),
            &op.result,
        )?;
        let base = crate::render::rust_field_name(&op.name);
        let mut function = base.clone();
        let mut suffix = 2;
        while !names.insert(function.clone()) {
            function = format!("{base}{suffix}");
            suffix += 1;
        }
        let shape = serde_json::json!({"input":graphql_scalar_fields(&op.variables),"result":graphql_scalar_shape(&op.result),"selections":incremental.and_then(|value|value.selections.get(&op.name))});
        let response = if generator.incremental_mode {
            format!("crate::graphql_incremental::GraphqlIncrementalStream<{result}>")
        } else if op.kind == GraphqlOperationKind::Subscription {
            format!("crate::graphql_sse::GraphqlSubscription<{result}>")
        } else {
            format!("crate::graphql_runtime::GraphqlResponse<{result}>")
        };
        let execute = if generator.incremental_mode {
            "incremental_with_shape"
        } else if op.kind == GraphqlOperationKind::Subscription {
            "subscribe_with_shape"
        } else {
            "execute_with_shape"
        };
        let mut operation_source = String::new();
        writeln!(
            operation_source,
            "pub async fn {function}(\n    transport: &crate::graphql_runtime::GraphqlHttpTransport,\n    variables: &{variables},\n) -> Result<{response}, crate::graphql_runtime::GraphqlTransportError> {{\n    transport.{execute}(\n        {},\n        {},\n        variables,\n        {},\n    ).await\n}}\n",
            serde_json::to_string(&op.document)?,
            serde_json::to_string(&op.name)?,
            serde_json::to_string(&serde_json::to_string(&shape)?)?
        )?;
        operation_files.insert(function.clone(), operation_source);
        operations.insert(
            op.name.clone(),
            GraphqlOperationSymbols {
                function,
                variables,
                result,
                kind: op.kind,
                incremental: generator.incremental_mode,
            },
        );
    }
    let client_files = layout.render_files(generator.style, &operations)?;
    let mut entry =
        String::from("// Generated by Poolster.\nuse serde::{Deserialize, Serialize};\n");
    for wrapper in ["Presence", "Optional"] {
        if models
            .files
            .values()
            .any(|source| source.contains(&format!("{wrapper}<")))
        {
            writeln!(entry, "use crate::graphql_runtime::{wrapper};")?;
        }
    }
    for (category, files) in [
        ("models", models.files),
        ("operations", operation_files),
        ("client", client_files),
    ] {
        let files: Vec<_> = files.into_iter().collect();
        for (index, chunk) in files.chunks(64).enumerate() {
            let mut index_source = String::from("// Generated by Poolster.\n");
            for (name, content) in chunk {
                let name = name
                    .split('/')
                    .map(poolster_core::files::source_file_stem)
                    .collect::<Vec<_>>()
                    .join("/");
                writeln!(index_source, "include!(\"../{name}.rs\");")?;
                emit_source(
                    &mut cx.files,
                    GeneratedFile::new(
                        format!("src/graphql/{category}/{name}.rs"),
                        content.clone(),
                    )?,
                    &mut oversized,
                )?;
            }
            writeln!(
                entry,
                "include!(\"graphql/{category}/_exports/part_{index}.rs\");"
            )?;
            emit_source(
                &mut cx.files,
                GeneratedFile::new(
                    format!("src/graphql/{category}/_exports/part_{index}.rs"),
                    index_source,
                )?,
                &mut oversized,
            )?;
        }
    }
    emit_source(
        &mut cx.files,
        GeneratedFile::new("src/graphql.rs", entry)?,
        &mut oversized,
    )?;
    emit_source(
        &mut cx.files,
        GeneratedFile::new(
            "src/graphql_runtime.rs",
            include_str!("runtime/runtime.rs.tmpl"),
        )?,
        &mut oversized,
    )?;
    for (path, source) in [
        (
            "src/graphql_codecs.rs",
            include_str!("runtime/codecs.rs.tmpl"),
        ),
        ("src/graphql_sse.rs", include_str!("runtime/sse.rs.tmpl")),
        (
            "src/graphql_incremental.rs",
            include_str!("runtime/incremental.rs.tmpl"),
        ),
    ] {
        emit_source(
            &mut cx.files,
            GeneratedFile::new(path, source)?,
            &mut oversized,
        )?;
    }
    let input_shapes = contract
        .input_objects
        .iter()
        .map(|(name, fields)| (name.clone(), graphql_scalar_fields(fields)))
        .collect::<BTreeMap<_, _>>();
    emit_source(
        &mut cx.files,
        GeneratedFile::new(
            "src/graphql_input_shapes.json",
            serde_json::to_string(&input_shapes)?,
        )?,
        &mut oversized,
    )?;
    emit_source(
        &mut cx.files,
        GeneratedFile::new(
            "README.md",
            "# Rust GraphQL client\n\nSelection-specific query/mutation functions use a caller-provided reqwest Client and endpoint. Results distinguish Success, Partial and Error; transport failures are separate. Presence::Absent, Null and Value preserve nullable input and conditional-result presence. Optional::Absent/Value preserves nonnullable optional fields. Unmapped custom scalars retain serde_json::Value. Configured input/output mappings define Rust model types; GraphqlScalarCodecs registers typed runtime encode/decode callbacks without changing null or omission semantics. Enabled subscriptions use distinct-connection graphql-sse next/complete streams; dropping a stream closes its connection, with no automatic reconnect/replay. Incremental packages negotiate multipart/mixed deferSpec=20220824; snapshots are recursively partial JSON and Complete validates the final selected model. Other incremental dialects are rejected. Abstract selections without __typename use structural untagged unions; structurally indistinguishable variants cannot identify a concrete GraphQL type. Native schema/operation documents remain in the input contract.\n",
        )?,
        &mut oversized,
    )?;
    cx.workspace.graphql_package = Some(NativePackage { name, version });
    if !oversized.is_empty() {
        cx.files.emit(GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    cx.publish(GraphqlClient {
        operations,
        style: generator.style,
        methods: layout.methods,
    })
}
