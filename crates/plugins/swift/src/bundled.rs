use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    if middleware.is_empty() {
        return Ok(());
    }
    let clients: Vec<_> = tree
        .iter()
        .filter(|(path, _)| {
            path.to_str()
                .is_some_and(|p| p.starts_with("Sources/") && p.ends_with("/PoolsterClient.swift"))
        })
        .map(|(path, _)| path.to_owned())
        .collect();
    ensure!(
        clients.len() == 1,
        "Swift bundled middleware requires exactly one generated PoolsterClient"
    );
    let client_path = &clients[0];
    let parent = client_path.parent().unwrap();
    let mut source = tree.get(client_path).unwrap().to_owned();
    let marker = "self.transport = transport ?? PoolsterURLSessionTransport(session: session)";
    ensure!(
        source.matches(marker).count() == 1
            && source.contains("public struct PoolsterMiddlewareTransport"),
        "Swift bundled middleware requires the default transport runtime"
    );
    let mut chain = "transport ?? PoolsterURLSessionTransport(session: session)".to_owned();
    for layer in middleware {
        ensure!(
            layer.async_symbol.is_none(),
            "Swift middleware does not accept async_symbol"
        );
        ensure!(
            layer.path.parent() == Some(parent)
                && layer.path.extension().is_some_and(|e| e == "swift"),
            "Swift middleware path must be a .swift file beside the generated PoolsterClient"
        );
        ensure!(
            identifier(&layer.symbol),
            "Swift middleware factory must be a root identifier"
        );
        ensure!(
            tree.get(&layer.path).is_none(),
            "Swift middleware source path collision"
        );
    }
    for layer in middleware.iter().rev() {
        chain = format!(
            "PoolsterMiddlewareTransport(inner: {chain}, middleware: {}())",
            layer.symbol
        );
    }
    source = source.replacen(marker, &format!("self.transport = {chain}"), 1);
    for layer in middleware {
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
    tree.replace(GeneratedFile::new(client_path, source)?)
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
    #[ignore = "requires Swift toolchain"]
    fn author_policy_is_enabled_without_registration() {
        use crate::PackageExt;
        let policy = BundledMiddleware {
            path: "Sources/DemoSdk/Policy.swift".into(), symbol: "authorPolicy".into(), async_symbol: None,
            contents: r#"import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
public func authorPolicy() -> PoolsterMiddleware {
    { request, _ in
        (Data("\"author\"".utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: [:])!)
    }
}
"#.into(),
        };
        let api = poolster_core::Api {
            name: "demo".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        let tree = poolster_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("demo-sdk")
                    .with(crate::sdk())
                    .middleware(policy),
            )
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("Client.swift"),
            tree.get("sdk/Sources/DemoSdk/PoolsterClient.swift")
                .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("Policy.swift"),
            tree.get("sdk/Sources/DemoSdk/Policy.swift").unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("Probe.swift"),
            r#"import Foundation
@main struct Probe {
    static func main() async throws {
        let client = PoolsterClient(options: .init(baseURL: URL(string:"https://unused.example")!))
        let request = try client.makeRequest(method:"GET", path:"/label")
        let value = try await client.send(request, as:String.self)
        precondition(value == "author")
    }
}
"#,
        )
        .unwrap();
        let output = std::process::Command::new("swiftc")
            .args([
                "-swift-version",
                "6",
                "-warnings-as-errors",
                "-module-cache-path",
                "cache",
                "Client.swift",
                "Policy.swift",
                "Probe.swift",
                "-o",
                "probe",
            ])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(root.path().join("probe"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn rejects_unsupported_paths_and_collisions() {
        let mut tree = GeneratedTree::default();
        tree.insert(
            GeneratedFile::new("Sources/Demo/PoolsterClient.swift", crate::client_runtime())
                .unwrap(),
        )
        .unwrap();
        let policy = BundledMiddleware {
            path: "Sources/Other/Policy.swift".into(),
            contents: "".into(),
            symbol: "policy".into(),
            async_symbol: None,
        };
        assert!(bundle(&mut tree, &[policy]).is_err());
        let policy = BundledMiddleware {
            path: "Sources/Demo/PoolsterClient.swift".into(),
            contents: "".into(),
            symbol: "policy".into(),
            async_symbol: None,
        };
        assert!(bundle(&mut tree, &[policy]).is_err());
    }
}
