//! Editable registry release scaffolding; generation never signs or publishes.
use crate::Terraform;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Meta, Plugin, PluginContext},
};

pub struct ReleaseScaffold {
    meta: Meta,
}
pub fn release_scaffold() -> ReleaseScaffold {
    ReleaseScaffold { meta: Meta::new() }
}
impl Plugin<Terraform> for ReleaseScaffold {
    fn kind(&self) -> &'static str {
        "terraform-release-scaffold"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Terraform>) -> Result<()> {
        let name = cx.settings.provider_name.as_deref().ok_or_else(|| {
            anyhow::anyhow!("release-scaffold requires an explicit provider_name")
        })?;
        ensure!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                && name.chars().next().is_some_and(|c| c.is_ascii_lowercase()),
            "release provider_name must start with a lowercase letter and use lowercase letters or digits so binary and provider addresses match"
        );
        let namespace = cx.settings.registry_namespace.as_deref().ok_or_else(|| {
            anyhow::anyhow!("release-scaffold requires an explicit registry_namespace")
        })?;
        ensure!(
            !namespace.is_empty()
                && namespace
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "release registry_namespace must be lowercase letters, digits or hyphens"
        );
        for (path, source) in [
            (
                ".goreleaser.yml",
                include_str!("../templates/goreleaser.yml.tmpl").replace("__PROVIDER__", name),
            ),
            (
                ".poolster/templates/terraform-release.yml",
                include_str!("../templates/release.yml.tmpl").to_owned(),
            ),
            (
                "terraform-registry-manifest.json",
                "{\"version\":1,\"metadata\":{\"protocol_versions\":[\"6.0\"]}}\n".into(),
            ),
            (
                "RELEASING.md",
                format!(
                    "# Release terraform-provider-{name}\n\nRegistry namespace: `{namespace}`. These are editable, create-once sources. Generation does not sign or publish.\n\n1. Use a dedicated public `terraform-provider-{name}` repository; place the generated package at its root.\n2. Review `.goreleaser.yml`; install GoReleaser v2 and run `goreleaser check` plus `go test ./...`. A local unsigned `goreleaser release --snapshot --clean --skip=publish,sign` validates archives without publication.\n3. Register your provider and public GPG signing key in the Terraform Registry.\n4. Copy `.poolster/templates/terraform-release.yml` to `.github/workflows/terraform-release.yml` after review. Configure the protected `release` environment and signing secrets.\n5. Push a new immutable `vMAJOR.MINOR.PATCH` tag. Checks precede signing/publication. Do not replace a released version.\n\nThe workflow emits native archives, protocol manifest, SHA-256 sums and detached signature using GoReleaser. Registry registration and live signing/publication remain unverified. No state migration is generated. See https://developer.hashicorp.com/terraform/registry/providers/publishing.\n"
                ),
            ),
        ] {
            cx.files.emit_custom(GeneratedFile::new(path, source)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackageExt;
    use poolster_core::{Api, engine::Packages};
    #[test]
    fn release_scaffold_requires_destination_and_preserves_authored_files() {
        let api = Api {
            name: "Example".into(),
            ..Api::default()
        };
        assert!(
            Packages::new()
                .package(crate::package("terraform").with(release_scaffold()))
                .generate(&api, None)
                .is_err()
        );
        let tree = Packages::new()
            .package(
                crate::package("terraform")
                    .provider_name("widgets")
                    .registry_namespace("acme")
                    .with(release_scaffold()),
            )
            .generate(&api, None)
            .unwrap();
        for name in [
            ".goreleaser.yml",
            "terraform-registry-manifest.json",
            "RELEASING.md",
            ".poolster/templates/terraform-release.yml",
        ] {
            assert!(tree.preserves_existing(format!("terraform/{name}")));
        }
        assert!(
            tree.get("terraform/.github/workflows/terraform-release.yml")
                .is_none()
        );
        assert!(
            tree.get("terraform/.goreleaser.yml")
                .unwrap()
                .contains("project_name: terraform-provider-widgets")
        );
        assert!(
            Packages::new()
                .package(
                    crate::package("terraform")
                        .provider_name("widgets-api")
                        .registry_namespace("acme")
                        .with(release_scaffold())
                )
                .generate(&api, None)
                .is_err()
        );
    }
}
