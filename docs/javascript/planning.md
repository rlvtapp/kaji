# Inspect plugins without generating

← [JavaScript](README.md)

```js
import { plan, formatPlan, loadConfig } from '@relevate/poolster';

const overview = plan(await loadConfig('./poolster.config.mjs'));
console.log(formatPlan(overview));
console.log(JSON.stringify(overview, null, 2));
```

Or call `createPoolster(config).plan()`. Planning is synchronous once the config
is loaded. It does not load input sources, execute callbacks or write output.
Errors loading an executable config still occur in `loadConfig`.

## Build your own viewer

Use the returned object directly. `plugins` contains local numeric IDs, phases,
callback names and declared requirements/publications. `edges` contains the
selected provider ID and consumer ID. The `stages` array describes lifecycle
boundaries. `formatPlan(overview)` is an optional text formatter.

```js
const connections = overview.edges.map(edge => ({
  from: edge.provider,
  to: edge.consumer,
  label: edge.contract ?? edge.dependency,
}));
```

A null provider denotes an unbound optional contract, or a name dependency on an
input boundary. Typed provider ambiguity, missing required contracts and graph
cycles throw the same planner errors used by JavaScript generation.

## Native boundary

Node planning exposes the JavaScript graph and configured native output nodes.
Those nodes have `detail: 'opaque-native-configuration'`: this API does not claim
to expose their Rust handlers, internal providers or full native validation.
JavaScript `transformApi`, `schema` and `operation` callbacks apply only to HTTP
inputs; their presence does not mean they will run for a native GraphQL source.

For the Rust graph of a JSON CLI recipe, use [CLI planning](../cli/plan.md).
Rust and Node have separate versioned planning envelopes because they expose
different runtime boundaries. Neither contains execution traces or generated
files. Native contract value serialization is separate from planning.
