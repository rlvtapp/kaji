use anyhow::{Context, Result, bail};
use poolster_core::customization::BundledMiddleware;
use poolster_core::{GeneratedFile, GeneratedTree};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    if middleware.is_empty() {
        return Ok(());
    }
    let middleware = middleware
        .iter()
        .map(|entry| {
            entry.validate()?;
            let mut entry = entry.clone();
            entry.path = entry
                .path
                .components()
                .filter(|part| !matches!(part, std::path::Component::CurDir))
                .collect();
            Ok(entry)
        })
        .collect::<Result<Vec<_>>>()?;
    let middleware = middleware.as_slice();
    let (target, original) = tree
        .iter()
        .find(|(_, text)| text.contains(ANCHOR))
        .map(|(path, text)| (path.to_owned(), text.to_owned()))
        .context("bundled middleware requires the native SDK runtime")?;
    if original.matches(ANCHOR).count() != 1 {
        bail!("unsupported native SDK middleware runtime ABI");
    }
    for marker in [
        "final class Client",
        "$this->baseUrl = rtrim($baseUrl, '/');",
        "private readonly string $baseUrl;",
    ] {
        if original.matches(marker).count() != 1 {
            bail!("unsupported PHP SDK middleware runtime ABI: {marker}");
        }
    }
    let source_directory = target
        .parent()
        .unwrap()
        .components()
        .filter(|part| !matches!(part, std::path::Component::CurDir))
        .collect::<std::path::PathBuf>();
    let mut staged = tree.clone();
    for entry in middleware {
        entry.validate()?;
        if entry.async_symbol.is_some() {
            bail!(
                "native middleware does not support async_symbol; wrap the native transport instead"
            );
        }
        if entry.path.extension().and_then(|value| value.to_str()) != Some("php")
            || !entry.path.starts_with(&source_directory)
        {
            bail!("PHP middleware must be a .php source under the generated src directory");
        }
        if staged.iter().any(|(path, _)| {
            path.components()
                .filter(|part| !matches!(part, std::path::Component::CurDir))
                .collect::<std::path::PathBuf>()
                == entry.path
        }) {
            bail!(
                "bundled middleware collides with generated file {}",
                entry.path.display()
            );
        }
        staged.insert(GeneratedFile::new(&entry.path, entry.contents.clone())?)?;
        staged.set_owner(&entry.path, format!("bundled-middleware:{}", entry.symbol))?;
    }
    let mut wrapped = "$httpClient".to_owned();
    let mut requires = String::new();
    for entry in middleware.iter().rev() {
        wrapped = format!("{}::wrap({wrapped})", entry.symbol);
    }
    for entry in middleware {
        let relative = entry
            .path
            .strip_prefix(&source_directory)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let quoted = relative.replace('\\', "\\\\").replace('\'', "\\'");
        requires.push_str(&format!("require_once __DIR__ . '/{quoted}';\n"));
    }
    let updated = original
        .replacen(ANCHOR, "ClientInterface $httpClient,", 1)
        .replacen(
            "final class Client",
            &format!("{requires}\nfinal class Client"),
            1,
        )
        .replacen(
            "$this->baseUrl = rtrim($baseUrl, '/');",
            &format!(
                "$this->httpClient = {wrapped};\n        $this->baseUrl = rtrim($baseUrl, '/');"
            ),
            1,
        )
        .replacen(
            "private readonly string $baseUrl;",
            "private readonly ClientInterface $httpClient;\n    private readonly string $baseUrl;",
            1,
        );
    staged.replace(GeneratedFile::new(&target, updated)?)?;
    let readme = staged
        .iter()
        .find(|(path, _)| path.file_name().is_some_and(|name| name == "README.md"))
        .map(|(path, contents)| (path.to_owned(), contents.to_owned()));
    if let Some((path, contents)) = readme {
        staged.replace(GeneratedFile::new(
            path,
            format!(
                "{contents}\n## Bundled customer middleware\n\n{}\n",
                DOCUMENTATION
            ),
        )?)?;
    }
    *tree = staged;
    Ok(())
}

const ANCHOR: &str = "private readonly ClientInterface $httpClient,";
const DOCUMENTATION: &str = r#"Each configured .php source is required and enabled automatically. Define the configured class in the generated namespace with `public static function wrap(\Psr\Http\Client\ClientInterface $next): \Psr\Http\Client\ClientInterface`. Return a PSR-18 decorator using PSR-7 replacement messages. The SDK wraps its supplied transport without caller middleware setup. Declaration order is outermost first. Decorators see each HTTP attempt including SDK retries. Preserve PSR-18 exception interfaces, stream positions and live SSE ownership. No async_symbol or other ABI is supported."#;

