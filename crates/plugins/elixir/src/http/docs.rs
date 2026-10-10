//! Docs emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn render_readme(
    api: &Api,
    package: &str,
    module: &str,
    client_style: SdkClientStyle,
) -> String {
    let operation = api
        .operations
        .first()
        .map(|operation| elixir_identifier(&operation.id))
        .unwrap_or_else(|| "operation".into());
    let resource = operation_groups(api)
        .first()
        .cloned()
        .unwrap_or_else(|| "Default".into());
    let style = match client_style {
        SdkClientStyle::Flat => "flat",
        SdkClientStyle::Namespaced => "namespaced",
    };
    let mut output = format!(
        "# {package}\n\nGenerated Elixir SDK for {}. This release selected the **{style}** client style.\n\n```elixir\n{{:ok, client}} = {module}.client(base_url: \"https://api.example.com\", api_key: System.get_env(\"API_KEY\"))\n```\n\n## Flat client\n\n```elixir\n{{:ok, result}} = {module}.API.{operation}(client)\n```\n\n## Namespaced client\n\n```elixir\n{{:ok, result}} = {module}.Resources.{resource}.{operation}(client)\n```\n\nThe namespaced example applies to packages generated with `SdkClientStyle::Namespaced`; direct `{module}.API` functions remain available in that mode. The client uses Finch, returns `{{:ok, value}}` on successful HTTP responses, and returns `{{:error, reason}}` for transport or non-2xx API errors. See [STYLE_GUIDE.md](STYLE_GUIDE.md) for selection guidance.\n",
        api.name,
    );
    output.push_str(&r#"
## Customer middleware

```elixir
customer_header = fn request, next ->
  request = %{request | headers: [{"x-customer", "example"} | request.headers]}
  case next.(request) do
    {:ok, response} -> {:ok, response} # inspect or replace a Finch.Response
    {:error, reason} -> {:error, reason} # inspect, recover, or propagate
  end
end
{:ok, client} = MODULE.client(
  base_url: "https://api.example.com", middleware: [customer_header])
```

The first middleware is outermost; requests run forward and responses return backward. Layers receive encoded `Finch.Request` values and can return `{:ok, %Finch.Response{}}` without invoking `next`. Optional `transport: fn request, options -> ... end` replaces execution; the default uses the configured Finch pool. Middleware executes once per SDK retry attempt, so avoid independently replaying unsafe requests. Status/decoding classification happens afterwards; lifecycle callbacks are separate notifications.

SSE uses an independent `stream_transport: fn request, options, accumulator, callback -> ... end` returning Finch-compatible stream results and delivering `{:status, code}`, `{:headers, headers}`, and `{:data, binary}` through `callback`. Buffered middleware does not process SSE frames. Decorate that callback to transform events, retaining cancellation/task lifetime behavior.
"#.replace("MODULE", module));
    output
}

pub(crate) fn render_style_guide(api: &Api, module: &str, client_style: SdkClientStyle) -> String {
    let operation = api
        .operations
        .first()
        .map(|operation| elixir_identifier(&operation.id))
        .unwrap_or_else(|| "operation".into());
    let resource = operation_groups(api)
        .first()
        .cloned()
        .unwrap_or_else(|| "Default".into());
    let selected = match client_style {
        SdkClientStyle::Flat => "flat",
        SdkClientStyle::Namespaced => "namespaced",
    };
    format!(
        "# Elixir SDK style guide\n\nThis generated package selected the **{selected}** layout. Both options use the same normal `{module}.Client` value, Finch transport, and typed direct API functions.\n\n## Flat\n\nUse direct functions from the API module.\n\n```elixir\n{{:ok, client}} = {module}.client(base_url: \"https://api.example.com\")\n{{:ok, result}} = {module}.API.{operation}(client)\n```\n\n## Namespaced\n\nUse resource modules to navigate larger APIs. Poolster uses the first OpenAPI tag; when tags are absent it derives the first useful path segment, skipping `v#`, `api`, and `email`.\n\n```elixir\n{{:ok, client}} = {module}.client(base_url: \"https://api.example.com\")\n{{:ok, result}} = {module}.Resources.{resource}.{operation}(client)\n```\n\nSelect the layout during generation with `SdkClientStyle::Flat` or `SdkClientStyle::Namespaced`. Namespaced packages keep the direct `{module}.API` functions too.\n"
    )
}
