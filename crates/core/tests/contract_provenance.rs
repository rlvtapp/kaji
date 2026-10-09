use poolster_core::{blocks::ContractReference, engine::*, input::*};
use std::{path::Path, sync::Arc};
struct Value;
impl Contract for Value {
    const NAME: &'static str = "example.value.v1";
}
struct Derived;
struct Absent;
impl Contract for Absent {
    const NAME: &'static str = "example.absent.v1";
}
impl Contract for Derived {
    const NAME: &'static str = "example.derived.v1";
}
struct Language;
impl poolster_core::engine::Language for Language {
    const NAME: &'static str = "provenance";
    type Settings = ();
    type Workspace = ();
}
fn reference<C: Contract>() -> ContractReference {
    ContractReference::from_bytes(C::NAME, "stable.input", b"revision one")
}
fn input() -> InputContract {
    InputContract::new(InputSummary {
        format: "example".into(),
        title: "test".into(),
        version: None,
        types: vec![],
        operations: vec![],
    })
}
struct Loader;
impl InputPlugin for Loader {
    fn id(&self) -> &str {
        "example.loader"
    }
    fn format(&self) -> &str {
        "example"
    }
    fn load(&self, _: &Path) -> anyhow::Result<InputContract> {
        let mut input = input();
        input.publish_with_reference(Value, reference::<Value>())?;
        Ok(input)
    }
}
struct Transform(Meta);
impl Plugin<Language> for Transform {
    fn kind(&self) -> &'static str {
        "transform"
    }
    fn meta(&self) -> &Meta {
        &self.0
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on(None::<Handle<Value>>),
            Requirement::on(None::<Handle<Absent>>).optional(),
        ]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Derived>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
        assert_eq!(cx.inputs.reference::<Value>()?, Some(&reference::<Value>()));
        assert!(cx.inputs.reference::<Derived>().is_err());
        assert_eq!(cx.inputs.reference::<Absent>()?, None);
        assert!(
            cx.publish_with_reference(Derived, reference::<Value>())
                .is_err()
        );
        cx.publish_with_reference(Derived, reference::<Derived>())
    }
}
struct Output(Meta);
impl Plugin<Language> for Output {
    fn kind(&self) -> &'static str {
        "output"
    }
    fn meta(&self) -> &Meta {
        &self.0
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(None::<Handle<Derived>>)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
        assert_eq!(
            cx.inputs.reference::<Derived>()?,
            Some(&reference::<Derived>())
        );
        Ok(())
    }
}
#[test]
fn provenance_is_forwarded_from_input_through_graph_and_transform() {
    let mut registry = InputRegistry::new();
    registry.register(Loader).unwrap();
    let provider = InputProvider::<Value>::new(Arc::new(registry), "example", "unused");
    Packages::new()
        .package(
            Package::<Language>::new("sdk")
                .with(Output(Meta::new()))
                .with(Transform(Meta::new()))
                .with(provider),
        )
        .generate_native()
        .unwrap();
}
#[test]
fn opaque_contracts_remain_supported_and_failed_metadata_does_not_publish() {
    let mut input = input();
    assert!(
        input
            .publish_with_reference(Value, reference::<Derived>())
            .is_err()
    );
    assert!(input.get::<Value>().is_err());
    input.publish(Value).unwrap();
    assert_eq!(input.get_reference::<Value>().unwrap(), None);
    input.take::<Value>().unwrap();
    input
        .publish_with_reference(Value, reference::<Value>())
        .unwrap();
    assert_eq!(
        input.get_reference::<Value>().unwrap(),
        Some(&reference::<Value>())
    );
    assert!(
        input
            .publish_with_reference(
                Value,
                ContractReference::from_bytes(Value::NAME, "different", b"changed")
            )
            .is_err()
    );
    assert_eq!(
        input.get_reference::<Value>().unwrap(),
        Some(&reference::<Value>())
    );
    input.take::<Value>().unwrap();
    input.publish(Value).unwrap();
    assert_eq!(input.get_reference::<Value>().unwrap(), None);
}
