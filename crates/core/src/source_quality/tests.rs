use super::*;

#[cfg(unix)]
fn fixture(script: &str) -> (tempfile::TempDir, SourceFormatter) {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let program = directory.path().join("formatter");
    fs::write(&program,format!("#!/bin/sh\nset -eu\nif [ \"$1\" = --version ]; then echo formatter-1; exit 0; fi\n{script}\n")).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    (
        directory,
        SourceFormatter {
            program: program.to_string_lossy().into(),
            arguments: vec!["{file}".into()],
            version_arguments: vec!["--version".into()],
            expected_version: "formatter-1".into(),
            extensions: vec!["php".into()],
            timeout_seconds: 2,
        },
    )
}
fn tree() -> GeneratedTree {
    let mut tree = GeneratedTree::default();
    tree.insert(GeneratedFile::new("src/a file.php", "<?php dense").unwrap())
        .unwrap();
    tree
}
#[test]
fn unformatted_mode_is_explicit_and_does_not_change_source() {
    let mut tree = tree();
    SourceQuality::Unformatted {
        reason: "PHP formatter unavailable".into(),
    }
    .apply(&mut tree, "php")
    .unwrap();
    assert_eq!(tree.get("src/a file.php"), Some("<?php dense"));
    assert!(
        tree.get(".poolster/source-quality.json")
            .unwrap()
            .contains("unformatted")
    );
}
#[test]
#[cfg(unix)]
fn formatter_handles_spaces_and_records_final_byte_sizes() {
    let (_directory, formatter) = fixture("printf '<?php\\nreadable();\\n' > \"$1\"");
    let mut tree = tree();
    SourceQuality::Formatted {
        formatter,
        max_file_bytes: 128 * 1024,
    }
    .apply(&mut tree, "php")
    .unwrap();
    assert_eq!(tree.get("src/a file.php"), Some("<?php\nreadable();\n"));
    let report: serde_json::Value =
        serde_json::from_str(tree.get(".poolster/source-quality.json").unwrap()).unwrap();
    assert_eq!(report["files"][0]["bytes"], 18);
}
#[test]
#[cfg(unix)]
fn wrong_version_or_invalid_output_is_transactional() {
    let (_directory, mut formatter) = fixture("printf '\\n' > \"$1\"");
    formatter.expected_version = "formatter-2".into();
    let mut tree = tree();
    let before = tree.clone();
    assert!(
        SourceQuality::Formatted {
            formatter: formatter.clone(),
            max_file_bytes: 1
        }
        .apply(&mut tree, "php")
        .is_err()
    );
    assert_eq!(tree, before);
    formatter.expected_version = "formatter-1".into();
    assert!(
        SourceQuality::Formatted {
            formatter,
            max_file_bytes: 1
        }
        .apply(&mut tree, "php")
        .is_err()
    );
    assert_eq!(tree, before);
}
#[test]
#[cfg(unix)]
fn oversized_source_requires_existing_atomic_diagnostic() {
    let (_directory, formatter) = fixture("printf '<?php\\nreadable();\\n' > \"$1\"");
    let mut tree = tree();
    assert!(
        SourceQuality::Formatted {
            formatter: formatter.clone(),
            max_file_bytes: 8
        }
        .apply(&mut tree, "php")
        .is_err()
    );
    tree.insert(
        GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            r#"[{"path":"src/a file.php","reason":"atomic"}]"#,
        )
        .unwrap(),
    )
    .unwrap();
    SourceQuality::Formatted {
        formatter,
        max_file_bytes: 8,
    }
    .apply(&mut tree, "php")
    .unwrap();
    assert!(
        tree.get(".poolster/source-layout-diagnostics.json")
            .unwrap()
            .contains("18")
    );
}

#[test]
#[cfg(unix)]
fn formatting_preserves_source_ownership_and_owns_its_report() {
    let (_directory, formatter) = fixture("printf '<?php\\nreadable();\\n' > \"$1\"");
    let mut tree = tree();
    tree.set_owner("src/a file.php", "customization:author")
        .unwrap();
    SourceQuality::Formatted {
        formatter,
        max_file_bytes: 128 * 1024,
    }
    .apply(&mut tree, "php")
    .unwrap();
    let owners = tree
        .into_owned_files()
        .map(|(file, _, owner)| (file.path, owner))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        owners[std::path::Path::new("src/a file.php")].as_deref(),
        Some("customization:author")
    );
    assert_eq!(
        owners[std::path::Path::new(".poolster/source-quality.json")].as_deref(),
        Some("source-quality")
    );
}

#[test]
#[cfg(unix)]
fn a_later_file_failure_does_not_publish_earlier_formatted_files() {
    let (_directory, formatter) = fixture(
        "case \"$1\" in */b.php) echo invalid >&2; exit 1;; esac\nprintf '<?php\\nreadable();\\n' > \"$1\"",
    );
    let mut tree = tree();
    tree.insert(GeneratedFile::new("src/b.php", "<?php original").unwrap())
        .unwrap();
    let before = tree.clone();
    let error = SourceQuality::Formatted {
        formatter,
        max_file_bytes: 128 * 1024,
    }
    .apply(&mut tree, "php")
    .unwrap_err();
    assert!(format!("{error:#}").contains("invalid"));
    assert_eq!(tree, before);
}

#[test]
#[cfg(unix)]
fn package_formats_after_finalization_and_source_customizations() {
    use crate::engine::{Common, FinalizeContext, Language, Package, Packages};
    struct Example;
    impl Language for Example {
        const NAME: &'static str = "quality-example";
        type Settings = ();
        type Workspace = ();
        fn finalize(cx: &mut FinalizeContext<'_, Self>) -> Result<()> {
            cx.files
                .emit(GeneratedFile::new("source.php", "<?php initial")?)
        }
        fn finalize_files(tree: &mut GeneratedTree) -> Result<()> {
            tree.replace(GeneratedFile::new("source.php", "<?php finalized")?)
        }
    }
    let (_directory, formatter) =
        fixture("grep -q customized \"$1\"\nprintf '<?php\\ncustomized();\\n' > \"$1\"");
    let package = Package::<Example>::new("sdk")
        .common(Common::default().source_quality(SourceQuality::Formatted {
            formatter,
            max_file_bytes: 128 * 1024,
        }))
        .customize(crate::customization::CodeCustomization::Patch {
            path: "source.php".into(),
            find: "finalized".into(),
            replacement: "customized".into(),
        });
    let packages = Packages::new().package(package);
    let plan = packages.plan(false);
    assert!(
        plan.packages[0]
            .stages
            .iter()
            .any(|stage| stage.contains("source formatting"))
    );
    let tree = packages.generate(&crate::Api::default(), None).unwrap();
    assert_eq!(tree.get("sdk/source.php"), Some("<?php\ncustomized();\n"));
    assert!(tree.get("sdk/.poolster/source-quality.json").is_some());
}
