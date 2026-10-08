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
    let mut staged = tree.clone();
    for entry in middleware {
        entry.validate()?;
        if entry.async_symbol.is_some() {
            bail!(
                "native middleware does not support async_symbol; wrap the native transport instead"
            );
        }
        if entry.path.extension().and_then(|value| value.to_str()) != Some("cs")
            || entry
                .path
                .components()
                .any(|part| part.as_os_str() == "obj" || part.as_os_str() == "bin")
        {
            bail!("C# middleware must be a compiled .cs source outside bin/obj");
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
    let mut wrapped =
        "(httpClient ?? throw new ArgumentNullException(nameof(httpClient)))".to_owned();
    for entry in middleware.iter().rev() {
        wrapped = format!("{}.Wrap({wrapped})", entry.symbol);
    }
    let updated = original.replacen(ANCHOR, &format!("_httpClient = {wrapped} ?? throw new InvalidOperationException(\"Middleware returned no transport\");"), 1);
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

const ANCHOR: &str =
    "_httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));";
const DOCUMENTATION: &str = r#"Each configured .cs source is compiled and enabled automatically. Define the configured class in the generated namespace with `public static HttpClient Wrap(HttpClient next)`. Return a client whose handler delegates to next.SendAsync while preserving cancellation, request/response content and streaming lifetimes. Clone HttpRequestMessage before forwarding into another HttpClient: the outer HttpClient already marks its request as sent; forwarding the same instance is invalid. The SDK automatically wraps its supplied HttpClient; callers do not configure middleware. Declaration order is outermost first. Decorators see each HTTP attempt including SDK retries. The wrapper owns resources it creates; do not dispose the caller-owned next client. No async_symbol or other ABI is supported."#;

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
        let first = entry(&target.parent().unwrap().join("First.cs"), "First");
        let second = entry(&target.parent().unwrap().join("Second.cs"), "Second");
        bundle(&mut generated, &[first.clone(), second.clone()]).unwrap();
        let source = generated.get(&target).unwrap();
        assert!(source.contains("First.Wrap(Second.Wrap("));
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
        assert!(source.contains("httpClient ?? throw"));
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
        let first = entry(&target.parent().unwrap().join("First.cs"), "First");
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
    #[ignore = "requires .NET 8; compiles bundled customer factory and executes generated constructor"]
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
        let path = target.parent().unwrap().join("CustomerPolicy.cs");
        let mut customer = entry(&path, "CustomerPolicy");
        customer.contents = format!(
            "namespace {namespace}; public static class CustomerPolicy {{ public static int Calls; public static System.Net.Http.HttpClient Wrap(System.Net.Http.HttpClient next) {{ Calls++; return next; }} }}"
        );
        bundle(&mut generated, &[customer]).unwrap();
        let root = tempfile::tempdir().unwrap();
        generated.write_to(root.path()).unwrap();

        let project = generated
            .iter()
            .find(|(path, _)| path.extension().is_some_and(|name| name == "csproj"))
            .unwrap()
            .0
            .to_owned();
        let project_text = generated.get(&project).unwrap().replace(
            "<TargetFramework>",
            "<OutputType>Exe</OutputType><TargetFramework>",
        );
        std::fs::write(root.path().join(&project), project_text).unwrap();
        let probe = format!(
            "var client = new {namespace}.PoolsterClient(new System.Net.Http.HttpClient(), new {namespace}.PoolsterClientOptions {{ BaseUrl = \"https://example.test\" }}); if ({namespace}.CustomerPolicy.Calls != 1) throw new System.Exception(\"Not automatically registered\");"
        );
        std::fs::write(root.path().join("Program.cs"), probe).unwrap();
        let output = std::process::Command::new("dotnet")
            .args(["run", "--project"])
            .arg(root.path().join(project))
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
