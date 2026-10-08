use anyhow::Result;
use poolster_core::{GeneratedFile, GeneratedTree, engine::*, input::*};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
struct Native(String);
impl Contract for Native {
    const NAME: &'static str = "test.native.v1";
}
struct TestLanguage;
impl Language for TestLanguage {
    const NAME: &'static str = "test";
    type Settings = ();
    type Workspace = ();
    fn finalize(_: &mut FinalizeContext<'_, Self>) -> Result<()> {
        Ok(())
    }
}
struct Loader(Arc<AtomicUsize>);
impl InputPlugin for Loader {
    fn id(&self) -> &str {
        "test.input"
    }
    fn format(&self) -> &str {
        "test"
    }
    fn load(&self, path: &std::path::Path) -> Result<InputContract> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let text = std::fs::read_to_string(path)?;
        let mut c = InputContract::new(InputSummary {
            format: "test".into(),
            title: "Test".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        c.publish(Native(text))?;
        Ok(c)
    }
}
struct Consumer {
    meta: Meta,
    native: bool,
}
impl Consumer {
    fn new(native: bool) -> Self {
        Self {
            meta: Meta::new(),
            native,
        }
    }
}
impl Plugin<TestLanguage> for Consumer {
    fn kind(&self) -> &'static str {
        "consumer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        self.native
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(None::<Handle<Native>>)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        cx.files.emit(GeneratedFile::new(
            "native.txt",
            &cx.inputs.get::<Native>()?.0,
        )?)
    }
}
fn registry(runs: Arc<AtomicUsize>) -> Arc<InputRegistry> {
    let mut registry = InputRegistry::new();
    registry.register(Loader(runs)).unwrap();
    Arc::new(registry)
}
#[test]
fn http_package_is_structurally_skipped_without_loading_missing_source() {
    let runs = Arc::new(AtomicUsize::new(0));
    let input = InputProvider::<Native>::new(
        registry(runs.clone()),
        "test",
        "/this-source-does-not-exist.graphql",
    );
    let report = Packages::new()
        .package(
            Package::<TestLanguage>::new("http-sdk")
                .with(input)
                .with(Consumer::new(false)),
        )
        .generate_native_report()
        .unwrap();
    assert_eq!(runs.load(Ordering::Relaxed), 0);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].package, "http-sdk");
    assert!(report.skipped[0].reason.contains("consumer"));
    assert!(report.tree.get("http-sdk/native.txt").is_none());
}
#[test]
fn missing_required_native_provider_errors_before_any_loader_runs() {
    let runs = Arc::new(AtomicUsize::new(0));
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "ready").unwrap();
    let input = InputProvider::<Native>::new(registry(runs.clone()), "test", file.path());
    let error = Packages::new()
        .package(
            Package::<TestLanguage>::new("valid")
                .with(input)
                .with(Consumer::new(true)),
        )
        .package(Package::<TestLanguage>::new("missing").with(Consumer::new(true)))
        .generate_native_report()
        .unwrap_err();
    assert_eq!(runs.load(Ordering::Relaxed), 0);
    assert!(format!("{error:#}").contains("test.native.v1"));
}
#[test]
fn mixed_generation_runs_native_packages_and_preserves_skipped_owned_files() {
    let runs = Arc::new(AtomicUsize::new(0));
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.native");
    std::fs::write(&source, "new").unwrap();
    let mut prior = GeneratedTree::default();
    prior
        .insert(GeneratedFile::new("http/old.ts", "original").unwrap())
        .unwrap();
    prior.write_to(root.path()).unwrap();
    std::fs::write(root.path().join("http/old.ts"), "local edit").unwrap();
    let mut report = Packages::new()
        .package(
            Package::<TestLanguage>::new("http")
                .with(InputProvider::<Native>::new(
                    registry(runs.clone()),
                    "test",
                    "/missing.native",
                ))
                .with(Consumer::new(false)),
        )
        .package(
            Package::<TestLanguage>::new("native")
                .with(InputProvider::<Native>::new(
                    registry(runs.clone()),
                    "test",
                    source,
                ))
                .with(Consumer::new(true)),
        )
        .generate_native_report()
        .unwrap();
    assert_eq!(runs.load(Ordering::Relaxed), 1);
    assert_eq!(report.tree.get("native/native.txt"), Some("new"));
    for skip in &report.skipped {
        report
            .tree
            .preserve_owned_prefix(root.path(), &skip.package)
            .unwrap();
    }
    report.tree.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("http/old.ts")).unwrap(),
        "local edit"
    );
    // Retention preserves old ownership hashes; subsequent generation still detects local drift.
    let mut regenerated = GeneratedTree::default();
    regenerated
        .insert(GeneratedFile::new("http/old.ts", "original").unwrap())
        .unwrap();
    assert!(regenerated.write_to(root.path()).is_err());
}
#[test]
fn skipped_prefixes_cannot_hide_active_files_or_escape_root() {
    let root = tempfile::tempdir().unwrap();
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("sdk/active.ts", "active").unwrap())
        .unwrap();
    assert!(tree.preserve_owned_prefix(root.path(), "sdk").is_err());
    assert!(tree.preserve_owned_prefix(root.path(), ".").is_err());
    assert!(
        tree.preserve_owned_prefix(root.path(), "../outside")
            .is_err()
    );
}

