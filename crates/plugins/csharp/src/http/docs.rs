//! Docs emission for the csharp HTTP SDK.
use crate::*;

pub(crate) fn render_readme(
    api: &Api,
    package: &str,
    namespace: &str,
    client_style: SdkClientStyle,
) -> String {
    let style = if client_style == SdkClientStyle::Namespaced {
        "namespaced"
    } else {
        "flat"
    };
    let operation = api
        .operations
        .first()
        .map(|operation| pascal_case(&operation.id))
        .unwrap_or_else(|| "Operation".into());
    let resource = operation_groups(api)
        .first()
        .cloned()
        .unwrap_or_else(|| "Default".into());
    let middleware = r#"## Runtime customization

Inject an `HttpClient` with a customer `DelegatingHandler`:

```csharp
sealed class CustomerPolicy : DelegatingHandler
{
    protected override async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request, CancellationToken cancellationToken)
    {
        request.Headers.TryAddWithoutValidation("X-Customer", "customer-a");
        var response = await base.SendAsync(request, cancellationToken);
        response.Headers.TryAddWithoutValidation("X-Customer-Policy", "applied");
        return response;
    }
}

// Inside application setup:
var handler = new CustomerPolicy { InnerHandler = new HttpClientHandler() };
var httpClient = new HttpClient(handler);
// Pass httpClient to the PoolsterClient constructor shown above.
```

The handler can replace request URI, method and content before forwarding;
replace response status/content after forwarding; catch and translate transport
errors; or return a synthetic response without calling `base.SendAsync`. Preserve
cancellation and dispose any discarded response/content. Observer hooks do not
return replacement messages. The handler sees each transport attempt, including
SDK retries. Keep rewrites repeatable and preserve stream ownership when handling
streaming responses. Reuse the configured client for the application's lifetime.
"#;
    format!(
        "# {package}\n\nGenerated .NET 8 client for {}. This release selected the **{style}** client style.\n\n```csharp\nusing {namespace};\n\nvar client = new PoolsterClient(httpClient, new PoolsterClientOptions\n{{\n    BaseUrl = \"https://api.example.com\",\n    ApiKey = Environment.GetEnvironmentVariable(\"API_KEY\"),\n}});\n```\n\n## Flat client\n\n```csharp\nawait client.{operation}Async(/* typed arguments */);\n```\n\n## Namespaced client\n\n```csharp\nawait client.{resource}.{operation}Async(/* typed arguments */);\n```\n\nThe namespaced example applies to packages generated with `SdkClientStyle::Namespaced`; direct `PoolsterClient` methods remain available in that mode. See [STYLE_GUIDE.md](STYLE_GUIDE.md) for selection guidance.\n\n{middleware}",
        api.name,
    )
}

pub(crate) fn render_style_guide(
    api: &Api,
    namespace: &str,
    client_style: SdkClientStyle,
) -> String {
    let operation = api
        .operations
        .first()
        .map(|operation| pascal_case(&operation.id))
        .unwrap_or_else(|| "Operation".into());
    let resource = operation_groups(api)
        .first()
        .cloned()
        .unwrap_or_else(|| "Default".into());
    let selected = match client_style {
        SdkClientStyle::Flat => "flat",
        SdkClientStyle::Namespaced => "namespaced",
    };
    format!(
        "# .NET SDK style guide\n\nThis generated package selected the **{selected}** client layout. Both layouts retain the same models, authentication options, cancellation support, and typed return values.\n\n## Flat\n\nUse the direct operation methods when a compact client is preferable.\n\n```csharp\nusing {namespace};\n\nvar client = new PoolsterClient(httpClient, options);\nawait client.{operation}Async(/* typed arguments */);\n```\n\n## Namespaced\n\nUse resource properties to make a larger API easier to navigate. Poolster uses the first OpenAPI tag; if none is present, it derives the first useful path segment while skipping `v#`, `api`, and `email`.\n\n```csharp\nusing {namespace};\n\nvar client = new PoolsterClient(httpClient, options);\nawait client.{resource}.{operation}Async(/* typed arguments */);\n```\n\n## Pagination\n\nDeclared `type: page` and `type: offsetLimit` integer query inputs expose `OperationPagesAsync` on `PoolsterClient`. Page inputs default to 1 and offsets to 0 when omitted; explicit zero is preserved. Helpers yield normal response pages, stop on empty or short result arrays, check cancellation, guard overflow, and stop after 10,000 pages. Results selectors use the shared validated JSONPath or RFC 6901 pointer subset. Page/offset helpers require direct int32/int64 integer query schemas; unsupported locations, references, required nullable controls, and control-name collisions produce generation errors. URL pagination is not emitted. Direct operation methods remain available.\n\n## Media and streaming\n\nNon-JSON success responses are returned as `byte[]`; non-JSON request bodies accept `byte[]`. A `text/event-stream` operation returns `IAsyncEnumerable<string>` containing complete event data payloads (multiline data fields are joined with a newline; metadata and comments are ignored).\n\nSelect the layout during generation with `SdkClientStyle::Flat` or `SdkClientStyle::Namespaced`. The original direct `PoolsterClient` methods remain available when the namespaced façade is selected.\n"
    )
}
