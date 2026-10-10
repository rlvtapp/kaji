# Symfony GraphQL bundles

The Symfony output includes the portable PHP GraphQL SDK and a Symfony bundle in
one Composer package. Use named query/mutation documents. Generated models,
variables, error envelopes and raw/flat/grouped methods are the PHP SDK's existing
implementation; the bundle adds HttpClient transport and container services.
This is unreleased checkout functionality.

## Generate

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language symfony --output generated
```

A JSON recipe can configure the package and style:

```json
{
  "input": { "path": "schema.graphql", "format": "graphql", "options": { "operation_files": ["operations.graphql"] } },
  "output": "generated",
  "packages": [{ "language": "symfony", "path": "bundle", "name": "acme/graphql", "plugins": [{ "name": "graphql", "style": "flat" }] }]
}
```

For npm generation, use a native SDK descriptor in your configuration:

```js
plugins: [{ kind: 'native-sdk', name: 'symfony', package: {
  language: 'symfony', path: 'bundle', name: 'acme/graphql',
  contracts: { graphql: { style: 'flat' } },
} }]
```

Rust uses `symfony::package("bundle").name("acme/graphql")
.with(symfony::graphql(Some(input.handle())).flat()).with(input)`, where `input`
is an `InputProvider<GraphqlOperations>`. Custom group mappings use `.group(...)`
or the same `groups` option as PHP. `sdk_package` is an HTTP-only option; GraphQL
bundles include their SDK.

## Install and configure

Install the generated Composer package, enable
`Acme\Graphql\Symfony\AcmeGraphqlBundle` in `config/bundles.php`, and configure:

```yaml
# config/packages/graphql.yaml
acme_graphql:
  endpoint: '%env(GRAPHQL_ENDPOINT)%'
  headers:
    Authorization: 'Bearer %env(GRAPHQL_TOKEN)%'
  timeout: 30.0
  http_client: http_client
```

Autowire `Acme\Graphql\Client` in application services. With flat output:

```php
$response = $client->readUser(new \Acme\Graphql\ReadUserVariables('42'));
$user = $response->requireData()->user;
```

`requireData()` throws for GraphQL errors; inspect `data`, `errors` and `extensions`
when handling partial results. The endpoint is required and must be HTTP(S), the
timeout must be positive, and `http_client` can name a scoped HttpClient service.
The bundle disables redirects and adds no retries. A supplied retrying client can
still retry mutations; its policy belongs to the application.

For runtime scalar callbacks, set the generated client's `$scalarCodecs` constructor
argument through PHP container configuration or a compiler pass. Callbacks retain
the portable PHP SDK's mixed value boundary and preserve missing/null values.

## Boundaries and checks

Symfony 6.4 and 7.x dependency ranges are emitted. Tests use locked Symfony 6.4.26
components on PHP 8.2, compile the real container, resolve a private client through
an autowired consumer, lint every generated PHP source and execute raw/flat/grouped/custom-group
queries and mutations against pinned GraphQL.js 16.14.2 locally. MockHttpClient
checks authentication, request policy, partial errors, non-2xx responses, malformed
JSON and scalar decoding. CLI/npm generation and regeneration have separate tests.

Subscriptions and incremental delivery are unsupported by this Symfony generator;
use the separate PHP SDK for its native streaming capabilities. No Symfony-specific
streaming adaptation is claimed. Existing OpenAPI Symfony generation is unchanged.

To reproduce the native probe locally, install the locked Composer dependencies
in `crates/plugins/symfony/tests/fixtures/symfony64`, set
`POOLSTER_SYMFONY_VENDOR` to that fixture's absolute `vendor` directory,
`POOLSTER_SYMFONY_COMPOSER` to your Composer PHAR and `POOLSTER_GRAPHQL_JS_ROOT`
to a directory containing GraphQL.js 16.14.2. Set `POOLSTER_SYMFONY_RUNNER` to the
absolute path of `crates/plugins/symfony/tests/run-php.sh`, then run:

```sh
cargo test -p poolster-plugin-symfony symfony_container -- --ignored
```

A Docker runner can use `POOLSTER_GRAPHQL_SERVER_HOST=host.docker.internal` to
reach the test's local server. The default runner uses host PHP and localhost.

See the [verification record](../../verification-results/graphql-symfony-2026-10-10.json).
Transport configuration follows [Symfony HttpClient](https://symfony.com/doc/6.4/http_client.html).
