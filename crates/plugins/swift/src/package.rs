//! Typed package integration for the Swift SDK generator.
use anyhow::Result;
use kaji_core::SdkClientStyle;
use kaji_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Swift;

#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}

impl Language for Swift {
    const NAME: &'static str = "swift";
    type Settings = Settings;
    type Workspace = ();
    fn finalize_files(tree: &mut kaji_core::GeneratedTree) -> Result<()> {
        if tree.get("test/OperationTests.swift").is_some() {
            anyhow::ensure!(
                tree.get("Package.swift").is_some(),
                "Swift operation tests require a generated SDK"
            );
        }
        if !tree
            .iter()
            .any(|(path, _)| path.ends_with("StandardWebhooks.swift"))
        {
            return Ok(());
        }
        let original = tree
            .get("Package.swift")
            .ok_or_else(|| {
                anyhow::anyhow!("Swift webhooks requires a generated SDK Package.swift")
            })?
            .to_owned();
        let marker = "    targets: [.target(name: ";
        if !original.contains(marker) {
            anyhow::bail!("Swift webhooks cannot modify this package manifest");
        }
        let updated = original.replace(marker, "    dependencies: [.package(url: \"https://github.com/apple/swift-crypto.git\", exact: \"3.12.3\")],\n    targets: [.target(name: ");
        let updated = updated.replace(
            ")]\n)",
            ", dependencies: [.product(name: \"Crypto\", package: \"swift-crypto\")])]\n)",
        );
        tree.replace(kaji_core::GeneratedFile::new("Package.swift", updated)?)
    }
    fn bundle_middleware(
        tree: &mut kaji_core::GeneratedTree,
        middleware: &[kaji_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled::bundle(tree, middleware)
    }
}

pub fn package(dir: impl Into<String>) -> Package<Swift> {
    Package::new(dir)
}

pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}

impl PackageExt for Package<Swift> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}

pub struct Sdk {
    meta: Meta,
    client_style: Option<SdkClientStyle>,
}

pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        client_style: None,
    }
}

impl Sdk {
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }

    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
}

impl Plugin<Swift> for Sdk {
    fn kind(&self) -> &'static str {
        "swift-sdk"
    }

    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn generate(&self, cx: &mut PluginContext<'_, Swift>) -> Result<()> {
        cx.files.append(crate::render_sdk(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            self.client_style
                .or(cx.common.client_style)
                .unwrap_or(SdkClientStyle::Namespaced),
        )?)
    }
}
