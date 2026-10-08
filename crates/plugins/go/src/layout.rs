//! Package-local files, keeping cross-references in the same Go package.
use super::*;
use std::collections::BTreeSet;

pub(super) fn generate(
    tree: &mut GeneratedTree,
    api: &Api,
    schemas: &[Schema],
    directory: &str,
    package: &str,
    style: SdkClientStyle,
    jobs: usize,
) -> Result<()> {
    let runtime =
        render_runtime(api, package, style).replace(&response_validation::render(api), "");
    let (_, runtime) = runtime.split_once("\n)\n\n").expect("runtime import block");
    emit(tree, directory, "client.go", package, runtime)?;
    for (path, source) in response_validation::split_files(api) {
        emit(tree, directory, &path, package, &source)?;
    }
    tree.append(parallel_files(schemas, jobs, |schema, tree| {
        let mut body = String::new();
        render_schema(&mut body, schema);
        emit(
            tree,
            directory,
            &filename("model", &schema.name),
            package,
            &body,
        )
    })?)?;
    tree.append(parallel_files(&api.operations, jobs, |operation, tree| {
        let mut body = String::new();
        render_operation(&mut body, api, operation);
        render_cursor_pager(&mut body, api, operation);
        page_pagination::render(&mut body, api, operation);
        if cursor_pagination(api, operation).is_none() {
            render_offset_pager(&mut body, api, operation);
        }
        render_url_pager(&mut body, api, operation);
        body = body.replace("client.do(request,", "client.doWithRetry(request,");
        emit(
            tree,
            directory,
            &filename("operation", &operation.id),
            package,
            &body,
        )
    })?)?;
    if style == SdkClientStyle::Namespaced {
        let resources = resource_operations(api);
        let facades = resource_facade_names(&resources);
        for (resource, operations) in &resources {
            let facade = &facades[resource];
            let mut used = BTreeSet::new();
            for (part, operations) in operations.chunks(50).enumerate() {
                let mut body = String::new();
                if part == 0 {
                    let _ = writeln!(
                        body,
                        "// {facade}Service groups {resource} operations.\ntype {facade}Service struct {{ client *Client }}\n"
                    );
                }
                for operation in operations {
                    let direct = go_type_name(&operation.id);
                    let preferred = resource_method_name(&direct, resource);
                    let mut method = preferred;
                    if !used.insert(method.clone()) {
                        method = direct.clone();
                        if !used.insert(method.clone()) {
                            bail!("duplicate Go service method {facade}.{method}");
                        }
                    }
                    render_namespaced_operation(
                        &mut body, api, resource, facade, &method, &direct, operation,
                    );
                }
                emit(
                    tree,
                    directory,
                    &filename("service", &format!("{resource}_{part}")),
                    package,
                    &body,
                )?;
            }
        }
    }
    Ok(())
}

fn parallel_files<T: Sync>(
    items: &[T],
    jobs: usize,
    render: impl Fn(&T, &mut GeneratedTree) -> Result<()> + Sync,
) -> Result<GeneratedTree> {
    if items.is_empty() {
        return Ok(GeneratedTree::default());
    }
    let workers = if jobs == 0 {
        std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(8)
    } else {
        jobs.min(64)
    }
    .min(items.len());
    let render_chunk = |chunk: &[T]| {
        let mut tree = GeneratedTree::default();
        for item in chunk {
            render(item, &mut tree)?;
        }
        Ok::<_, anyhow::Error>(tree)
    };
    if workers == 1 || items.len() < 32 {
        return render_chunk(items);
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(items.len().div_ceil(workers))
            .map(|chunk| scope.spawn(|| render_chunk(chunk)))
            .collect();
        let mut output = GeneratedTree::default();
        // Join in input order: file collision errors and output stay deterministic.
        for handle in handles {
            output.append(
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Go rendering worker panicked"))??,
            )?;
        }
        Ok(output)
    })
}

