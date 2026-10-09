# poolster-input-asyncapi

Register `AsyncApiInput` with `InputRegistry`, or enable `asyncapi` in
`poolster-inputs`. AsyncAPI 2.6, 3.0 and 3.1 retain native inspection support.

The public plugin-author surfaces are:

- `contracts`: `EventOperations`, `EventOperation`, `EventMessage`, `EventAction`,
  `KafkaBroker` and `MessageBlocks`. Reexports preserve existing typed graph
  identities; output contracts contain no parser-library types.
- `blocks`: the `MessageBlocks` collection, metadata/ID types,
  `lower_message_blocks(document, source)` extractor and optional
  `event_messages(provider, source)` transformer.
- `AsyncApiDocument`: the authoritative native source and parser model, kept
  separately for inspection and details outside the owned contracts.

The provider publishes useful `MessageBlocks` directly without broker
configuration. IDs use source identities and resolved native JSON pointers;
messages shared by operations deduplicate. Unsupported schemas get per-message
`asyncapi-message-block-unsupported` diagnostics while other messages and the
native document remain available. Capability tags describe blocks; exact typed
requirements select contracts.

```rust
use poolster_core::input::InputProvider;
use poolster_input_asyncapi::contracts::MessageBlocks;
let input = InputProvider::<MessageBlocks>::new(registry, "asyncapi", "events.yaml");
let models = input.handle(); // A consumer requires this typed collection.
```

When executable Kafka routing is supported, the provider also publishes
`EventOperations`. Select it explicitly for the TypeScript Kafka generator:

```rust
use poolster_core::input::{InputOptions, InputProvider};
use poolster_input_asyncapi::contracts::EventOperations;
let input = InputProvider::<EventOperations>::new(registry, "asyncapi", "events.yaml")
    .with_options(InputOptions {
        broker: Some(serde_json::json!({
            "kind": "kafka", "brokers": ["127.0.0.1:29092"], "client_id": "orders"
        })),
        ..Default::default()
    });
let generator = poolster_plugin_typescript::asyncapi(Some(input.handle()));
```

The optional `blocks::event_messages(Some(input.handle()), "example:orders")`
transformer exposes blocks from a composed whole-contract provider. It does not
replace that provider or impose decomposition on opaque contracts.

Generation supports AsyncAPI 3.0/3.1 static plaintext Kafka topics, one JSON message
per operation, local acyclic schema references, primitive/object/array schemas,
required/optional and explicit null-valued fields, string keys/headers, validation
constraints, and fixed singleton group/client binding IDs. KafkaJS 2.2.4 and Ajv
8.17.1 implement and validate both wire directions. `ts::KafkaClient` publishes
actual emitted symbols for downstream and post-generation plugins.

Generation rejects security, schema registries, dynamic topics, replies, traits,
multiple messages, recursive/composed schemas, unsupported dialects/keywords and
bindings, and unsupported TypeScript operation names. AsyncAPI 2.6 model blocks do
not imply executable publish/subscribe support. The pinned ADEO upstream document
remains inspectable but rejects generation for unsupported features. Default loads
report unavailable generation as a diagnostic; explicit broker options return
feature errors. Broader compatibility is not claimed.

[Reproducible real broker tests](../../plugins/typescript/tests/fixtures/KAFKA.md)
cover compilation, producer/consumer behavior and invalid incoming/outgoing data.