#[cfg(test)]
mod tests {
    use super::*;
    fn api() -> poolster_core::Api {
        poolster_core::Api {
            name: "Example".into(),
            version: "1.0.0".into(),
            ..Default::default()
        }
    }
    fn tree() -> GeneratedTree {
        crate::render_sdk(&api(), ".", None, poolster_core::SdkClientStyle::Namespaced).unwrap()
    }
    fn entry(path: &std::path::Path, symbol: &str) -> BundledMiddleware {
        BundledMiddleware {
            path: path.to_owned(),
            symbol: symbol.into(),
            contents: "customer-owned source".into(),
            async_symbol: None,
        }
    }
    #[test]
    fn bundles_owned_sources_with_automatic_ordered_registration() {
        let mut generated = tree();
        let target = generated
            .iter()
            .find(|(_, text)| text.contains(ANCHOR))
            .unwrap()
            .0
            .to_owned();
        let first = entry(&target.parent().unwrap().join("First.php"), "First");
        let second = entry(&target.parent().unwrap().join("Second.php"), "Second");
        bundle(&mut generated, &[first.clone(), second.clone()]).unwrap();
        let source = generated.get(&target).unwrap();
        assert!(source.contains("$this->httpClient = First::wrap(Second::wrap($httpClient));"));
        let normalized = first
            .path
            .components()
            .filter(|part| !matches!(part, std::path::Component::CurDir))
            .collect::<std::path::PathBuf>();
        assert_eq!(generated.get(&normalized), Some("customer-owned source"));
        assert!(generated.iter().any(|(path, text)| {
            path.file_name().is_some_and(|name| name == "README.md")
                && text.contains("Bundled customer middleware")
        }));
        assert!(source.contains("require_once __DIR__ . '/First.php';"));
        assert!(source.contains("private readonly ClientInterface $httpClient;"));
        assert!(!source.contains(ANCHOR));
    }
    #[test]
    fn rejects_collisions_and_unsupported_abi_without_partial_changes() {
        let mut generated = tree();
        let target = generated
            .iter()
            .find(|(_, text)| text.contains(ANCHOR))
            .unwrap()
            .0
            .to_owned();
        let first = entry(&target.parent().unwrap().join("First.php"), "First");
        let collision = first.clone();
        let before = generated.clone();
        assert!(
            bundle(&mut generated, &[first.clone(), collision])
                .unwrap_err()
                .to_string()
                .contains("collides")
        );
        assert_eq!(before, generated);
        let mut asynchronous = first;
        asynchronous.async_symbol = Some("Async".into());
        assert!(bundle(&mut generated, &[asynchronous]).is_err());
        assert_eq!(before, generated);
    }
    #[test]
    #[ignore = "requires PHP 8.2; lints generated bundle and executes constructor against PSR interface stubs"]
    fn bundled_middleware_native_compile_and_default_registration() {
        let mut generated = tree();
        let (target, source) = generated
            .iter()
            .find(|(_, text)| text.contains(ANCHOR))
            .map(|(path, text)| (path.to_owned(), text.to_owned()))
            .unwrap();
        let namespace = source
            .lines()
            .find_map(|line| {
                line.strip_prefix("namespace ")
                    .map(|value| value.trim_end_matches(';').to_owned())
            })
            .unwrap();
        let path = target.parent().unwrap().join("CustomerPolicy.php");
        let mut customer = entry(&path, "CustomerPolicy");
        customer.contents = format!(
            "<?php\nnamespace {namespace}; final class CustomerPolicy {{ public static int $calls = 0; public static function wrap(\\Psr\\Http\\Client\\ClientInterface $next): \\Psr\\Http\\Client\\ClientInterface {{ self::$calls++; return $next; }} }}"
        );
        bundle(&mut generated, &[customer]).unwrap();
        let root = tempfile::tempdir().unwrap();
        generated.write_to(root.path()).unwrap();

        for (path, _) in generated
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|name| name == "php"))
        {
            let output = std::process::Command::new("php")
                .arg("-l")
                .arg(root.path().join(path))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let probe = format!(
            r#"<?php
namespace Psr\Http\Client {{ interface ClientInterface {{}} }}
namespace Psr\Http\Message {{ interface RequestFactoryInterface {{}} interface StreamFactoryInterface {{}} }}
namespace Nyholm\Psr7\Factory {{ final class Psr17Factory implements \Psr\Http\Message\RequestFactoryInterface, \Psr\Http\Message\StreamFactoryInterface {{}} }}
namespace {{ require __DIR__ . '/src/Client.php'; $next = new class implements \Psr\Http\Client\ClientInterface {{}}; new \{namespace}\Client($next, 'https://example.test'); if (\{namespace}\CustomerPolicy::$calls !== 1) {{ throw new \RuntimeException('Not automatically registered'); }} }}
"#
        );
        std::fs::write(root.path().join("probe.php"), probe).unwrap();
        let output = std::process::Command::new("php")
            .arg(root.path().join("probe.php"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
