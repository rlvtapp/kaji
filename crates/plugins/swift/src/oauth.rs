use super::*;
use poolster_core::engine::{Meta, Plugin, PluginContext};
pub struct OAuth {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
}
pub fn oauth() -> OAuth {
    OAuth {
        http_input: Default::default(),
        meta: Meta::new(),
    }
}
impl Plugin<crate::Swift> for OAuth {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http_input.requirements()
    }

    fn kind(&self) -> &'static str {
        "swift-oauth"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Swift>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            let package = cx
                .settings
                .package_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{}-sdk", kebab_case(&cx.api.name)));
            let module = type_name(&package);
            cx.files.emit(GeneratedFile::new(
                format!("Sources/{module}/OAuth.swift"),
                include_str!("../templates/oauth.swift.tmpl"),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires Swift compiler"]
    fn native_oauth_singleflight_origin_and_explicit_auth() {
        let api = Api {
            name: "Probe".into(),
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).with(oauth()))
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let mut source = String::new();
        for entry in std::fs::read_dir(dir.path().join("sdk/Sources/ProbeSdk")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|s| s == "swift") {
                source.push_str(&std::fs::read_to_string(path).unwrap());
                source.push('\n');
            }
        }
        source.push_str(include_str!("../tests/fixtures/oauth_probe.swift"));
        let path = dir.path().join("probe.swift");
        std::fs::write(&path, source).unwrap();
        let binary = dir.path().join("probe");
        let output = std::process::Command::new("swiftc")
            .arg("-parse-as-library")
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[path = "oauth_input.rs"]
mod http_input;
