use super::*;

#[derive(Serialize)]
struct ShownOperation {
    method: String,
    path: String,
    operation_id: String,
}

#[derive(Default)]
struct ShowTree {
    operations: Vec<String>,
    children: BTreeMap<String, ShowTree>,
}

pub(super) fn show(options: Show) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let mut compiler = Command::new(&helper);
    compiler.arg("--out").arg(temporary.path()).arg(&source);
    // JSON is an API for agent callers; compiler progress must not corrupt it.
    if options.format == ShowFormat::Json {
        compiler.stdout(Stdio::null());
    }
    let status = compiler
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = poolster_input_openapi::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    let api = slice_api_paths(api, &options.paths)?;
    match options.format {
        ShowFormat::Json => {
            let operations = api
                .operations
                .iter()
                .map(|operation| ShownOperation {
                    method: operation.method.as_str().into(),
                    path: operation.path.clone(),
                    operation_id: operation.id.clone(),
                })
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&operations)?);
        }
        ShowFormat::Human => print_show_tree(&api),
    }
    Ok(())
}

fn print_show_tree(api: &Api) {
    let mut root = ShowTree::default();
    for operation in &api.operations {
        let mut node = &mut root;
        for segment in operation
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
        {
            node = node.children.entry(segment.into()).or_default();
        }
        node.operations
            .push(format!("{} {}", operation.method.as_str(), operation.id));
    }
    println!("/");
    print_show_children(&root, "");
}

fn print_show_children(node: &ShowTree, prefix: &str) {
    let entries = node.children.iter().collect::<Vec<_>>();
    for (index, (segment, child)) in entries.iter().enumerate() {
        let last = index + 1 == entries.len();
        let branch = if last { "└─" } else { "├─" };
        let operations = if child.operations.is_empty() {
            String::new()
        } else {
            format!(" [{}]", child.operations.join(", "))
        };
        println!("{prefix}{branch}{segment}{operations}");
        let next_prefix = format!("{prefix}{}", if last { "  " } else { "│ " });
        print_show_children(child, &next_prefix);
    }
}
