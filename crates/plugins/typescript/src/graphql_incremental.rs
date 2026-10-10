//! Opt-in incremental execution, deliberately distinct from unary GraphQL SDKs.
use crate::TypeScript;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::{GraphqlIncrementalDialect, GraphqlIncrementalOperations},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Clone, Debug)]
pub struct GraphqlIncrementalClient {
    pub definition: GraphqlIncrementalOperations,
}
impl Contract for GraphqlIncrementalClient {
    const NAME: &'static str = "poolster.typescript.graphql-incremental-client.v1";
}
pub struct GraphqlIncremental {
    meta: Meta,
    provider: Option<Handle<GraphqlIncrementalOperations>>,
    scalars: BTreeMap<String, crate::GraphqlScalarMapping>,
}
pub fn graphql_incremental(
    provider: Option<Handle<GraphqlIncrementalOperations>>,
) -> GraphqlIncremental {
    GraphqlIncremental {
        meta: Meta::new(),
        provider,
        scalars: BTreeMap::new(),
    }
}
impl GraphqlIncremental {
    pub fn scalar(
        mut self,
        name: impl Into<String>,
        input: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        self.scalars
            .insert(name.into(), crate::GraphqlScalarMapping::new(input, output));
        self
    }
    pub fn scalars(mut self, scalars: BTreeMap<String, crate::GraphqlScalarMapping>) -> Self {
        self.scalars = scalars;
        self
    }
    pub fn input(mut self, provider: Handle<GraphqlIncrementalOperations>) -> Self {
        self.provider = Some(provider);
        self
    }
    pub fn handle(&self) -> Handle<GraphqlIncrementalClient> {
        self.meta.handle()
    }
}
impl Plugin<TypeScript> for GraphqlIncremental {
    fn kind(&self) -> &'static str {
        "typescript-graphql-incremental"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlIncrementalClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlIncrementalOperations>()?;
        ensure!(
            contract.dialect == GraphqlIncrementalDialect::DeferSpec20220824,
            "unsupported incremental dialect"
        );
        crate::graphql::scalars::validate_mappings(&self.scalars, &contract.definition)?;
        let scalars = &self.scalars;
        let mut entry = String::from("export * from './graphql-incremental-runtime.js';\n");
        entry.push_str(
            "export type {ScalarCodec,ScalarCodecMap} from './graphql-scalar-runtime.js';\n",
        );
        let mut imports = String::new();
        let mut symbols = BTreeSet::new();
        for (index, (name, fields)) in contract.definition.input_objects.iter().enumerate() {
            valid(name)?;
            ensure!(
                symbols.insert(name.clone()),
                "incremental symbol collision {name}"
            );
            writeln!(
                imports,
                "import type {{ {name} }} from '../models/model_{index}.js';"
            )?;
            let mut source = String::new();
            for (other_index, other) in contract.definition.input_objects.keys().enumerate() {
                if other != name {
                    writeln!(
                        source,
                        "import type {{ {other} }} from './model_{other_index}.js';"
                    )?;
                }
            }
            writeln!(
                source,
                "export type {name} = {};",
                crate::graphql::render_fields(fields, scalars, true)
            )?;
            cx.files.emit(GeneratedFile::new(
                format!("graphql-incremental/models/model_{index}.ts"),
                source,
            )?)?;
            writeln!(
                entry,
                "export type {{ {name} }} from './graphql-incremental/models/model_{index}.js';"
            )?;
        }
        let mut operations: Vec<_> = contract.definition.operations.iter().collect();
        operations.sort_by_key(|op| &op.name);
        ensure!(
            !operations.is_empty(),
            "incremental contract has no operations"
        );
        for (index, op) in operations.iter().enumerate() {
            valid(&op.name)?;
            let function = format!("{}{}", op.name[..1].to_ascii_lowercase(), &op.name[1..]);
            for symbol in [
                format!("{}Variables", op.name),
                format!("{}Result", op.name),
                function.clone(),
            ] {
                ensure!(
                    symbols.insert(symbol.clone()),
                    "incremental symbol collision {symbol}"
                );
            }
            let variable_shape = serde_json::to_string(
                &poolster_core::native::graphql_scalar_fields(&op.variables),
            )?;
            let result_shape =
                serde_json::to_string(&poolster_core::native::graphql_scalar_shape(&op.result))?;
            let input_shapes: BTreeMap<_, _> = contract
                .definition
                .input_objects
                .iter()
                .map(|(name, fields)| (name, poolster_core::native::graphql_scalar_fields(fields)))
                .collect();
            let inputs = serde_json::to_string(&input_shapes)?;
            let source = format!(
                "import {{ executeIncremental, type IncrementalHttpOptions, type IncrementalSnapshot }} from '../../graphql-incremental-runtime.js';\n{imports}export type {}Variables = {};\nexport type {}Result = {};\nexport const document = {};\nexport function {function}(options: IncrementalHttpOptions, variables: {}Variables): AsyncGenerator<IncrementalSnapshot<{}Result>> {{ return executeIncremental(options, {}, document, variables,{variable_shape},{result_shape},{inputs}); }}\n",
                op.name,
                crate::graphql::render_fields(&op.variables, scalars, true),
                op.name,
                crate::graphql::render_type(&op.result, scalars, false),
                serde_json::to_string(&op.document)?,
                op.name,
                op.name,
                serde_json::to_string(&op.name)?
            );
            cx.files.emit(GeneratedFile::new(
                format!("graphql-incremental/operations/operation_{index}.ts"),
                source,
            )?)?;
            writeln!(
                entry,
                "export {{ {function}, type {}Variables, type {}Result }} from './graphql-incremental/operations/operation_{index}.js';",
                op.name, op.name
            )?;
        }
        cx.files
            .emit(GeneratedFile::new("graphql-incremental.ts", entry)?)?;
        cx.files.emit(GeneratedFile::new(
            "graphql-incremental-runtime.ts",
            include_str!("graphql_incremental/runtime.ts"),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "graphql-scalar-runtime.ts",
            include_str!("../templates/graphql_scalar_runtime.ts.tmpl"),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "graphql-runtime.ts",
            include_str!("../templates/graphql_runtime.ts.tmpl"),
        )?)?;
        cx.workspace.export("graphql-incremental")?;
        cx.workspace.package_file(GeneratedFile::new("package.json", serde_json::to_string_pretty(&serde_json::json!({"name":cx.settings.package_name.as_deref().unwrap_or("poolster-graphql-incremental-client"),"version":cx.common.package_version.as_deref().unwrap_or("0.0.0"),"type":"module","main":"./dist/index.js","types":"./dist/index.d.ts","scripts":{"build":"tsc -p tsconfig.json"},"devDependencies":{"typescript":"5.9.3"}}))?)?)?;
        cx.workspace.package_file(GeneratedFile::new("tsconfig.json", r#"{"compilerOptions":{"target":"ES2022","module":"NodeNext","moduleResolution":"NodeNext","strict":true,"declaration":true,"outDir":"dist","lib":["ES2022","DOM"]},"include":["**/*.ts"],"exclude":["dist"]}"#)?)?;
        cx.files.emit(GeneratedFile::new("README.md", "# Incremental GraphQL TypeScript\n\nExplicit opt-in multipart/mixed;deferSpec=20220824. Use `for await (const snapshot of readUser({endpoint}, variables))`. Each snapshot exposes recursively partial `data`, accumulated `errors`, original `patches`, and `complete`. Configure scalarCodecs on HTTP options; generation-time .scalar/.scalars provide input/output type mappings. Codecs traverse selected fields, nested inputs, lists and discriminated unions. Raw aggregate wire data is retained so callbacks run once per emitted snapshot. Snapshots are independent values. `complete` means delivery completed, not application success. Generated final Result models describe all selected fields, never intermediate data. Supports path-based defer/stream patches only; pending/id/subPath drafts, subscriptions, mutations, grouped facades and dynamic field selection are unsupported. HTTP JSON fallback is supported when the server delivers all selections at once. Errors in patches remain explicit; malformed or truncated streams throw. Supply AbortSignal to cancel. Stream patches must append contiguous items.\n")?)?;
        cx.publish(GraphqlIncrementalClient {
            definition: contract.clone(),
        })
    }
}
fn valid(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.len() <= 128
            && name
                .bytes()
                .enumerate()
                .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()),
        "unsupported incremental TypeScript identifier {name}"
    );
    Ok(())
}
#[cfg(test)]
mod tests;
