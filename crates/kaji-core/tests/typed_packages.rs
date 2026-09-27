use anyhow::Result;
use kaji_core::engine::*;
use kaji_core::{Api, GeneratedFile, SdkClientStyle};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct TestLanguage;
impl Language for TestLanguage {
    const NAME: &'static str = "test";
    type Settings = ();
    type Workspace = Vec<String>;
    fn finalize(cx: &mut FinalizeContext<'_, Self>) -> Result<()> {
        cx.files
            .emit(GeneratedFile::new("order.txt", cx.workspace.join("\n"))?)
    }
}
struct Text(String);
impl Contract for Text {
    const NAME: &'static str = "Text";
}
struct Other;
impl Contract for Other {
    const NAME: &'static str = "Other";
}

struct Producer {
    meta: Meta,
    text: String,
    runs: Arc<AtomicUsize>,
}
impl Producer {
    fn new(text: &str) -> Self {
        Self {
            meta: Meta::new().label(text),
            text: text.into(),
            runs: Default::default(),
        }
    }
    fn handle(&self) -> Handle<Text> {
        self.meta.handle()
    }
}
impl Plugin<TestLanguage> for Producer {
    fn kind(&self) -> &'static str {
        "producer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Text>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        self.runs.fetch_add(1, Ordering::Relaxed);
        cx.workspace.push(self.text.clone());
        cx.publish(Text(self.text.clone()))
    }
}
struct Consumer {
    meta: Meta,
    handle: Option<Handle<Text>>,
    optional: bool,
    file: &'static str,
}
impl Consumer {
    fn new(handle: Option<Handle<Text>>) -> Self {
        Self {
            meta: Meta::new(),
            handle,
            optional: false,
            file: "value.txt",
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
    fn requires(&self) -> Vec<Requirement> {
        let r = Requirement::on(self.handle);
        vec![if self.optional { r.optional() } else { r }]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
        let value = cx
            .inputs
            .optional::<Text>()?
            .map(|s| s.0.as_str())
            .unwrap_or("absent");
        cx.workspace.push(format!("read {value}"));
        cx.files.emit(GeneratedFile::new(self.file, value)?)
    }
}
fn run(package: Package<TestLanguage>) -> Result<kaji_core::GeneratedTree> {
    Packages::new()
        .package(package)
        .generate(&Api::default(), None)
}
fn error(package: Package<TestLanguage>) -> String {
    format!("{:#}", run(package).unwrap_err())
}

#[test]
fn auto_binding_orders_dependencies_and_finalizes_shared_workspace() {
    let tree = run(Package::new("sdk")
        .with(Consumer::new(None))
        .with(Producer::new("hello")))
    .unwrap();
    assert_eq!(tree.get("sdk/value.txt"), Some("hello"));
    assert_eq!(tree.get("sdk/order.txt"), Some("hello\nread hello"));
}

#[test]
fn repeated_plugin_kinds_bind_by_instance_and_survive_moves() {
    let first = Producer::new("first");
    let second = Producer::new("second");
    let chosen = second.handle();
    let tree = run(Package::new("sdk")
        .with(Consumer::new(Some(chosen)))
        .with(first)
        .with(second))
    .unwrap();
    assert_eq!(tree.get("sdk/value.txt"), Some("second"));
}

#[test]
fn ambiguous_and_missing_requirements_fail_before_generation() {
    let producer = Producer::new("one");
    let runs = producer.runs.clone();
    let err = error(
        Package::new("sdk")
            .with(producer)
            .with(Producer::new("two"))
            .with(Consumer::new(None)),
    );
    assert!(err.contains("found 2"), "{err}");
    assert!(err.contains("producer (one)") && err.contains("producer (two)"));
    assert_eq!(runs.load(Ordering::Relaxed), 0);
    assert!(error(Package::new("sdk").with(Consumer::new(None))).contains("no provider"));
}

#[test]
fn all_packages_resolve_before_any_generator_runs() {
    let producer = Producer::new("one");
    let runs = producer.runs.clone();
    let err = Packages::new()
        .package(Package::new("a").with(producer))
        .package(Package::new("b").with(Consumer::new(None)))
        .generate(&Api::default(), None)
        .unwrap_err();
    assert!(err.to_string().contains("no provider"));
    assert_eq!(runs.load(Ordering::Relaxed), 0);
}

#[test]
fn optional_requirements_do_not_hide_ambiguity_or_dangling_handles() {
    let optional = || Consumer {
        optional: true,
        ..Consumer::new(None)
    };
    let tree = run(Package::new("sdk").with(optional())).unwrap();
    assert_eq!(tree.get("sdk/value.txt"), Some("absent"));
    assert!(
        error(
            Package::new("sdk")
                .with(optional())
                .with(Producer::new("a"))
                .with(Producer::new("b"))
        )
        .contains("found 2")
    );
    let absent = Producer::new("absent");
    let consumer = Consumer {
        handle: Some(absent.handle()),
        ..optional()
    };
    assert!(error(Package::new("sdk").with(consumer)).contains("not registered"));
}

#[test]
fn handles_cannot_cross_packages_or_claim_the_wrong_contract() {
    let producer = Producer::new("a");
    let consumer = Consumer::new(Some(producer.handle()));
    let err = Packages::new()
        .package(Package::new("a").with(producer))
        .package(Package::new("b").with(consumer))
        .generate(&Api::default(), None)
        .unwrap_err();
    assert!(err.to_string().contains("not registered"));
    struct Wrong {
        meta: Meta,
    }
    impl Plugin<TestLanguage> for Wrong {
        fn kind(&self) -> &'static str {
            "wrong"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn generate(&self, _: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
            Ok(())
        }
    }
    let wrong = Wrong { meta: Meta::new() };
    let consumer = Consumer::new(Some(wrong.meta.handle()));
    assert!(error(Package::new("sdk").with(wrong).with(consumer)).contains("does not provide"));
}

#[test]
fn cycles_including_optional_edges_are_rejected() {
    struct Cycle {
        meta: Meta,
        other: bool,
    }
    impl Plugin<TestLanguage> for Cycle {
        fn kind(&self) -> &'static str {
            "cycle"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn requires(&self) -> Vec<Requirement> {
            vec![if self.other {
                Requirement::on::<Text>(None).optional()
            } else {
                Requirement::on::<Other>(None)
            }]
        }
        fn provides(&self) -> Vec<Provision> {
            vec![if self.other {
                Provision::of::<Other>()
            } else {
                Provision::of::<Text>()
            }]
        }
        fn generate(&self, _: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
            panic!("must resolve before execution")
        }
    }
    assert!(
        error(
            Package::new("sdk")
                .with(Cycle {
                    meta: Meta::new(),
                    other: false
                })
                .with(Cycle {
                    meta: Meta::new(),
                    other: true
                })
        )
        .contains("cycle")
    );
}

#[test]
fn contract_publication_is_checked_and_undeclared_reads_fail() {
    struct Broken {
        meta: Meta,
        mode: u8,
    }
    impl Plugin<TestLanguage> for Broken {
        fn kind(&self) -> &'static str {
            "broken"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn provides(&self) -> Vec<Provision> {
            if self.mode < 2 {
                vec![Provision::of::<Text>()]
            } else {
                vec![]
            }
        }
        fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
            match self.mode {
                0 => Ok(()),
                1 => {
                    cx.publish(Text("a".into()))?;
                    cx.publish(Text("b".into()))
                }
                2 => cx.publish(Text("a".into())),
                _ => cx.inputs.optional::<Text>().map(|_| ()),
            }
        }
    }
    for (mode, expected) in [
        (0, "did not publish"),
        (1, "published twice"),
        (2, "undeclared contract publication"),
        (3, "undeclared contract read"),
    ] {
        let err = error(Package::new("sdk").with(Broken {
            meta: Meta::new(),
            mode,
        }));
        assert!(err.contains(expected), "{err}");
    }
}

#[test]
fn output_conflicts_report_owners_and_normalize_paths() {
    let first = Consumer {
        file: "./value.txt",
        ..Consumer::new(None)
    };
    let err = error(
        Package::new("sdk")
            .with(Producer::new("a"))
            .with(first)
            .with(Consumer::new(None)),
    );
    assert!(err.contains("both emit value.txt"), "{err}");
    for path in ["../escape", "/absolute", ".", ""] {
        assert!(run(Package::<TestLanguage>::new(path)).is_err(), "{path}");
    }
    assert!(
        Packages::new()
            .package(Package::<TestLanguage>::new("a"))
            .package(Package::<TestLanguage>::new("a/child"))
            .generate(&Api::default(), None)
            .is_err()
    );
}

#[test]
fn common_layers_keep_unset_and_explicit_local_values() {
    let shared = Common::default()
        .client_name("Shared")
        .client_style(SdkClientStyle::Flat);
    let merged = shared.overlay(&Common::default().client_name("Package"));
    assert_eq!(merged.client_name.as_deref(), Some("Package"));
    assert_eq!(merged.client_style, Some(SdkClientStyle::Flat));
    assert_eq!(merged.package_version, None);
}

#[test]
fn generated_custom_files_remain_create_once_across_package_prefixing() {
    struct Custom(Meta);
    impl Plugin<TestLanguage> for Custom {
        fn kind(&self) -> &'static str {
            "custom"
        }
        fn meta(&self) -> &Meta {
            &self.0
        }
        fn generate(&self, cx: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
            cx.files
                .emit_custom(GeneratedFile::new("custom.txt", "initial")?)
        }
    }
    let tree = run(Package::new("sdk").with(Custom(Meta::new()))).unwrap();
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/custom.txt"), "user change").unwrap();
    tree.write_to(dir.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("sdk/custom.txt")).unwrap(),
        "user change"
    );
}
