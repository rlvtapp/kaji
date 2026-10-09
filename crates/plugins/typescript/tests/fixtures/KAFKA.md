# Kafka runtime test

The generated SDK pins KafkaJS 2.2.4 and Ajv 8.17.1; the compile test uses TypeScript 5.9.3 and @types/node 22.19.15. The local integration broker is Kafka-compatible Redpanda 25.3.11, pinned by digest in `kafka-broker.compose.yaml`. Version and image digest were checked against the actual pulled image; the fixture runs a real broker, without transport mocks.

From the repository root:

```sh
docker compose -f crates/plugins/typescript/tests/fixtures/kafka-broker.compose.yaml up -d
mkdir -p /tmp/poolster-kafka-node
npm install --prefix /tmp/poolster-kafka-node --ignore-scripts --no-audit --no-fund kafkajs@2.2.4 ajv@8.17.1 typescript@5.9.3 @types/node@22.19.15
POOLSTER_KAFKA_NODE_MODULES=/tmp/poolster-kafka-node/node_modules \
POOLSTER_KAFKA_BROKER=127.0.0.1:29092 \
cargo test -p poolster-plugin-typescript --test asyncapi_native -- --include-ignored
docker compose -f crates/plugins/typescript/tests/fixtures/kafka-broker.compose.yaml down
```

Use an isolated test broker: the probe recreates topic `poolster-orders`. It tests generated compilation, producer/consumer delivery, keys, string headers, invalid incoming/outgoing payloads, and fixed consumer group bindings. No credentials, registry or external services are used.

The optional `onInvalidMessage` callback acknowledges an invalid message after the callback completes. Omitting it throws to KafkaJS, leaving failure/retry handling to the consumer. Handler errors propagate to KafkaJS. Generated consumers expose their KafkaJS handle; `KafkaClient.disconnect()` closes all connections it created.

AsyncAPI 3.0/3.1 generation supports static Kafka channels, one JSON message per operation, local acyclic schema references, JSON primitive/object/array schemas, required properties, explicit null fields, validation constraints, string keys and headers, and fixed group/client binding IDs. Other features reject explicitly. AsyncAPI 2.6 and broader protocols retain existing inspection support.

Official references: [KafkaJS producing](https://kafka.js.org/docs/producing), [KafkaJS consuming](https://kafka.js.org/docs/consuming), [AsyncAPI Kafka bindings](https://www.asyncapi.com/docs/reference/bindings/kafka), [Redpanda 25.3 quickstart](https://docs.redpanda.com/streaming/25.3/get-started/quick-start/).
