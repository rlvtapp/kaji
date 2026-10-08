# Finding and downloading OpenAPI contracts

[CLI workflow](cli/README.md) · [Remote recipes](cli/recipes.md#private-remote-contract)

Kaji can search the public [APIs.guru directory](https://api.apis.guru) before
you have a spec URL. The directory is queried when the command runs; Kaji does
not retain API credentials or a local catalogue.

```sh
kaji discover github --limit 5
```

Human output gives each API's stable directory id, preferred version, title,
and download URL. Use JSON when another tool or agent needs a structured list:

```sh
kaji discover github --format json
```

Download the preferred version using its id. Kaji will not overwrite an
existing destination, so a mistaken directory query cannot replace a checked-in
contract.

```sh
kaji download github.com --output ./openapi/github.yaml
```

Use `--version` to choose a non-preferred version exposed by the directory:

```sh
kaji download github.com --version 1.1.4 --output ./openapi/github-1.1.4.yaml
```

The downloaded document is ordinary local input. Check it and then generate as
usual:

```sh
kaji check ./openapi/github.yaml
kaji generate ./openapi/github.yaml --output ./generated --language typescript
```

For private or unpublished APIs, skip the directory and supply a local file, a
public HTTPS URL, or the authenticated remote-input object documented in the
[configuration guide](cli/config.md).
