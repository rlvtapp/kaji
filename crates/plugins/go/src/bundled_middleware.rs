use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    let anchor = "for index := len(config.Middleware) - 1; index >= 0; index-- {";
    let clients = tree
        .iter()
        .filter(|(_, text)| text.contains(anchor))
        .map(|(path, text)| (path.to_path_buf(), text.to_owned()))
        .collect::<Vec<_>>();
    ensure!(
        clients.len() == 1,
        "Go bundled middleware requires the native client middleware registration boundary"
    );
    let (client_path, mut client) = clients.into_iter().next().unwrap();
    let package_line = client
        .lines()
        .find(|line| line.starts_with("package "))
        .ok_or_else(|| anyhow::anyhow!("Go client has no package declaration"))?;
    let parent = client_path.parent().unwrap();
    let mut symbols = Vec::new();
    for item in middleware {
        item.validate()?;
        ensure!(
            ![
                "_",
                "break",
                "default",
                "func",
                "interface",
                "select",
                "case",
                "defer",
                "go",
                "map",
                "struct",
                "chan",
                "else",
                "goto",
                "package",
                "switch",
                "const",
                "fallthrough",
                "if",
                "range",
                "type",
                "continue",
                "for",
                "import",
                "return",
                "var"
            ]
            .contains(&item.symbol.as_str()),
            "Go middleware symbol cannot be a keyword or blank identifier"
        );
        ensure!(
            item.async_symbol.is_none(),
            "Go middleware does not accept async_symbol"
        );
        ensure!(
            item.path.parent() == Some(parent)
                && item.path.extension().is_some_and(|ext| ext == "go"),
            "Go middleware must be a .go source beside the generated client"
        );
        let filename = item
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap();
        ensure!(
            !filename.starts_with(['_', '.'])
                && !filename.ends_with("_test.go")
                && !filename.trim_end_matches(".go").split('_').any(|part| [
                    "aix",
                    "android",
                    "darwin",
                    "dragonfly",
                    "freebsd",
                    "illumos",
                    "ios",
                    "js",
                    "linux",
                    "netbsd",
                    "openbsd",
                    "plan9",
                    "solaris",
                    "wasip1",
                    "windows",
                    "386",
                    "amd64",
                    "arm",
                    "arm64",
                    "loong64",
                    "mips",
                    "mips64",
                    "mips64le",
                    "mipsle",
                    "ppc64",
                    "ppc64le",
                    "riscv64",
                    "s390x",
                    "wasm"
                ]
                .contains(&part)),
            "Go bundled middleware filenames cannot be hidden, tests, or platform-restricted"
        );
        ensure!(
            tree.get(&item.path).is_none(),
            "Go middleware source collides with a generated file"
        );
        ensure!(
            item.contents
                .lines()
                .filter(|line| line.starts_with("package "))
                .count()
                == 1
                && item.contents.lines().any(|line| line == package_line),
            "Go middleware package must equal the generated SDK package"
        );
        ensure!(
            !item.contents.contains("//go:build") && !item.contents.contains("// +build"),
            "Go bundled middleware cannot use build constraints"
        );
        symbols.push(item.symbol.clone());
        tree.insert(GeneratedFile::new(&item.path, &item.contents)?)?;
    }
    ensure!(
        client.matches(anchor).count() == 1,
        "Go client has ambiguous middleware registration"
    );
    client = client.replacen(anchor, &format!("middleware := append([]PoolsterMiddleware{{{}}}, config.Middleware...)\n\tfor index := len(middleware) - 1; index >= 0; index-- {{", symbols.join(", ")), 1).replace("config.Middleware[index]", "middleware[index]");
    tree.replace(GeneratedFile::new(client_path, client)?)?;
    Ok(())
}
