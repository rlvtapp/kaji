//! Docs emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn readme(api: &Api, package: &str, module: &str, style: SdkClientStyle) -> String {
    let call = api.operations.first().map_or_else(
        || "// Call an operation on client.".to_owned(),
        |operation| {
            format!(
                "let response = try await client.{}()",
                function_name(&operation.id)
            )
        },
    );
    let style_note = match style {
        SdkClientStyle::Flat => "Operations are available directly on `PoolsterClient`.",
        SdkClientStyle::Namespaced => {
            "Operations are available directly on `PoolsterClient`; resource facades are also generated for navigation."
        }
    };
    let mut output = format!(
        "# {package}\n\nGenerated Swift SDK for {}. {style_note}\n\n```swift\nimport {module}\n\nlet client = PoolsterClient(options: .init(baseURL: URL(string: \"https://api.example.com\")!))\n{call}\n```\n\nUse Swift Package Manager to add this directory as a local package or publish it to a source-control repository.\n",
        api.name
    );
    output.push_str(&r#"
## Customer middleware

```swift
import Foundation
import MODULE
let transport = PoolsterMiddlewareTransport(inner: PoolsterURLSessionTransport()) { request, next in
    var request = request
    request.setValue("example", forHTTPHeaderField: "X-Customer")
    do {
        let (data, response) = try await next(request)
        // Inspect or replace data/response before normal SDK decoding.
        return (data, response)
    } catch {
        // Inspect, recover with a synthetic response, or propagate.
        throw error
    }
}
let client = PoolsterClient(options: .init(baseURL: URL(string: "https://api.example.com")!), transport: transport)
```

Nest wrappers for composition; outer layers see requests first and responses last. Implement `PoolsterTransport` directly to substitute execution, or return a response without calling `next`. Existing `session:` initialization remains supported. Middleware wraps the buffered transport once per attempt. Enable automatic retries with `PoolsterClientOptions(baseURL: ..., maxAttempts: 3, retryBaseDelay: 0.5, retryMaxDelay: 30)`; the default is one attempt. Attempts are capped at 10, delays at 60 seconds. Exponential backoff honors bounded `retry-after-ms` and `Retry-After` (seconds or HTTP date). GET/HEAD/OPTIONS/PUT/DELETE may replay; POST/PATCH require a nonblank standard idempotency key or an operation-declared custom key. Generated keys remain stable across attempts. Transient URLSession errors and HTTP 408/429/500/502/503/504 are eligible. Task cancellation interrupts backoff and is never retried. Buffered response decoding happens after retry selection, so decode errors do not replay requests. Errors from HTTP status validation and decoding occur after middleware. Captures must satisfy Swift `Sendable` rules. Lifecycle hooks remain separate notifications.
"#.replace("MODULE", module));
    if api.operations.iter().any(operation_is_sse) {
        output.push_str("\n## Server-sent events\n\nDeclared SSE operations return a lazy `PoolsterEventSequence`: use `for try await data in client.<operation>(...)`. Each element is the raw joined `data:` string; JSON decoding remains your choice. The first iterator advance opens the connection. Split UTF-8, CR/LF framing, comments and multiline data are supported; incomplete events at EOF are discarded. Cancellation or iterator destruction closes the stream. Streams have bounded queues and 1 MiB frame/line limits; overflow fails explicitly. SSE does not automatically retry or reconnect.\n\nThe default URLSession transport streams incrementally and denies redirects. A custom URLSession delegate requires an explicit `PoolsterStreamingTransport`. Buffered `PoolsterMiddlewareTransport` cannot intercept streaming requests and fails with an actionable capability error. Use `PoolsterStreamingMiddlewareTransport(inner: PoolsterURLSessionTransport()) { request, next in ... }` for streaming policies, or implement `PoolsterStreamingTransport.stream(_:)` returning `PoolsterByteStream`. Streaming middleware must retain cancellation and bounded delivery; lifecycle hooks receive headers with an empty buffered body.\n");
    }
    output
}

pub(crate) fn style_guide(api: &Api, module: &str, style: SdkClientStyle) -> String {
    let surface = match style {
        SdkClientStyle::Flat => "Operations are direct async methods on `PoolsterClient`.",
        SdkClientStyle::Namespaced => {
            "Operations remain direct async methods on `PoolsterClient`; named resource facades delegate to those methods without duplicating HTTP behavior."
        }
    };
    format!(
        "# {} Swift SDK style guide\n\nModule: `{module}`.\n\n{surface}\n\n## Errors\n\nUnsuccessful HTTP responses throw `PoolsterAPIError.status`. Transport and decoding errors are surfaced unchanged so callers can use normal Swift error handling.\n\n## Authentication and hooks\n\nSet API credentials or defaults through `PoolsterClientOptions.headers`. `PoolsterClientHook` provides request and response callbacks for telemetry, tracing, or policy.\n\n## Concurrency\n\nClients, models, and JSON values are `Sendable` where Swift's standard library permits it. Every generated operation is `async`.\n",
        api.name
    )
}
