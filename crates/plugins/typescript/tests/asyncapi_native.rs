use poolster_core::{
    engine::Packages,
    input::{InputOptions, InputProvider, InputRegistry},
    native::events::EventOperations,
};
use poolster_plugin_typescript as ts;
use std::{process::Command, sync::Arc};
fn generate(path: &std::path::Path) -> poolster_core::GeneratedTree {
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    let input = InputProvider::<EventOperations>::new(Arc::new(registry), "asyncapi", path)
        .with_options(InputOptions::default());
    let generator = ts::asyncapi(Some(input.handle()));
    Packages::new()
        .package(ts::package("client").with(generator).with(input))
        .generate_native()
        .unwrap()
}
#[test]
fn kafka_models_and_functions_regenerate_with_native_document() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let first = generate(&path);
    let second = generate(&path);
    assert_eq!(
        first.get("client/events.ts"),
        second.get("client/events.ts")
    );
    let source = first.get("client/events.ts").unwrap();
    assert!(source.contains("export function sendOrder"));
    assert!(source.contains("export function receiveOrder"));
    assert!(source.contains("\"quantity\": number"));
    assert!(source.contains("\"note\"?: null"));
    assert!(
        first
            .get("client/asyncapi.json")
            .unwrap()
            .contains("bindingVersion")
    );
}
#[test]
#[ignore = "requires Node, POOLSTER_KAFKA_NODE_MODULES and POOLSTER_KAFKA_BROKER local broker"]
fn generated_kafka_package_compiles_and_roundtrips_real_broker() {
    let modules =
        std::env::var("POOLSTER_KAFKA_NODE_MODULES").expect("POOLSTER_KAFKA_NODE_MODULES");
    let broker = std::env::var("POOLSTER_KAFKA_BROKER").expect("POOLSTER_KAFKA_BROKER");
    let directory = tempfile::tempdir().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    generate(&path).write_to(directory.path()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&modules, directory.path().join("client/node_modules")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&modules, directory.path().join("client/node_modules"))
        .unwrap();
    std::fs::write(directory.path().join("client/check.ts"),r#"import { createKafkaClient, sendOrder, receiveOrder } from './index.js';
const client = createKafkaClient();
void sendOrder(client,{payload:{id:'1',quantity:2},headers:{trace:'t'}});
// @ts-expect-error integer payload field cannot be string
void sendOrder(client,{payload:{id:'1',quantity:'2'}});
void receiveOrder(client,message => {const quantity: number = message.payload.quantity; console.log(quantity);});
"#).unwrap();
    let compiled = Command::new("node")
        .arg(std::path::Path::new(&modules).join("typescript/lib/tsc.js"))
        .args(["-p", "tsconfig.json"])
        .current_dir(directory.path().join("client"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(
        directory.path().join("client/probe.cjs"),
        include_str!("fixtures/kafka_probe.cjs"),
    )
    .unwrap();
    let runtime = Command::new("node")
        .arg("probe.cjs")
        .env("POOLSTER_KAFKA_BROKER", broker)
        .current_dir(directory.path().join("client"))
        .output()
        .unwrap();
    assert!(
        runtime.status.success(),
        "{}{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
fn kafka_output_accepts_replacement_input_provider() {
    struct Replacement;
    impl poolster_core::input::InputPlugin for Replacement {
        fn id(&self) -> &str {
            "asyncapi.replacement"
        }
        fn format(&self) -> &str {
            "asyncapi"
        }
        fn load(
            &self,
            path: &std::path::Path,
        ) -> anyhow::Result<poolster_core::input::InputContract> {
            let doc = poolster_input_asyncapi::parse(&std::fs::read_to_string(path)?)?;
            let mut events =
                poolster_input_asyncapi::lower_operations(&doc, &InputOptions::default())?;
            events
                .operations
                .iter_mut()
                .for_each(|op| op.name = format!("custom_{}", op.name));
            let mut input = poolster_core::input::InputContract::new(doc.summary());
            input.publish(events)?;
            Ok(input)
        }
    }
    let mut registry = InputRegistry::new();
    registry.register(Replacement).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let input = InputProvider::<EventOperations>::new(Arc::new(registry), "asyncapi", path)
        .using("asyncapi.replacement");
    let generator = ts::asyncapi(Some(input.handle()));
    let tree = Packages::new()
        .package(ts::package("client").with(generator).with(input))
        .generate_native()
        .unwrap();
    assert!(
        tree.get("client/events.ts")
            .unwrap()
            .contains("export function custom_sendOrder")
    );
}

#[test]
fn kafka_output_publishes_actual_symbols_for_post_generation_plugins() {
    use poolster_core::engine::{Enforce, Handle, Meta, Plugin, PluginContext, Requirement};
    struct Post {
        meta: Meta,
        provider: Handle<ts::KafkaClient>,
    }
    impl Plugin<ts::TypeScript> for Post {
        fn kind(&self) -> &'static str {
            "kafka-post"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn enforce(&self) -> Enforce {
            Enforce::Post
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![Requirement::on(Some(self.provider))]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let contract = cx.inputs.get::<ts::KafkaClient>()?;
            let op = &contract.operations["sendOrder"];
            let import = op.function.import_from("hooks/send.ts")?;
            cx.files.emit(poolster_core::GeneratedFile::new(
                "hooks/send.ts",
                format!(
                    "export {{ {} }} from '{}.js';\nexport type {{ {} }} from '{}.js';\n",
                    op.function.name, import, op.payload.name, import
                ),
            )?)
        }
    }
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let input = InputProvider::<EventOperations>::new(Arc::new(registry), "asyncapi", path);
    let generator = ts::asyncapi(Some(input.handle()));
    let post = Post {
        meta: Meta::new(),
        provider: generator.handle(),
    };
    let tree = Packages::new()
        .package(ts::package("client").with(post).with(generator).with(input))
        .generate_native()
        .unwrap();
    assert!(
        tree.get("client/hooks/send.ts")
            .unwrap()
            .contains("sendOrderPayload")
    );
    assert!(
        tree.get("client/hooks/send.ts")
            .unwrap()
            .contains("../events.js")
    );
}

#[test]
fn explicit_message_blocks_compose_without_replacing_whole_contract() {
    use poolster_core::{
        blocks::Blocks,
        engine::{Handle, Meta, Plugin, PluginContext, Requirement},
        native::events::EventMessage,
    };
    struct Inspect {
        meta: Meta,
        whole: Handle<EventOperations>,
        messages: Handle<Blocks<EventMessage>>,
    }
    impl Plugin<ts::TypeScript> for Inspect {
        fn kind(&self) -> &'static str {
            "event-block-inspector"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![
                Requirement::on(Some(self.whole)),
                Requirement::on(Some(self.messages)),
            ]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let whole = cx.inputs.get::<EventOperations>()?;
            let blocks = cx.inputs.get::<Blocks<EventMessage>>()?;
            assert_eq!(whole.operations.len(), 2);
            assert_eq!(blocks.items.len(), 1);
            assert_eq!(blocks.with_capability("broker.kafka").count(), 1);
            let message = &blocks.items[0];
            assert_eq!(
                message.metadata.id.local,
                "#/channels/orders/messages/created"
            );
            assert_eq!(message.metadata.id.source, "example:orders");
            assert_eq!(
                message.value.payload_schema["properties"]["quantity"]["minimum"],
                1
            );
            assert!(whole.source["channels"]["orders"]["bindings"].is_object());
            cx.files.emit(poolster_core::GeneratedFile::new(
                "message-manifest.json",
                serde_json::to_string_pretty(blocks)?,
            )?)
        }
    }
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let input = InputProvider::<EventOperations>::new(Arc::new(registry), "asyncapi", path);
    let expose = poolster_input_asyncapi::event_messages(Some(input.handle()), "example:orders");
    let inspect = Inspect {
        meta: Meta::new(),
        whole: input.handle(),
        messages: expose.handle(),
    };
    let generator = ts::asyncapi(Some(input.handle()));
    let tree = Packages::new()
        .package(
            ts::package("client")
                .with(inspect)
                .with(expose)
                .with(generator)
                .with(input),
        )
        .generate_native()
        .unwrap();
    assert!(
        tree.get("client/message-manifest.json")
            .unwrap()
            .contains("messaging.headers")
    );
    assert!(tree.get("client/events.ts").unwrap().contains("sendOrder"));
}
