use anyhow::Result;
use poolster_core::native::GraphqlOperation;
pub(super) fn files(
    package: &str,
    command: &str,
    endpoint: Option<&str>,
    operations: &[GraphqlOperation],
) -> Result<Vec<(String, String)>> {
    let mut files = vec![
        (
            "Cargo.toml".into(),
            format!(
                "[workspace]\n[package]\nname={package:?}\nversion=\"0.1.0\"\nedition=\"2024\"\n[[bin]]\nname={command:?}\npath=\"src/main.rs\"\n[dependencies]\nanyhow=\"1\"\nclap={{version=\"4\",features=[\"std\",\"string\"]}}\nreqwest={{version=\"0.12\",default-features=false,features=[\"blocking\",\"json\",\"rustls-tls\"]}}\nserde_json=\"1\"\n"
            ),
        ),
        (
            "src/main.rs".into(),
            format!(
                "mod operations; mod runtime;\nfn main() {{ let code = match runtime::run({command:?}, {}) {{ Ok(code) => code, Err(error) => {{ eprintln!(\"{{error:#}}\"); 2 }} }}; std::process::exit(code); }}\n",
                endpoint
                    .map(|s| format!("Some({s:?})"))
                    .unwrap_or_else(|| "None".into())
            ),
        ),
        (
            "src/runtime.rs".into(),
            include_str!("runtime.rs.tmpl").into(),
        ),
        (
            "README.md".into(),
            format!(
                "# {command}\n\nFixed GraphQL operation CLI. Use `{command} --endpoint URL read-user --variables '{{\"id\":\"7\"}}'`. Operation names become kebab-case commands. `--variables-file FILE` accepts the same JSON object; absent variables default to `{{}}`, preserving omission separately from explicit `null`. Global flags may appear before or after the operation. Endpoint fallback: `GRAPHQL_ENDPOINT`; bearer token fallback: `GRAPHQL_TOKEN`. Repeated `--header 'Name: value'` configures request headers. Avoid command-line tokens in shell history; environment tokens are supported.\n\nStdout contains the complete GraphQL response envelope, including partial data and errors. Exit codes: 0 successful data, 3 partial data plus errors, 4 GraphQL errors without data, 2 malformed variables/response, HTTP/network failure or configuration error. Subscription/incremental transports are unsupported. Results are JSON rather than generated typed SDK models.\n"
            ),
        ),
    ];
    let mut modules = String::from(
        "pub struct Operation { pub command: &'static str, pub name: &'static str, pub document: &'static str, pub variables: &'static [&'static str] }\n",
    );
    for (chunk_index, chunk) in operations.chunks(50).enumerate() {
        modules.push_str(&format!("mod table_{chunk_index};\n"));
        let mut table = String::new();
        for (index, op) in chunk.iter().enumerate() {
            let file = format!(
                "operation-{}.rs",
                poolster_core::files::source_file_stem(&crate::kebab_case(&op.name))
            );
            table.push_str(&format!("#[path = {file:?}] mod op_{index};\n"));
            let document_file = file.trim_end_matches(".rs").to_owned() + ".graphql";
            files.push((
                format!("src/operations/{document_file}"),
                op.document.clone(),
            ));
            files.push((format!("src/operations/{file}"), format!("pub const OPERATION: super::super::Operation = super::super::Operation {{ command: {:?}, name: {:?}, document: include_str!({document_file:?}), variables: &{:?} }};\n", crate::kebab_case(&op.name), op.name, op.variables.iter().map(|v| &v.name).collect::<Vec<_>>())));
        }
        table.push_str("pub const OPERATIONS: &[super::Operation] = &[\n");
        for index in 0..chunk.len() {
            table.push_str(&format!("op_{index}::OPERATION,\n"));
        }
        table.push_str("];\n");
        files.push((format!("src/operations/table_{chunk_index}.rs"), table));
    }
    modules.push_str("pub fn operations() -> impl Iterator<Item = &'static Operation> {\n");
    modules.push_str("[\n");
    for index in 0..operations.len().div_ceil(50) {
        modules.push_str(&format!("table_{index}::OPERATIONS,\n"));
    }
    modules.push_str("].into_iter().flatten()\n}\n");
    files.push(("src/operations/mod.rs".into(), modules));
    Ok(files)
}
