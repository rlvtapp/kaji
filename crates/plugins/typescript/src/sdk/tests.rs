use super::*;
use poolster_core::{
    HttpMethod, OperationMediaType, OperationRequestBody, SecurityRequirement, SecurityScheme,
    SecuritySchemeKind,
};
#[test]
fn structured_typescript_sdk_uses_named_catalog_credentials() {
    let api = Api {
        name: "Secure API".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "listMessages".into(),
            method: HttpMethod::Get,
            path: "/messages".into(),
            responses: vec![poolster_core::OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(poolster_core::SchemaValue::reference(
                        "#/components/schemas/MessageList",
                    )),
                }],
            }],
            security: vec![SecurityRequirement {
                schemes: [("poolsterApiKey".into(), Vec::new())]
                    .into_iter()
                    .collect(),
            }],
            ..Operation::default()
        }],
        ..Api::default()
    };
    let catalog = SecuritySchemeCatalog {
        schemes: vec![SecurityScheme {
            name: "poolsterApiKey".into(),
            description: None,
            kind: SecuritySchemeKind::ApiKey {
                name: Some("X-API-Key".into()),
                location: Some("header".into()),
            },
        }],
    };

    let tree = generate_sdk(&api, &SdkConfig::new("sdk/typescript"), Some(&catalog)).unwrap();
    let runtime = tree.get("sdk/typescript/.poolster/client.ts").unwrap();
    let operation = tree
        .get("sdk/typescript/clients/messages/listMessages.ts")
        .unwrap();
    assert!(runtime.contains("\"poolsterApiKey\"?: string"));
    assert!(runtime.contains("applySecurity"));
    assert!(operation.contains("id: 'poolsterApiKey'"));
    assert!(operation.contains("name: 'X-API-Key', in: 'header'"));
}

