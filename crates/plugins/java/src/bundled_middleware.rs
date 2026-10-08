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
        if entry.path.extension().and_then(|value| value.to_str()) != Some("java")
            || entry.path.parent() != Some(source_directory.as_path())
            || entry.path.file_stem().and_then(|value| value.to_str())
                != Some(entry.symbol.as_str())
        {
            bail!(
                "Java middleware must be {}/{}.java in the generated client package",
                source_directory.display(),
                entry.symbol
            );
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
    let mut wrapped = "(config.httpClient() != null ? config.httpClient() : HttpClient.newBuilder().connectTimeout(timeout).build())".to_owned();
    for entry in middleware.iter().rev() {
        wrapped = format!("{}.wrap({wrapped})", entry.symbol);
    }
    let updated = original.replacen(
        ANCHOR,
        &format!("this.httpClient = Objects.requireNonNull({wrapped}, \"middleware transport\");"),
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

const ANCHOR: &str = "this.httpClient = config.httpClient() != null ? config.httpClient() : HttpClient.newBuilder().connectTimeout(timeout).build();";
const DOCUMENTATION: &str = r#"Each configured source is compiled and enabled automatically. Define the configured class in the generated client package, with `public static java.net.http.HttpClient wrap(java.net.http.HttpClient next)`. It must return a transport decorator preserving all abstract HttpClient methods and both sendAsync overloads. The SDK passes its injected client or its default client; callers need no middleware configuration. Declaration order is outermost first. Decorators see each HTTP attempt including SDK retries. Preserve body handler types and interruption; manage streaming body ownership deliberately. No async_symbol or other ABI is supported."#;

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
        crate::render_sdk_with_policy(
            &api(),
            ".",
            None,
            poolster_core::SdkClientStyle::Namespaced,
            false,
        )
        .unwrap()
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
        let first = entry(&target.parent().unwrap().join("First.java"), "First");
        let second = entry(&target.parent().unwrap().join("Second.java"), "Second");
        bundle(&mut generated, &[first.clone(), second.clone()]).unwrap();
        let source = generated.get(&target).unwrap();
        assert!(source.contains("First.wrap(Second.wrap("));
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
        assert!(source.contains("config.httpClient() != null"));
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
        let first = entry(&target.parent().unwrap().join("First.java"), "First");
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
    #[ignore = "requires Maven and JDK 17; compiles bundled customer class and generated registration"]
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
                line.strip_prefix("package ")
                    .map(|value| value.trim_end_matches(';').to_owned())
            })
            .unwrap();
        let path = target.parent().unwrap().join("CustomerPolicy.java");
        let mut customer = entry(&path, "CustomerPolicy");
        customer.contents = format!(
            "package {namespace};\npublic final class CustomerPolicy {{ public static int calls = 0; public static java.net.http.HttpClient wrap(java.net.http.HttpClient next) {{ calls++; return next; }} }}\n"
        );
        bundle(&mut generated, &[customer]).unwrap();
        let root = tempfile::tempdir().unwrap();
        generated.write_to(root.path()).unwrap();

        let probe = format!(
            "package {namespace}; public final class MiddlewareProbe {{ public static void main(String[] args) {{ new Client(new ClientConfig(\"https://example.test\", null)); if (CustomerPolicy.calls != 1) throw new AssertionError(\"Not automatically registered\"); }} }}"
        );
        std::fs::write(
            root.path()
                .join(target.parent().unwrap())
                .join("MiddlewareProbe.java"),
            probe,
        )
        .unwrap();
        let output = std::process::Command::new("mvn")
            .args([
                "--batch-mode",
                "--no-transfer-progress",
                "-q",
                "-DskipTests",
                "compile",
                "dependency:build-classpath",
                "-Dmdep.outputFile=dependencies.txt",
            ])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let dependencies = std::fs::read_to_string(root.path().join("dependencies.txt")).unwrap();
        let classpath = format!(
            "{}:{}",
            root.path().join("target/classes").display(),
            dependencies.trim()
        );
        let output = std::process::Command::new("java")
            .args(["-cp", &classpath, &format!("{namespace}.MiddlewareProbe")])
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
