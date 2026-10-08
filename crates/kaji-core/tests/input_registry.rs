use anyhow::{Result, bail};
use kaji_core::{engine::Contract, input::*};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug, PartialEq)]
struct Text(String);
impl Contract for Text {
    const NAME: &'static str = "test.text";
}
#[derive(Debug, PartialEq)]
struct Count(usize);
impl Contract for Count {
    const NAME: &'static str = "test.count";
}
fn summary(format: &str) -> InputSummary {
    InputSummary {
        format: format.into(),
        title: "Native ✓".into(),
        version: Some("1.0".into()),
        types: vec!["Example".into()],
        operations: vec![InputOperation {
            name: "read".into(),
            kind: "query".into(),
        }],
    }
}
struct Probe {
    id: String,
    format: String,
    calls: Arc<AtomicUsize>,
    fail: bool,
}
impl InputPlugin for Probe {
    fn id(&self) -> &str {
        &self.id
    }
    fn format(&self) -> &str {
        &self.format
    }
    fn load(&self, path: &Path) -> Result<InputContract> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            bail!("native parser rejected line 7")
        }
        let mut input = InputContract::new(summary(&self.format));
        input.publish(Text(path.to_string_lossy().into_owned()))?;
        input.diagnostics.push(InputDiagnostic {
            code: "retained".into(),
            message: "source retained".into(),
        });
        Ok(input)
    }
}
fn probe(id: &str, format: &str, calls: &Arc<AtomicUsize>) -> Probe {
    Probe {
        id: id.into(),
        format: format.into(),
        calls: calls.clone(),
        fail: false,
    }
}

#[test]
fn multiple_native_capabilities_can_be_read_and_taken_independently() {
    let mut input = InputContract::new(summary("custom"));
    input.publish(Text("payload".into())).unwrap();
    input.publish(Count(3)).unwrap();
    assert_eq!(input.get::<Count>().unwrap(), &Count(3));
    assert_eq!(input.take::<Text>().unwrap(), Text("payload".into()));
    assert!(
        input
            .get::<Text>()
            .unwrap_err()
            .to_string()
            .contains("test.text")
    );
    assert_eq!(input.take::<Count>().unwrap(), Count(3));
    assert!(input.take::<Count>().is_err());
    input
        .publish(Text("replacement after take".into()))
        .unwrap();
    assert_eq!(input.get::<Text>().unwrap().0, "replacement after take");
}
#[test]
fn missing_take_does_not_remove_another_capability() {
    let mut input = InputContract::new(summary("custom"));
    input.publish(Text("unchanged".into())).unwrap();
    assert!(input.take::<Count>().is_err());
    assert_eq!(input.get::<Text>().unwrap().0, "unchanged");
}
#[test]
fn listing_is_sorted_and_has_no_parser_side_effects() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = InputRegistry::new();
    for id in ["z.parser", "a.parser", "m.parser"] {
        registry.register(probe(id, "custom", &calls)).unwrap();
    }
    assert_eq!(
        registry
            .plugins()
            .iter()
            .map(|p| p.provider.as_str())
            .collect::<Vec<_>>(),
        ["a.parser", "m.parser", "z.parser"]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn selection_errors_never_invoke_a_parser() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = InputRegistry::new();
    registry
        .register(probe("a.parser", "custom", &calls))
        .unwrap();
    registry
        .register(probe("b.parser", "custom", &calls))
        .unwrap();
    for (format, provider) in [
        ("custom", None),
        ("missing", None),
        ("custom", Some("missing")),
        ("other", Some("a.parser")),
    ] {
        assert!(
            registry
                .load(format, provider, Path::new("unused"))
                .is_err()
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn explicit_selection_invokes_only_the_chosen_provider_and_retains_metadata() {
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let mut registry = InputRegistry::new();
    registry
        .register(probe("a.parser", "custom", &first))
        .unwrap();
    registry
        .register(probe("b.parser", "custom", &second))
        .unwrap();
    let path = Path::new("dir with spaces/契約.custom");
    let loaded = registry.load("custom", Some("b.parser"), path).unwrap();
    assert_eq!(loaded.source, path);
    assert_eq!(loaded.provider, "b.parser");
    assert_eq!(loaded.contract.summary, summary("custom"));
    assert_eq!(loaded.contract.diagnostics[0].code, "retained");
    assert_eq!(first.load(Ordering::SeqCst), 0);
    assert_eq!(second.load(Ordering::SeqCst), 1);
}
#[test]
fn failed_registration_keeps_the_original_provider() {
    let calls = Arc::new(AtomicUsize::new(0));
    let replacement = Arc::new(AtomicUsize::new(0));
    let mut registry = InputRegistry::new();
    registry
        .register(probe("a.parser", "custom", &calls))
        .unwrap();
    assert!(
        registry
            .register(probe("a.parser", "other", &replacement))
            .is_err()
    );
    registry.load("custom", None, Path::new("unused")).unwrap();
    assert_eq!(registry.plugins().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(replacement.load(Ordering::SeqCst), 0);
}
#[test]
fn invalid_provider_and_format_identifiers_are_rejected_without_registration() {
    let calls = Arc::new(AtomicUsize::new(0));
    for invalid in [
        "",
        "A",
        "9parser",
        " space",
        "a/b",
        "a:b",
        "a\n",
        "é.parser",
        "a🚀",
    ] {
        let mut registry = InputRegistry::new();
        assert!(
            registry.register(probe(invalid, "valid", &calls)).is_err(),
            "id {invalid:?}"
        );
        assert!(
            registry
                .register(probe("valid.parser", invalid, &calls))
                .is_err(),
            "format {invalid:?}"
        );
        assert!(registry.plugins().is_empty());
    }
    for valid in ["a", "a9", "a.b-c_d"] {
        InputRegistry::new()
            .register(probe(valid, valid, &calls))
            .unwrap();
    }
}
#[test]
fn loader_error_preserves_cause_provider_and_source() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut plugin = probe("custom.bad", "custom", &calls);
    plugin.fail = true;
    let mut registry = InputRegistry::new();
    registry.register(plugin).unwrap();
    let error = registry
        .load("custom", None, Path::new("broken.contract"))
        .err()
        .unwrap();
    let chain = format!("{error:#}");
    for expected in ["custom.bad", "broken.contract", "line 7"] {
        assert!(chain.contains(expected), "{chain}");
    }
}
#[test]
fn summary_and_diagnostics_roundtrip_json_without_losing_unicode_or_kinds() {
    let expected = summary("community.format-v2");
    let encoded = serde_json::to_string(&expected).unwrap();
    assert_eq!(
        serde_json::from_str::<InputSummary>(&encoded).unwrap(),
        expected
    );
    let diagnostic = InputDiagnostic {
        code: "unresolved".into(),
        message: "契約 unavailable".into(),
    };
    assert_eq!(
        serde_json::from_str::<InputDiagnostic>(&serde_json::to_string(&diagnostic).unwrap())
            .unwrap(),
        diagnostic
    );
}
#[test]
fn empty_registry_has_actionable_missing_provider_error() {
    let registry = InputRegistry::default();
    assert!(registry.plugins().is_empty());
    let error = registry
        .load("community", None, Path::new("unused"))
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("no input provider registered for format \"community\"")
    );
}
