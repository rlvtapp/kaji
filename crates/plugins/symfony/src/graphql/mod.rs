//! Symfony integration for the portable selection-specific PHP GraphQL client.
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
    native::GraphqlOperations,
};
use poolster_plugin_php::{GraphqlStyle, graphql_package_namespace, render_graphql_package};
use std::collections::BTreeMap;

pub struct Graphql {
    meta: Meta,
    provider: Option<Handle<GraphqlOperations>>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
/// Generate a self-contained Symfony bundle and PHP GraphQL SDK.
pub fn graphql(provider: Option<Handle<GraphqlOperations>>) -> Graphql {
    Graphql {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::default(),
        groups: BTreeMap::new(),
    }
}
impl Graphql {
    pub fn raw(mut self) -> Self {
        self.style = GraphqlStyle::Raw;
        self
    }
    pub fn flat(mut self) -> Self {
        self.style = GraphqlStyle::Flat;
        self
    }
    pub fn idiomatic(mut self) -> Self {
        self.style = GraphqlStyle::Idiomatic;
        self
    }
    pub fn namespaced(self) -> Self {
        self.idiomatic()
    }
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
        self
    }
    pub fn group(
        mut self,
        group: impl Into<String>,
        method: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        self.groups
            .entry(group.into())
            .or_default()
            .insert(method.into(), operation.into());
        self
    }
}
impl Plugin<crate::Symfony> for Graphql {
    fn kind(&self) -> &'static str {
        "symfony-graphql"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Symfony>) -> Result<()> {
        ensure!(
            cx.settings.sdk_package.is_none(),
            "Symfony GraphQL bundles include their PHP SDK; sdk_package is an HTTP integration option"
        );
        let contract = cx.inputs.get::<GraphqlOperations>()?;
        let name = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster/graphql-symfony");
        let mut tree = render(contract, name, self.style, &self.groups)?;
        if let Some(version) = &cx.common.package_version {
            let mut manifest: serde_json::Value =
                serde_json::from_str(tree.get("composer.json").unwrap())?;
            manifest["version"] = version.clone().into();
            tree.replace(GeneratedFile::new(
                "composer.json",
                serde_json::to_string_pretty(&manifest)?,
            )?)?;
        }
        cx.files.append(tree)
    }
}
fn render(
    contract: &GraphqlOperations,
    package: &str,
    style: GraphqlStyle,
    groups: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<GeneratedTree> {
    let mut tree = render_graphql_package(contract, package, style, groups)?;
    let sdk = graphql_package_namespace(package);
    let namespace = format!("{sdk}\\Symfony");
    let alias = crate::slug(package).replace('-', "_");
    let stem = sdk.replace('\\', "");
    let bundle = format!("{stem}Bundle");
    let extension = format!("{stem}Extension");
    let mut manifest: serde_json::Value = serde_json::from_str(tree.get("composer.json").unwrap())?;
    manifest["type"] = "symfony-bundle".into();
    for component in [
        "config",
        "dependency-injection",
        "http-client",
        "http-kernel",
    ] {
        manifest["require"][format!("symfony/{component}")] = "^6.4 || ^7.0".into();
    }
    manifest["autoload"]["psr-4"] = serde_json::json!({format!("{namespace}\\"): "src/Symfony/"});
    tree.replace(GeneratedFile::new(
        "composer.json",
        serde_json::to_string_pretty(&manifest)?,
    )?)?;
    tree.insert(GeneratedFile::new(
        format!("src/Symfony/{bundle}.php"),
        crate::bundle(&namespace, &bundle),
    )?)?;
    for (path, source) in [
        (
            "GraphqlTransport.php",
            include_str!("runtime/transport.php.tmpl"),
        ),
        (
            "DependencyInjection/PoolsterGraphqlExtension.php",
            include_str!("runtime/extension.php.tmpl"),
        ),
        (
            "DependencyInjection/Configuration.php",
            include_str!("runtime/configuration.php.tmpl"),
        ),
    ] {
        tree.insert(GeneratedFile::new(
            format!(
                "src/Symfony/{}",
                path.replace("PoolsterGraphqlExtension", &extension)
            ),
            source
                .replace("__NAMESPACE__", &namespace)
                .replace("__SDK__", &sdk)
                .replace("__ALIAS__", &alias)
                .replace("__EXTENSION__", &extension),
        )?)?;
    }
    let readme = include_str!("readme.md.tmpl")
        .replace("__SDK__", &sdk)
        .replace("__NAMESPACE__", &namespace)
        .replace("__ALIAS__", &alias)
        .replace("__BUNDLE__", &bundle);
    tree.replace(GeneratedFile::new("README.md", readme)?)?;
    Ok(tree)
}
#[cfg(test)]
mod tests;