fn filename(kind: &str, name: &str) -> String {
    let stem = identifier_words(name).join("_").to_ascii_lowercase();
    // Keep full identifiers in source, but bound filenames for common filesystems.
    // Hash the original name so punctuation/case variants cannot overwrite files.
    let hash = name.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{kind}_{}_{hash:016x}.go", &stem[..stem.len().min(100)])
}

pub(super) fn emit(
    tree: &mut GeneratedTree,
    directory: &str,
    name: &str,
    package: &str,
    body: &str,
) -> Result<()> {
    let selectors = selectors(body);
    let mut file = format!("{NOTICE}\npackage {package}\n\n");
    let imports: Vec<_> = [
        ("bytes", "bytes"),
        ("context", "context"),
        ("rand", "crypto/rand"),
        ("json", "encoding/json"),
        ("errors", "errors"),
        ("fmt", "fmt"),
        ("io", "io"),
        ("mime", "mime"),
        ("sort", "sort"),
        ("multipart", "mime/multipart"),
        ("textproto", "net/textproto"),
        ("http", "net/http"),
        ("url", "net/url"),
        ("reflect", "reflect"),
        ("strconv", "strconv"),
        ("strings", "strings"),
        ("time", "time"),
    ]
    .into_iter()
    .filter(|(alias, _)| selectors.contains(*alias))
    .collect();
    if !imports.is_empty() {
        file.push_str("import (\n");
        for (_, path) in imports {
            let _ = writeln!(file, "\t\"{path}\"");
        }
        file.push_str(")\n\n");
    }
    file.push_str(body);
    tree.insert(GeneratedFile::new(output_path(directory, name), file)?)
}

