use kaji_core::{Api, CodegenPlugin, GeneratedFile, GeneratedTree, GeneratorConfig, generate};

struct First;

impl CodegenPlugin for First {
    fn name(&self) -> &'static str {
        "first"
    }

    fn generate(
        &self,
        _api: &Api,
        _config: &GeneratorConfig,
    ) -> anyhow::Result<Vec<GeneratedFile>> {
        Ok(vec![GeneratedFile::new("models.ts", "export {};\n")?])
    }
}

struct Conflicting;

impl CodegenPlugin for Conflicting {
    fn name(&self) -> &'static str {
        "conflicting"
    }

    fn generate(
        &self,
        _api: &Api,
        _config: &GeneratorConfig,
    ) -> anyhow::Result<Vec<GeneratedFile>> {
        Ok(vec![GeneratedFile::new("models.ts", "different\n")?])
    }
}

#[test]
fn engine_preserves_output_and_rejects_path_conflicts() {
    let api = Api::default();
    let first = First;
    let result = generate(&api, &[(&first, GeneratorConfig::default())]).unwrap();
    assert_eq!(result.get("models.ts"), Some("export {};\n"));

    let conflicting = Conflicting;
    assert!(
        generate(
            &api,
            &[
                (&first, GeneratorConfig::default()),
                (&conflicting, GeneratorConfig::default()),
            ],
        )
        .is_err()
    );
}

#[test]
fn generated_files_cannot_escape_the_output_directory() {
    assert!(GeneratedFile::new("../outside.ts", "").is_err());
    assert!(GeneratedFile::new("/absolute.ts", "").is_err());
    assert!(GeneratedTree::default().get("missing.ts").is_none());
}
