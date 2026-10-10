//! Runtime codec descriptors stay separate from the protocol contract's model semantics.
use super::*;
use serde_json::Value;
fn shape(ty: &ModelType) -> Value {
    poolster_core::native::graphql_scalar_shape(ty)
}
pub(super) fn fields_shape(fields: &[ModelField]) -> Value {
    poolster_core::native::graphql_scalar_fields(fields)
}
pub(super) fn operation_shapes(op: &poolster_core::native::GraphqlOperation) -> Result<String> {
    Ok(format!(
        "const variablesShape: ScalarShape = JSON.parse({});\nconst resultShape: ScalarShape = JSON.parse({});\n",
        serde_json::to_string(&serde_json::to_string(&fields_shape(&op.variables))?)?,
        serde_json::to_string(&serde_json::to_string(&shape(&op.result))?)?
    ))
}
pub(super) fn files(
    contract: &GraphqlOperations,
    mappings: &BTreeMap<String, GraphqlScalarMapping>,
) -> Result<Vec<GeneratedFile>> {
    let mut files = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    fn collect(ty: &ModelType, used: &mut std::collections::BTreeSet<String>) {
        match &ty.kind {
            ModelKind::Scalar(name)
                if !matches!(name.as_str(), "Int" | "Float" | "String" | "ID" | "Boolean") =>
            {
                used.insert(name.clone());
            }
            ModelKind::Object(fields) => {
                for field in fields {
                    collect(&field.ty, used)
                }
            }
            ModelKind::List(item) => collect(item, used),
            ModelKind::Union(items) => {
                for item in items {
                    collect(item, used)
                }
            }
            _ => {}
        }
    }
    for fields in contract.input_objects.values() {
        for field in fields {
            collect(&field.ty, &mut used);
        }
    }
    for op in &contract.operations {
        collect(&op.result, &mut used);
        for field in &op.variables {
            collect(&field.ty, &mut used);
        }
    }
    let mut source = String::from(
        "/** Direction-specific callbacks are supplied by the SDK consumer. */\nimport type { ScalarInputShapes } from './graphql-scalar-runtime.js';\nexport type GraphqlScalarCodecs = {\n",
    );
    for name in &used {
        let (input, output) = mappings
            .get(name)
            .map(|m| (m.input.as_str(), m.output.as_str()))
            .unwrap_or(("unknown", "unknown"));
        writeln!(
            source,
            "  {}?: {{ encode?: (value: {input}) => unknown; decode?: (value: unknown) => {output} }};",
            serde_json::to_string(name)?
        )?;
    }
    source.push_str("}\n");
    let inputs = contract.input_objects.iter().collect::<Vec<_>>();
    for (chunk, entries) in inputs.chunks(50).enumerate() {
        let mut module = String::from(
            "import type { ScalarInputShapes } from '../../graphql-scalar-runtime.js';\n",
        );
        let mut fields = Vec::new();
        for (i, (name, fields_value)) in entries.iter().enumerate() {
            let file = poolster_core::files::source_file_stem(name);
            files.push(GeneratedFile::new(format!("graphql/codec-inputs/{file}.ts"),format!("import type {{ ScalarShape }} from '../../graphql-scalar-runtime.js';\nexport default JSON.parse({}) as ScalarShape;\n",serde_json::to_string(&serde_json::to_string(&fields_shape(fields_value))?)?))?);
            writeln!(module, "import input{i} from './{file}.js';")?;
            fields.push(format!("[{}]: input{i}", serde_json::to_string(name)?));
        }
        writeln!(
            module,
            "export default {{ {} }} satisfies ScalarInputShapes;",
            fields.join(", ")
        )?;
        files.push(GeneratedFile::new(
            format!("graphql/codec-inputs/part_{chunk}.ts"),
            module,
        )?);
        writeln!(
            source,
            "import part{chunk} from './graphql/codec-inputs/part_{chunk}.js';"
        )?;
    }
    writeln!(
        source,
        "export const scalarInputShapes: ScalarInputShapes = {{ {} }};",
        (0..inputs.len().div_ceil(50))
            .map(|i| format!("...part{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    files.push(GeneratedFile::new("graphql-codecs.ts", source)?);
    files.push(GeneratedFile::new(
        "graphql-scalar-runtime.ts",
        include_str!("../../templates/graphql_scalar_runtime.ts.tmpl"),
    )?);
    files.push(GeneratedFile::new(
        "graphql-sse.ts",
        include_str!("../../templates/graphql_sse.ts.tmpl"),
    )?);
    Ok(files)
}
