# GraphQL → PHP clients

Unreleased. PHP 8.2+ clients use Composer autoload and immutable models for the selections in your operation documents.

```sh
poolster generate schema.graphql --input-format graphql \
  --operation operations.graphql --language php --output generated
```

Use the existing npm `pluginPhp({ contracts: { graphql: { style: 'flat' } } })`,
or Rust `php::graphql(Some(input.handle())).flat()` inside a PHP package.

## Styles

For `query ReadUser`, raw exposes a namespaced `readUser($client, $variables)` function.
Flat exposes `$client->readUser($variables)`. Grouped exposes
`$client->query()->readUser($variables)` and mutation accessors.
Configure `groups: { users: { read: 'ReadUser' } }` to call `$client->users()->read($variables)`.
Namespace follows the Composer package name. Construct the generated variables model for the selected operation.

## Results and transport

`GraphqlResponse` preserves data, errors and extensions. Inspect partial data before
calling `requireData()`, which rejects GraphQL application errors. Transport failures
and malformed responses fail separately. PHP HTTP streams are the default transport;
an injectable callable can adapt PSR-18 or Symfony HttpClient. A dedicated Symfony
GraphQL dependency-injection generator is not implemented.

`Presence::missing()` differs from `Presence::of(null)`. Selected nested objects,
lists and input models retain nullability; abstract variants require a selected
`__typename` discriminator. Custom scalars retain JSON values without mapping codecs.
Subscriptions, incremental delivery and dynamic selections are unsupported.

All four styles compile and execute against pinned GraphQL.js 16.14.2.
See the [support matrix](../../plugin-support-matrix.md) for the verification boundary.
