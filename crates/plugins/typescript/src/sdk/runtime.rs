use super::*;

pub(crate) fn poolster_runtime(
    transport: SdkTransport,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let security_types = render_security_types(security_schemes);
    let runtime = match transport {
        SdkTransport::Fetch => {
            format!(
                "{}\n{}",
                security_types,
                include_str!("runtime/fetch.ts.tmpl")
            )
        }
        SdkTransport::Axios => {
            format!(
                "{}\n{}",
                security_types,
                include_str!("runtime/axios.ts.tmpl")
            )
        }
    };
    let runtime = format!(
        "{}\n{}",
        runtime,
        include_str!("../../templates/multipart32.ts.tmpl")
    );
    let runtime = runtime.replace("paginationUrl?: string; idempotencyHeader?: string }", "paginationUrl?: string; idempotencyHeader?: string; jsonPlan?: JsonPlan }")
        .replace("validation, paginationUrl, idempotencyHeader, requestOptions })", "validation, paginationUrl, idempotencyHeader, requestOptions, jsonPlan })")
        .replace("formHeaders, config.multipartEncoder, multipartPlan)", "formHeaders, config.multipartEncoder, multipartPlan, jsonPlan)")
        .replace("multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan) =>", "multipartEncoder?: MultipartEncoder, multipartPlan?: MultipartPlan, jsonPlan?: JsonPlan) =>")
        .replace("return JSON.stringify(body)", "return jsonPlan?.lossless ? stringifyJson(body, requestJsonShape(jsonPlan, mediaType), jsonPlan.refs) : JSON.stringify(body)")
        .replace("const responseBody = async (response: Response, codecs?: Record<string, Codec>)", "const responseBody = async (response: Response, codecs?: Record<string, Codec>, jsonPlan?: JsonPlan)")
        .replace("return response.json()", "return jsonPlan?.lossless ? parseJson(await response.text(), responseJsonShape(jsonPlan, response.status, contentType), jsonPlan.refs) : response.json()")
        .replace("responseBody(response.clone(), config.codecs)", "responseBody(response.clone(), config.codecs, jsonPlan)")
        .replace("  return body\n}", "  return jsonPlan?.lossless && (mediaType.includes('json') || mediaType.endsWith('+json')) ? stringifyJson(body, requestJsonShape(jsonPlan, mediaType), jsonPlan.refs) : body\n}")
        .replace("responseType === 'arraybuffer' ? 'arraybuffer' : undefined", "responseType === 'arraybuffer' ? 'arraybuffer' : jsonPlan?.lossless ? 'text' : undefined")
        .replace("validateStatus: () => true })", "validateStatus: () => true, ...(jsonPlan?.lossless && responseType !== 'stream' && responseType !== 'arraybuffer' ? { transformResponse: [(value: unknown) => value] } : {}) })")
        .replace("?? response.data", "?? (jsonPlan?.lossless && typeof response.data === 'string' && (mediaType.includes('json') || mediaType.includes('+json')) ? parseJson(response.data, responseJsonShape(jsonPlan, response.status, mediaType), jsonPlan.refs) : response.data)")
        .replace("(response: Promise<unknown>): Promise<EventStreamResult<T>>", "(response: Promise<unknown>, jsonPlan?: JsonPlan): Promise<EventStreamResult<T>>")
        .replace("const stream = raw instanceof Response ? raw.body : raw as ReadableStream<Uint8Array> | null", "const stream = raw instanceof Response ? raw.body : (raw && typeof raw === 'object' && 'data' in raw ? raw.data : raw) as ReadableStream<Uint8Array> | null")
        .replace("yield JSON.parse(data) as T", "yield (jsonPlan ? parseJson(data, responseJsonShape(jsonPlan, eventStreamStatus(raw), 'text/event-stream'), jsonPlan.refs) : JSON.parse(data)) as T");
    format!(
        "{}\n{}\n{}",
        runtime,
        include_str!("../../templates/request_control.ts.tmpl"),
        crate::json::RUNTIME
    )
}

pub(crate) fn render_security_types(security_schemes: Option<&SecuritySchemeCatalog>) -> String {
    let fields = security_schemes
        .map(|catalog| {
            catalog
                .schemes
                .iter()
                .map(|scheme| {
                    format!(
                        "  {}?: string",
                        serde_json::to_string(&scheme.name).unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|fields| !fields.is_empty())
        .unwrap_or_else(|| "  [name: string]: string | undefined".into());
    format!(
        "export interface SecurityCredentials {{\n{fields}\n}}\nexport type SecurityDescriptor = {{ id: string; type: 'apiKey' | 'http' | 'oauth2'; name?: string; in?: 'header' | 'query' | 'cookie'; scheme?: string; scopes?: string[] }}\nexport class ApiError extends Error {{\n  constructor(public readonly status: number, public readonly body: unknown) {{ super(`Request failed: ${{status}}`) }}\n}}"
    )
}