struct Events(String);
impl Contract for Events {
    const NAME: &'static str = "test.events.v1";
}
struct Combined(String);
impl Contract for Combined {
    const NAME: &'static str = "test.combined.v1";
}
struct EventsLoader;
impl InputPlugin for EventsLoader {
    fn id(&self) -> &str {
        "events.test"
    }
    fn format(&self) -> &str {
        "events"
    }
    fn load(&self, path: &std::path::Path) -> Result<InputContract> {
        let mut c = InputContract::new(InputSummary {
            format: "events".into(),
            title: "Events".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        c.publish(Events(std::fs::read_to_string(path)?))?;
        Ok(c)
    }
}
struct Combine {
    meta: Meta,
    source: Handle<Native>,
    events: Handle<Events>,
}
impl Plugin<TestLanguage> for Combine {
    fn kind(&self) -> &'static str {
        "combine-inputs"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![
            Requirement::on(Some(self.source)),
            Requirement::on(Some(self.events)),
        ]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Combined>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        cx.publish(Combined(format!(
            "{} + {}",
            cx.inputs.get::<Native>()?.0,
            cx.inputs.get::<Events>()?.0
        )))
    }
}
struct CombinedConsumer {
    meta: Meta,
    source: Handle<Combined>,
}
impl Plugin<TestLanguage> for CombinedConsumer {
    fn kind(&self) -> &'static str {
        "combined-output"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        cx.files.emit(GeneratedFile::new(
            "mixed.txt",
            &cx.inputs.get::<Combined>()?.0,
        )?)
    }
}
#[test]
fn heterogeneous_inputs_compose_through_typed_intermediate_before_output() {
    let root = tempfile::tempdir().unwrap();
    let api = root.path().join("api.native");
    let events = root.path().join("events.native");
    std::fs::write(&api, "operations").unwrap();
    std::fs::write(&events, "messages").unwrap();
    let runs = Arc::new(AtomicUsize::new(0));
    let mut registry = InputRegistry::new();
    registry.register(Loader(runs.clone())).unwrap();
    registry.register(EventsLoader).unwrap();
    let registry = Arc::new(registry);
    let api = InputProvider::<Native>::new(registry.clone(), "test", api);
    let events = InputProvider::<Events>::new(registry, "events", events);
    let combine = Combine {
        meta: Meta::new(),
        source: api.handle(),
        events: events.handle(),
    };
    let output = CombinedConsumer {
        meta: Meta::new(),
        source: combine.meta.handle(),
    };
    // Deliberately reverse dependency order to require graph scheduling.
    let report = Packages::new()
        .package(
            Package::<TestLanguage>::new("mixed")
                .with(output)
                .with(combine)
                .with(events)
                .with(api),
        )
        .generate_native_report()
        .unwrap();
    assert!(report.skipped.is_empty());
    assert_eq!(
        report.tree.get("mixed/mixed.txt"),
        Some("operations + messages")
    );
    assert_eq!(runs.load(Ordering::Relaxed), 1);
}
struct TransformNative {
    meta: Meta,
    source: Handle<Native>,
}
impl Plugin<TestLanguage> for TransformNative {
    fn kind(&self) -> &'static str {
        "transform-native"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Native>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        cx.publish(Native(format!(
            "{} with hook",
            cx.inputs.get::<Native>()?.0
        )))
    }
}
struct ExplicitNativeConsumer {
    meta: Meta,
    source: Handle<Native>,
}
impl Plugin<TestLanguage> for ExplicitNativeConsumer {
    fn kind(&self) -> &'static str {
        "explicit-native-output"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        cx.files.emit(GeneratedFile::new(
            "value.txt",
            &cx.inputs.get::<Native>()?.0,
        )?)
    }
}
#[test]
fn same_contract_transform_uses_explicit_handles_without_source_ambiguity() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "source").unwrap();
    let input = InputProvider::<Native>::new(registry(Default::default()), "test", file.path());
    let hook = TransformNative {
        meta: Meta::new(),
        source: input.handle(),
    };
    let output = ExplicitNativeConsumer {
        meta: Meta::new(),
        source: hook.meta.handle(),
    };
    let report = Packages::new()
        .package(
            Package::<TestLanguage>::new("sdk")
                .with(output)
                .with(hook)
                .with(input),
        )
        .generate_native_report()
        .unwrap();
    assert_eq!(report.tree.get("sdk/value.txt"), Some("source with hook"));
}
