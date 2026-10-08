# Execute collection structure against a local mock

Opt-in Newman execution for generated collections. `npm ci --ignore-scripts`, then
`node run.mjs path/to/collection.json`. The pinned Newman dependency executes one
iteration against an ephemeral loopback HTTP mock, never the collection's server.
The runner rejects existing scripts, replaces every request URL and auth helper,
and adds its own status assertion using the first saved successful response.
Requests, bodies and timeouts are bounded. No environment credentials are needed.

This verifies that Newman can execute the exported methods/bodies/query structure
and saved status fixtures. It deliberately substitutes paths and authentication;
it does not validate source URL/path encoding, actual API authentication, semantic
response correctness, CRUD sequencing or streaming. Unit tests cover generator
mapping details separately. To exercise a real sandbox, author and review a
separate test workflow rather than treating this mock check as acceptance coverage.

`npm test` verifies execution and rejection behavior. Newman options follow the
[official command reference](https://learning.postman.com/docs/collections/using-newman-cli/newman-options).