// Inspect code tokens, not strings/comments (a path containing `json.foo`
// must not create an unused encoding/json import). All generated imports are
// from the fixed standard-library allowlist above.
fn selectors(code: &str) -> BTreeSet<&str> {
    let bytes = code.as_bytes();
    let mut result = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            b'"' | b'\'' | b'`' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == quote {
                        index += 1;
                        break;
                    }
                    if bytes[index] == b'\\' && quote != b'`' {
                        index += 1;
                    }
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let end = index;
                while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                    index += 1;
                }
                if bytes.get(index) == Some(&b'.') {
                    result.insert(&code[start..end]);
                }
            }
            _ => index += 1,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::{HttpMethod, OperationMediaType, OperationResponse};
    use std::process::Command;

    #[test]
    fn import_detection_ignores_documentation_literals_and_struct_tags() {
        assert_eq!(
            selectors(
                "// fmt.String\n/* json.RawMessage */\n`json:\"io.ReadCloser\"`\n\"strings.Replace\"\nhttp.Header{}\ncontext . Context"
            ),
            BTreeSet::from(["http", "context"])
        );
    }

    #[test]
    fn split_response_registry_retains_shapes_across_chunk_boundaries() {
        let api = Api {
            schemas: (0..201)
                .map(|index| {
                    Schema::new(
                        format!("Model{index}"),
                        SchemaValue::new(SchemaKind::String),
                    )
                })
                .collect(),
            ..Api::default()
        };
        let root = tempfile::tempdir().unwrap();
        render_sdk(&api, "sdk", Some("probe"), SdkClientStyle::Flat, 0)
            .unwrap()
            .write_to(root.path())
            .unwrap();
        std::fs::write(root.path().join("sdk/registry_test.go"), r#"package probe
import ("reflect"; "testing")
func TestAllDescriptorChunks(t *testing.T) {
 if len(poolsterResponseShapes) != 201 { t.Fatalf("missing descriptors: %d",len(poolsterResponseShapes)) }
 for _, name := range []string{"Model0","Model100","Model200"} { if poolsterResponseShapes[name].Kind != "string" { t.Fatalf("missing %s",name) } }
 if err := poolsterValidateResponse("ok",reflect.TypeOf(Model200("")),"$",0); err != nil {t.Fatal(err)}
 if err := poolsterValidateResponse(true,reflect.TypeOf(Model200("")),"$",0); err == nil {t.Fatal("validation lost across chunk boundary")}
}
"#).unwrap();
        assert!(
            Command::new("go")
                .args(["test", "./..."])
                .env("GOCACHE", "/private/tmp/poolster-go-cache")
                .current_dir(root.path().join("sdk"))
                .status()
                .unwrap()
                .success()
        );
    }

    #[test]
    fn output_names_are_bounded_and_do_not_collide_after_normalization() {
        assert_ne!(filename("model", "foo-bar"), filename("model", "foo_bar"));
        let name = "LongGraphResource".repeat(100);
        assert!(filename("operation", &name).len() < 160);
    }

    #[test]
    fn split_sdk_compiles_with_recursive_models_and_all_response_kinds() {
        let mut api = Api {
            name: "Graph sample".into(),
            version: "1.0.0".into(),
            ..Api::default()
        };
        api.schemas.push(Schema::new(
            "Node",
            SchemaValue::new(SchemaKind::Object {
                fields: vec![poolster_core::Field {
                    name: "parent".into(),
                    value: SchemaValue::reference("#/components/schemas/Node"),
                    required: false,
                    annotations: BTreeMap::new(),
                }],
                additional_properties: AdditionalProperties::Forbidden,
            }),
        ));
        let mut enum_schema = SchemaValue::new(SchemaKind::String);
        enum_schema.enum_values = vec![
            serde_json::json!("-INF"),
            serde_json::json!("INF"),
            serde_json::json!("INF2"),
        ];
        api.schemas.push(Schema::new("NumberLiteral", enum_schema));
        for (id, media, schema) in [
            (
                "getNode",
                "application/json",
                SchemaValue::reference("#/components/schemas/Node"),
            ),
            (
                "getText",
                "text/plain",
                SchemaValue::new(SchemaKind::String),
            ),
            (
                "getBinary",
                "application/octet-stream",
                SchemaValue::new(SchemaKind::String),
            ),
            (
                "getEvents",
                "text/event-stream",
                SchemaValue::new(SchemaKind::String),
            ),
        ] {
            api.operations.push(Operation {
                id: id.into(),
                method: HttpMethod::Get,
                path: format!("/nodes/{id}"),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![OperationMediaType {
                        content_type: media.into(),
                        schema: Some(schema),
                    }],
                }],
                ..Operation::default()
            });
        }
        // Force several bounded service files, even when all operations share one tag.
        for index in 0..120 {
            api.operations.push(Operation {
                id: format!("listNodes{index}"),
                method: HttpMethod::Get,
                path: format!("/nodes/{index}/json.literal"),
                ..Operation::default()
            });
        }
        api.operations[0].parameters = vec![
            OperationParameter {
                name: "user-id".into(),
                location: "path".into(),
                required: true,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: BTreeMap::new(),
            },
            OperationParameter {
                name: "userId".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::String)),
                description: None,
                annotations: BTreeMap::new(),
            },
        ];
        api.operations[0].path = "/nodes/{user-id}".into();
        let tree = render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 0).unwrap();
        let serial = render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 1).unwrap();
        let parallel =
            render_sdk(&api, "sdk", Some("graph"), SdkClientStyle::Namespaced, 4).unwrap();
        assert_eq!(serial, parallel);
        assert_eq!(tree, parallel);
        let operation = tree
            .get(format!("sdk/{}", filename("operation", "getNode")))
            .unwrap();
        assert!(operation.contains("PathUserID string"));
        assert!(operation.contains("QueryUserID *string"));
        assert!(operation.contains("fmt.Sprint(input.PathUserID)"));
        assert!(operation.contains("addQuery(query, \"userId\", input.QueryUserID)"));
        assert!(tree.get("sdk/models.go").is_none());
        assert!(
            !tree
                .get("sdk/client.go")
                .unwrap()
                .contains("func (client *Client) GetNode(")
        );
        assert_eq!(
            tree.iter()
                .filter(|(p, _)| p
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("operation_"))
                .count(),
            124
        );
        assert_eq!(
            tree.iter()
                .filter(|(p, _)| p
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("service_"))
                .count(),
            3
        );
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let output = Command::new("go")
            .args(["test", "./..."])
            .current_dir(root.path().join("sdk"))
            .env("GOCACHE", root.path().join("go-cache"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
