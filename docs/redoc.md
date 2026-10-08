# Generate a ReDoc entry point

SDK authors can publish an API reference next to generated clients using the
`artifacts` target. It does not depend on TypeScript SDK output despite sharing
that plugin's rendering utilities.

```json
{
  "openapi": { "input": "./openapi.yaml", "name": "Example API", "version": "1.0.0" },
  "output": { "path": "./generated" },
  "packages": [
    {
      "language": "artifacts",
      "path": "reference",
      "plugins": [
        { "name": "redoc", "title": "Example API reference", "openapi_spec": "./openapi.yaml" }
      ]
    }
  ]
}
```

Run `poolster generate --config poolster.json`. The package contains `redoc/redoc.html`
and `redoc/redocly.yaml`. Copy a public, credential-free OpenAPI export to
`generated/reference/redoc/openapi.yaml` before hosting. `openapi_spec` is a
browser URL resolved relative to the HTML page, not a file Poolster copies or bundles.
Serve the folder over HTTP so the browser can fetch the specification. If you use
an absolute specification URL, its host must permit the browser's CORS request.

The HTML loads ReDoc's JavaScript from its public CDN. Generation itself needs no
browser or CDN connection, but the emitted page is not an offline distribution.
For a reproducible or offline site, maintain an author-owned HTML wrapper that
loads a pinned, locally distributed ReDoc bundle. Keep that wrapper separate from
owned generated files, or apply a deliberate source customization.

## Supported subset and validation

This artifact provides an escaped HTML title/specification URL and a starter
Redocly theme configuration. It does not validate the specification, copy referenced
files, infer auth credentials, execute a documentation build, or publish hosting.
Use the compiler's bundled specification export when external references would
otherwise be inaccessible to the browser. SDK author middleware and plugins affect
SDK generation; the reference displays the specification you host.

The renderer regression verifies the generated HTML/config shape. Browser rendering,
CORS, accessibility, CDN availability and the selected ReDoc release's interpretation
of newer OpenAPI features need a site-level check before publication.
