use super::*;

pub(crate) fn sdk_client_name(api_name: &str) -> String {
    let name = pascal_identifier(api_name);
    let name = name
        .get(..name.len().saturating_sub(3))
        .filter(|_| name[name.len().saturating_sub(3)..].eq_ignore_ascii_case("api"))
        .unwrap_or(&name);
    if name.is_empty() {
        "ApiClient".into()
    } else {
        name.into()
    }
}

pub(crate) fn operation_group(operation: &Operation) -> String {
    operation_tag_directory_if_present(operation).unwrap_or_else(|| sdk_namespace(operation))
}

pub(crate) fn operation_tag_directory(operation: &Operation) -> String {
    sdk_namespace(operation)
}

pub(crate) fn operation_tag_directory_if_present(operation: &Operation) -> Option<String> {
    operation
        .annotations
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .and_then(|tags| tags.first())
        .and_then(serde_json::Value::as_str)
        .map(lower_camel_identifier)
        .filter(|tag| !tag.is_empty())
}

pub(crate) fn poolster_package(
    api: &Api,
    transport: SdkTransport,
    name: Option<&str>,
) -> Result<String> {
    let package_name = name
        .map(str::to_owned)
        .unwrap_or_else(|| poolster_package_name(api, transport));
    let mut package = serde_json::json!({
        "name": package_name,
        "version": package_version(&api.version),
        "type": "module",
        "sideEffects": false,
        "exports": {
            ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
            "./*": { "types": "./dist/*.d.ts", "import": "./dist/*.js" }
        },
        "files": ["dist"],
        "scripts": { "build": "tsc -p tsconfig.json" },
        "devDependencies": { "typescript": "^5.9.3" }
    });
    if transport == SdkTransport::Axios {
        package["peerDependencies"] = serde_json::json!({ "axios": "^1.0.0" });
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&package)?))
}

pub(crate) fn poolster_package_name(api: &Api, transport: SdkTransport) -> String {
    format!(
        "@poolster/{}-{}",
        package_slug(&api.name),
        match transport {
            SdkTransport::Fetch => "fetch",
            SdkTransport::Axios => "axios",
        }
    )
}

pub(crate) fn poolster_readme(api: &Api, profile: &SdkConfig) -> String {
    let package_name = profile
        .package_name
        .clone()
        .unwrap_or_else(|| poolster_package_name(api, profile.transport));
    let client_name = profile
        .client_name
        .clone()
        .unwrap_or_else(|| sdk_client_name(&api.name));
    let example = api
        .operations
        .iter()
        .find(|operation| {
            operation.id.starts_with("list")
                && operation
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.required)
                && operation
                    .request_body
                    .as_ref()
                    .is_none_or(|body| !body.required)
        })
        .or_else(|| {
            api.operations.iter().find(|operation| {
                operation
                    .parameters
                    .iter()
                    .all(|parameter| !parameter.required)
                    && operation
                        .request_body
                        .as_ref()
                        .is_none_or(|body| !body.required)
            })
        })
        .map(|operation| {
            let namespace = sdk_namespace(operation);
            let method = sdk_method_name(operation, &namespace);
            format!(
                "const response = await client.{namespace}.{method}({{}});\nconsole.log(response);"
            )
        })
        .unwrap_or_else(|| "// Call a generated resource method with its typed options.".into());
    let client = if profile.surface == SdkSurface::Client {
        format!(
            "import {{ {client_name} }} from {package_name:?};\n\nconst client = new {client_name}({{\n  baseUrl: \"https://api.example.com\",\n  apiKey: process.env.API_KEY,\n}});\n\n{example}"
        )
    } else {
        "// This package was generated with the raw surface; import the direct operation functions from the package entrypoint.".into()
    };
    let middleware = format!(
        "## Runtime middleware\n\nFetch and Axios clients accept `middleware` in their configuration.\n\n```ts\nimport {{ createClient, type ClientMiddleware }} from {package_name:?};\n\nconst customerPolicy: ClientMiddleware = async (request, next) => {{\n  try {{\n    return await next({{ ...request, query: {{ ...request.query, tenant: 'customer-a' }} }});\n  }} catch (cause) {{\n    throw new Error('Customer API request failed', {{ cause }});\n  }}\n}};\nconst transport = createClient({{ middleware: [customerPolicy] }});\n// Pass the same middleware config to the generated SDK constructor.\n```\n\nThe first middleware is outermost. Call `next(updatedRequest)` at most once; request changes apply before serialization and authentication. Replace the returned response envelope to rewrite data, or return an envelope without calling `next` to short circuit. A short circuit bypasses remaining middleware, transport hooks and transport validation callbacks. Optional response shape checks still run after the completed chain. Middleware runs once per logical call; built-in retries remain inside `next`. Existing hooks keep their transport timing. Ordinary responses contain `status`, `contentType`, `data` and `headers`. Preserve native Fetch `Response` objects for streams, and Axios stream envelopes with a readable `data` stream.\n"
    );
    let middleware = format!(
        "{middleware}\n## Response shape checks\n\nSet `validateResponses: true` in client configuration to check declared buffered successful JSON response shapes, including responses returned or rewritten by middleware. Checks are disabled by default; a request can override the setting. `ResponseDecodeError` reports the failing path without response values. Extra fields and new enum strings remain accepted. HEAD, 204, SSE, error responses, and absent or unmatched response schemas are outside this scope. These checks cover structural types, required properties, nullability, and supported compositions, rather than all JSON Schema constraints.\n"
    );
    format!(
        "# {} TypeScript SDK\n\nGenerated by Poolster.\n\n```sh\nnpm install {package_name}\n```\n\n```ts\n{client}\n```\n\nSee [STYLE_GUIDE.md](STYLE_GUIDE.md) for the selected client surface.\n\n{middleware}",
        api.name
    )
}

/// Normalizes common OpenAPI release labels into a version accepted by npm
/// and Cargo. APIs often publish a date (for example `2026-09-19`) rather
/// than a semver version.
pub(crate) fn package_version(version: &str) -> String {
    let pieces: Vec<_> = version.split('-').collect();
    if pieces.len() == 3
        && pieces.iter().all(|piece| {
            !piece.is_empty() && piece.chars().all(|character| character.is_ascii_digit())
        })
    {
        return pieces
            .iter()
            .map(|piece| piece.parse::<u64>().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join(".");
    }
    let semver_parts: Vec<_> = version.split('.').collect();
    if semver_parts.len() == 3
        && semver_parts.iter().all(|piece| {
            !piece.is_empty()
                && piece.chars().all(|character| {
                    character.is_ascii_digit()
                        || character == '-'
                        || character.is_ascii_alphabetic()
                })
        })
    {
        return version.to_owned();
    }
    "0.1.0".to_owned()
}
