//! Package-author middleware is compiled into the default runtime.
use anyhow::{Result, ensure};
use kaji_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    if middleware.is_empty() {
        return Ok(());
    }
    let mut client = tree
        .get("src/client/mod.rs")
        .ok_or_else(|| anyhow::anyhow!("Rust bundled middleware requires an SDK client"))?
        .to_owned();
    let marker = "transport: Arc::new(crate::transport::DefaultTransport::default())";
    ensure!(
        client.matches(marker).count() == 1
            && tree
                .get("src/transport.rs")
                .is_some_and(|source| source.contains("pub struct MiddlewareTransport")),
        "Rust bundled middleware requires the default transport provider"
    );
    let mut exports = tree
        .get("src/lib.rs")
        .ok_or_else(|| anyhow::anyhow!("Rust bundled middleware requires src/lib.rs"))?
        .to_owned();
    let mut chain = "crate::transport::DefaultTransport::default()".to_owned();
    let mut override_chain = "transport".to_owned();
    let mut modules = Vec::new();
    for layer in middleware {
        ensure!(
            layer.async_symbol.is_none(),
            "Rust middleware does not accept async_symbol"
        );
        let path = layer
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Rust middleware path must be UTF-8"))?;
        let module = path
            .strip_prefix("src/")
            .and_then(|p| p.strip_suffix(".rs"))
            .ok_or_else(|| anyhow::anyhow!("Rust middleware path must be src/<module>.rs"))?;
        ensure!(
            identifier(module) && identifier(&layer.symbol),
            "Rust middleware module and factory must be root identifiers"
        );
        ensure!(
            !exports.contains(&format!("pub mod {module};"))
                && !exports.contains(&format!("mod {module};"))
                && tree.get(format!("src/{module}/mod.rs")).is_none(),
            "Rust middleware module collision: {module}"
        );
        ensure!(
            tree.get(&layer.path).is_none() && !modules.iter().any(|m| m == module),
            "Rust middleware path collision: {path}"
        );
        modules.push(module.to_owned());
    }
    for (layer, module) in middleware.iter().zip(&modules).rev() {
        chain = format!(
            "crate::transport::MiddlewareTransport::new(crate::{module}::{}(), {chain})",
            layer.symbol
        );
        override_chain = format!(
            "crate::transport::MiddlewareTransport::new(crate::{module}::{}(), {override_chain})",
            layer.symbol
        );
    }
    let override_marker = "self.transport = transport; self";
    ensure!(
        client.matches(override_marker).count() == 1,
        "Rust bundled middleware requires the maintained with_transport ABI"
    );
    client = client
        .replacen(marker, &format!("transport: Arc::new({chain})"), 1)
        .replacen(
            override_marker,
            &format!("self.transport = Arc::new({override_chain}); self"),
            1,
        );
    for (layer, module) in middleware.iter().zip(&modules) {
        exports.push_str(&format!("pub mod {module};\n"));
        tree.insert(GeneratedFile::new(&layer.path, layer.contents.clone())?)?;
        tree.set_owner(&layer.path, format!("bundled-middleware:{}", layer.symbol))?;
    }
    if let Some(readme) = tree.get("README.md") {
        let mut readme = readme.to_owned();
        readme.push_str("\n## Bundled author middleware\n\nThese policies are compiled into the SDK and enabled automatically when a client is created. Consumers need no registration.\n\n");
        for layer in middleware {
            readme.push_str(&format!(
                "- `{}`: `{}`\n",
                layer.path.display(),
                layer.symbol
            ));
        }
        tree.replace(GeneratedFile::new("README.md", readme)?)?;
    }
    tree.replace(GeneratedFile::new("src/client/mod.rs", client)?)?;
    tree.replace(GeneratedFile::new("src/lib.rs", exports)?)
}
fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_custom_transport_and_source_collision() {
        let mut tree = GeneratedTree::default();
        tree.insert(GeneratedFile::new("src/client/mod.rs", "custom transport runtime").unwrap())
            .unwrap();
        let policy = BundledMiddleware {
            path: "src/policy.rs".into(),
            symbol: "policy".into(),
            contents: "".into(),
            async_symbol: None,
        };
        assert!(bundle(&mut tree, std::slice::from_ref(&policy)).is_err());
        let api = kaji_core::Api {
            name: "demo".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        let generated = kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .with(crate::sdk())
                    .middleware(BundledMiddleware {
                        path: "src/models.rs".into(),
                        ..policy
                    }),
            )
            .generate(&api, None);
        assert!(generated.is_err());
    }
}