#[test]
#[ignore = "requires Node and POOLSTER_TSC_JS"]
fn generated_page_pagination_executes_defaults_and_selectors() {
    let compiler = std::env::var("POOLSTER_TSC_JS").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let iterator = render_pagination_iterator(
        "listItems",
        "listItems",
        &Pagination::OffsetLimit(OffsetPagination {
            step: OffsetStep::Page(PaginationInput {
                name: "page".into(),
                location: "query".into(),
                body_path: None,
            }),
            limit: Some(PaginationInput {
                name: "limit".into(),
                location: "query".into(),
                body_path: None,
            }),
            results_path: Some("/a~1b/~0items/0".into()),
            num_pages_path: None,
        }),
    );
    let source = format!(
        "type ClientInstance = (request: any) => Promise<any>;\nconst seen: number[]=[];\nconst listItems=async (options: any) => {{ seen.push(options.query.page); return {{'a/b':{{'~items':[options.query.page < 3 || options.query.page === Number.MAX_SAFE_INTEGER ? [options.query.page] : []]}}}} }};\n{}\nclass Pages {{ transport: ClientInstance=async request=>request; listItemsPages!: (options: any) => AsyncGenerator<any>; constructor() {{ {} }} }}\n(async()=>{{ const p=new Pages(); let count=0; for await (const page of p.listItemsPages({{}})) count++; if(count!==3 || seen.join(',')!=='1,2,3') throw Error('default page failed'); seen.length=0; for await (const page of p.listItemsPages({{query:{{page:0,limit:2}}}})) {{}}; if(seen.join(',')!=='0') throw Error('zero or short limit failed'); if(poolsterJsonPath([{{items:[1,2]}}], '$[0].items[-1]')!==2) throw Error('selector failed'); if(poolsterJsonPath([10,20], '/01')!==undefined || poolsterJsonPath([10,20], '/-1')!==undefined || poolsterJsonPath([10,20], '/')!==undefined || poolsterJsonPath({{'a/b': 1}}, '/a~2b')!==undefined) throw Error('invalid pointer accepted'); const original={{query:{{page:0,limit:2}}}}; for await (const page of p.listItemsPages(original)) {{}}; if(original.query.page!==0) throw Error('caller mutated'); seen.length=0; for await(const page of p.listItemsPages({{query:{{page:Number.MAX_SAFE_INTEGER}}}})) {{}}; if(seen.length!==1) throw Error('unsafe page advanced'); for(const limit of [true,0,-1,1.5]) {{ let failed=false; try {{ for await (const result of p.listItemsPages({{query:{{limit}}}})) {{}} }} catch {{ failed=true }} if(!failed) throw Error('bad limit accepted'); }} for(const page of [-1,1.5,NaN]) {{ let failed=false; try {{ for await (const result of p.listItemsPages({{query:{{page}}}})) {{}} }} catch {{ failed=true }} if(!failed) throw Error('bad page accepted'); }} }})().catch(error=>{{ console.error(error); throw error }});",
        pagination_helpers(),
        iterator
    );
    std::fs::write(directory.path().join("page.ts"), &source).unwrap();
    let output = std::process::Command::new("node")
        .arg(compiler)
        .args([
            "--strict", "--target", "ES2022", "--module", "commonjs", "page.ts",
        ])
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new("node")
            .arg("page.js")
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn offset_limit_pagination_advances_from_declared_results() {
    let iterator = render_pagination_iterator(
        "listPages",
        "listThings",
        &Pagination::OffsetLimit(OffsetPagination {
            step: OffsetStep::Offset(PaginationInput {
                name: "offset".into(),
                location: "query".into(),
                body_path: None,
            }),
            limit: Some(PaginationInput {
                name: "limit".into(),
                location: "query".into(),
                body_path: None,
            }),
            results_path: Some("$.data.results".into()),
            num_pages_path: None,
        }),
    );
    assert!(iterator.contains("poolsterJsonPath(response, \"$.data.results\")"));
    assert!(iterator.contains("currentValue + items.length"));
    assert!(iterator.contains("poolsterWithValue(current, \"query\", \"offset\", nextValue)"));
}

#[test]
fn body_pagination_requires_an_explicit_pointer_for_nested_values() {
    let operation = Operation {
        id: "listThings".into(),
        request_body: Some(OperationRequestBody {
            required: false,
            description: None,
            media_types: vec![OperationMediaType {
                content_type: "application/json".into(),
                schema: None,
            }],
        }),
        annotations: BTreeMap::from([(
            "x-poolster-pagination".into(),
            serde_json::json!({
                "type": "cursor",
                "inputs": [{
                    "name": "cursor",
                    "in": "requestBody",
                    "type": "cursor",
                    "bodyPath": "/filters/page/cursor"
                }],
                "outputs": { "nextCursor": "$.meta.next" }
            }),
        )]),
        ..Operation::default()
    };
    let Some(Pagination::Cursor(cursor_pager)) = pagination(&operation) else {
        panic!("explicit nested body pagination should be recognized")
    };
    assert_eq!(
        cursor_pager.input_body_path.as_deref(),
        Some("/filters/page/cursor")
    );
    let iterator =
        render_pagination_iterator("listPages", "listThings", &Pagination::Cursor(cursor_pager));
    assert!(iterator.contains("poolsterWithBodyValue(current, \"/filters/page/cursor\", cursor)"));
    assert!(iterator.contains("const next = poolsterWithBodyValue"));
    assert!(pagination_helpers().contains("poolsterJsonPointer"));

    let invalid = Operation {
        annotations: BTreeMap::from([(
            "x-poolster-pagination".into(),
            serde_json::json!({
                "type": "cursor",
                "inputs": [{
                    "name": "cursor",
                    "in": "requestBody",
                    "type": "cursor",
                    "bodyPath": "/filters/page/not-cursor"
                }],
                "outputs": { "nextCursor": "$.meta.next" }
            }),
        )]),
        ..Operation::default()
    };
    assert!(pagination(&invalid).is_none());
}

#[test]
fn body_offset_pagination_uses_the_declared_pointer_without_mutation() {
    let pagination = OffsetPagination {
        step: OffsetStep::Offset(PaginationInput {
            name: "offset".into(),
            location: "body".into(),
            body_path: Some("/page/offset".into()),
        }),
        limit: Some(PaginationInput {
            name: "limit".into(),
            location: "body".into(),
            body_path: Some("/page/limit".into()),
        }),
        results_path: Some("$.items".into()),
        num_pages_path: None,
    };
    let rendered = render_offset_pagination(&pagination);
    assert!(rendered.contains("poolsterBodyValue(current, \"/page/offset\")"));
    assert!(rendered.contains("poolsterWithBodyValue(current, \"/page/offset\", nextValue)"));
}

#[test]
fn url_pagination_reenters_the_generated_operation_with_a_safe_continuation() {
    let iterator = render_pagination_iterator(
        "listPages",
        "listThings",
        &Pagination::Url(UrlPagination {
            next_url_path: "$.links.next".into(),
        }),
    );
    assert!(iterator.contains("const request: ClientInstance"));
    assert!(iterator.contains("paginationClient({ ...operation, paginationUrl: nextUrl })"));
    assert!(iterator.contains("await listThings({ ...requestOptions, client: request }"));
    assert!(iterator.contains("query: undefined"));
    assert!(iterator.contains("poolsterPaginationUrl(response, \"$.links.next\")"));
    assert!(!iterator.contains("url: nextUrl"));

    let fetch_runtime = poolster_runtime(SdkTransport::Fetch, None);
    assert!(fetch_runtime.contains("paginationUrl?: string"));
    assert!(fetch_runtime.contains("Pagination URL must use the configured API origin"));
    assert!(
        fetch_runtime
            .contains("applySecurity(mergedHeaders, resolvedQuery, security, config.auth)")
    );

    let axios_runtime = poolster_runtime(SdkTransport::Axios, None);
    assert!(axios_runtime.contains("Pagination URL must use the configured API origin"));
    assert!(
        axios_runtime
            .contains("applySecurity(resolvedHeaders, resolvedQuery, security, config.auth)")
    );
}

#[test]
fn url_pagination_uses_the_declared_next_url_output() {
    let operation = Operation {
        id: "listThings".into(),
        annotations: BTreeMap::from([(
            "x-speakeasy-pagination".into(),
            serde_json::json!({
                "type": "url",
                "outputs": { "nextUrl": "$.links.next" },
            }),
        )]),
        ..Operation::default()
    };
    let Some(Pagination::Url(pagination)) = pagination(&operation) else {
        panic!("URL pagination should be recognized")
    };
    assert_eq!(pagination.next_url_path, "$.links.next");
}

#[test]
fn package_versions_accept_openapi_date_versions() {
    assert_eq!(package_version("2026-09-19"), "2026.9.19");
    assert_eq!(package_version("1.2.3"), "1.2.3");
    assert_eq!(package_version("latest"), "0.1.0");
}

#[test]
fn fetch_runtime_has_openapi_serializers_codecs_and_status_results() {
    let runtime = poolster_runtime(SdkTransport::Fetch, None);
    assert!(runtime.contains("export interface Codec"));
    assert!(runtime.contains("export type StatusResult<T>"));
    assert!(runtime.contains("export type ResponseResult<T"));
    assert!(runtime.contains("export const resolveResponse"));
    assert!(!runtime.contains("withUnwrap"));
    assert!(runtime.contains("serializePath"));
    assert!(runtime.contains("deepObject"));
    assert!(runtime.contains("multipart/form-data"));
    assert!(runtime.contains("application/x-www-form-urlencoded"));
    assert!(runtime.contains("export type FormEncoding"));
    assert!(runtime.contains("export type FormPartHeader"));
    assert!(runtime.contains("export interface MultipartEncoder"));
    assert!(runtime.contains("Native FormData cannot set per-part headers"));
    assert!(runtime.contains("Missing required multipart header"));
    assert!(runtime.contains("contentType: response.headers.get('content-type')"));
    assert!(runtime.contains("scheme.in === 'cookie'"));
    assert!(runtime.contains("export interface StandardSchema"));
    assert!(runtime.contains("await config.hooks?.afterResponse?.({ request, status: response.status, response: response.clone() })\n  if (!response.ok"));
    assert!(
        runtime.contains("validate(validation?.response ?? config.validation?.response, data)")
    );
}

#[test]
fn axios_runtime_has_openapi_serializers_codecs_and_status_results() {
    let runtime = poolster_runtime(SdkTransport::Axios, None);
    assert!(runtime.contains("export type FormEncoding"));
    assert!(runtime.contains("export type FormPartHeader"));
    assert!(runtime.contains("export interface MultipartEncoder"));
    assert!(runtime.contains("Native FormData cannot set per-part headers"));
    assert!(runtime.contains("contentType: mediaType.split(';')[0].trim()"));
    assert!(runtime.contains("export interface Codec"));
    assert!(runtime.contains("export type StatusResult<T>"));
    assert!(runtime.contains("export type ResponseResult<T"));
    assert!(runtime.contains("export const resolveResponse"));
    assert!(!runtime.contains("withUnwrap"));
    assert!(runtime.contains("serializeQuery"));
    assert!(runtime.contains("encodeBody"));
    assert!(runtime.contains("multipart/form-data"));
    assert!(runtime.contains("config.codecs"));
    assert!(runtime.contains("export interface StandardSchema"));
    assert!(runtime.contains("await config.hooks?.afterResponse?.({ request, status: response.status, headers: response.headers as Record<string, unknown>, data })\n        if (response.status >= 400"));
    assert!(runtime.contains(
            "if (signal?.aborted || axios.isCancel(error) || !axios.isAxiosError(error) || attempt + 1 >= maxAttempts)"
        ));
    assert!(runtime.contains("validate(validation?.request ?? config.validation?.request, body)"));
}

#[test]
fn sdk_client_names_strip_the_openapi_api_suffix() {
    assert_eq!(sdk_client_name("Poolster Email API"), "PoolsterEmail");
    assert_eq!(sdk_client_name("Mistral API"), "Mistral");
}
