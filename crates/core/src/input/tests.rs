use super::*;
struct Native(String);
impl Contract for Native {
    const NAME: &'static str = "example.native";
}
#[derive(Debug)]
struct Missing;
impl Contract for Missing {
    const NAME: &'static str = "example.missing";
}
struct Provider(&'static str);
impl InputPlugin for Provider {
    fn id(&self) -> &str {
        self.0
    }
    fn format(&self) -> &str {
        "custom"
    }
    fn load(&self, path: &Path) -> Result<InputContract> {
        let mut input = InputContract::new(InputSummary {
            format: "custom".into(),
            title: "Custom".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        input.publish(Native(std::fs::read_to_string(path)?))?;
        Ok(input)
    }
}

#[test]
fn community_provider_publishes_native_contract_without_core_enum_changes() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "native contract").unwrap();
    let mut registry = InputRegistry::new();
    registry.register(Provider("custom.parser")).unwrap();
    let input = registry.load("custom", None, file.path()).unwrap();
    assert_eq!(input.contract.get::<Native>().unwrap().0, "native contract");
    assert_eq!(input.provider, "custom.parser");
    assert!(
        input
            .contract
            .get::<Missing>()
            .unwrap_err()
            .to_string()
            .contains("example.missing")
    );
}

#[test]
fn replacements_need_explicit_selection_and_duplicate_ids_fail() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut registry = InputRegistry::new();
    registry.register(Provider("custom.first")).unwrap();
    registry.register(Provider("custom.second")).unwrap();
    assert!(
        registry
            .load("custom", None, file.path())
            .err()
            .unwrap()
            .to_string()
            .contains("multiple input providers")
    );
    assert!(
        registry
            .load("custom", Some("custom.second"), file.path())
            .is_ok()
    );
    assert!(registry.register(Provider("custom.first")).is_err());
    assert!(
        registry
            .load("other", Some("custom.first"), file.path())
            .is_err()
    );
    assert!(registry.load("other", None, file.path()).is_err());
    assert!(
        registry
            .load("custom", Some("unknown"), file.path())
            .is_err()
    );
}

#[test]
fn registry_rejects_invalid_ids_and_false_format_claims() {
    struct Wrong;
    impl InputPlugin for Wrong {
        fn id(&self) -> &str {
            "custom.wrong"
        }
        fn format(&self) -> &str {
            "custom"
        }
        fn load(&self, _: &Path) -> Result<InputContract> {
            Ok(InputContract::new(InputSummary {
                format: "other".into(),
                title: String::new(),
                version: None,
                types: vec![],
                operations: vec![],
            }))
        }
    }
    let mut registry = InputRegistry::new();
    assert!(registry.register(Provider("../invalid")).is_err());
    registry.register(Wrong).unwrap();
    assert!(
        registry
            .load("custom", None, Path::new("unused"))
            .err()
            .unwrap()
            .to_string()
            .contains("published format")
    );
}

#[test]
fn duplicate_publications_preserve_first_contract() {
    let mut input = InputContract::new(InputSummary {
        format: "custom".into(),
        title: "Custom".into(),
        version: None,
        types: vec![],
        operations: vec![],
    });
    input.publish(Native("first".into())).unwrap();
    assert!(input.publish(Native("second".into())).is_err());
    assert_eq!(input.get::<Native>().unwrap().0, "first");
}
