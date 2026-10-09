use poolster_core::{
    Api,
    engine::*,
    input::{InputContract, InputSummary},
};
struct Language;
impl poolster_core::engine::Language for Language {
    const NAME: &'static str = "identity-test";
    type Settings = ();
    type Workspace = ();
}
struct First;
struct Second;
impl Contract for First {
    const NAME: &'static str = "example.same.v1";
}
impl Contract for Second {
    const NAME: &'static str = "example.same.v1";
}
struct Provider {
    meta: Meta,
    second: bool,
}
impl Plugin<Language> for Provider {
    fn kind(&self) -> &'static str {
        "provider"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        if self.second {
            vec![Provision::of::<Second>()]
        } else {
            vec![Provision::of::<First>()]
        }
    }
    fn generate(&self, _: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
        panic!("incompatible graph must fail before execution")
    }
}
struct Optional {
    meta: Meta,
}
impl Plugin<Language> for Optional {
    fn kind(&self) -> &'static str {
        "optional"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(None::<Handle<Second>>).optional()]
    }
    fn generate(&self, _: &mut PluginContext<'_, Language>) -> anyhow::Result<()> {
        panic!("optional mismatch must fail before execution")
    }
}
#[test]
fn same_name_distinct_provider_types_fail_before_binding() {
    let error = Packages::new()
        .package(
            Package::<Language>::new("sdk")
                .with(Provider {
                    meta: Meta::new(),
                    second: false,
                })
                .with(Provider {
                    meta: Meta::new(),
                    second: true,
                }),
        )
        .generate(&Api::default(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("contract compatibility error for example.same.v1"));
}
#[test]
fn optional_handler_cannot_hide_a_stable_name_type_mismatch() {
    let error = Packages::new()
        .package(
            Package::<Language>::new("sdk")
                .with(Optional { meta: Meta::new() })
                .with(Provider {
                    meta: Meta::new(),
                    second: false,
                }),
        )
        .generate(&Api::default(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("different Rust types"));
}
#[test]
fn input_publication_and_access_report_identity_incompatibility() {
    let mut input = InputContract::new(InputSummary {
        format: "custom".into(),
        title: "identity".into(),
        version: None,
        types: vec![],
        operations: vec![],
    });
    input.publish(First).unwrap();
    assert!(
        input
            .publish(Second)
            .unwrap_err()
            .to_string()
            .contains("compatibility")
    );
    assert!(
        input
            .get::<Second>()
            .err()
            .unwrap()
            .to_string()
            .contains("compatibility")
    );
    assert!(
        input
            .take::<Second>()
            .err()
            .unwrap()
            .to_string()
            .contains("compatibility")
    );
    assert!(input.get::<First>().is_ok());
    input.take::<First>().unwrap();
    // Taking a payload does not change the contract identity of this input.
    assert!(
        input
            .publish(Second)
            .unwrap_err()
            .to_string()
            .contains("compatibility")
    );
    input.publish(First).unwrap();
}
