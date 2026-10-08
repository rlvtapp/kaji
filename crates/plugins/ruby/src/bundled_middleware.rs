use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    let anchor = "      @middleware = middleware.to_a.dup.freeze";
    let clients = tree
        .iter()
        .filter(|(_, text)| text.contains(anchor))
        .map(|(path, text)| (path.to_path_buf(), text.to_owned()))
        .collect::<Vec<_>>();
    ensure!(
        clients.len() == 1,
        "Ruby bundled middleware requires one native client registration boundary"
    );
    let (client_path, mut client) = clients.into_iter().next().unwrap();
    let mut imports = String::new();
    let mut symbols = Vec::new();
    for item in middleware {
        item.validate()?;
        ensure!(
            !["BEGIN", "END"].contains(&item.symbol.as_str()),
            "Ruby middleware symbol cannot be a keyword"
        );
        ensure!(
            item.async_symbol.is_none(),
            "Ruby middleware does not accept async_symbol"
        );
        ensure!(
            item.path.parent() == client_path.parent()
                && item.path.extension().is_some_and(|ext| ext == "rb"),
            "Ruby middleware must be a .rb source beside client.rb"
        );
        let module = item
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap();
        ensure!(
            module
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
            "Ruby middleware module path requires an ASCII filename"
        );
        ensure!(
            item.symbol
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_uppercase),
            "Ruby middleware symbol must be a top-level callable constant"
        );
        ensure!(
            tree.get(&item.path).is_none(),
            "Ruby middleware source collides with a generated file"
        );
        ensure!(
            !symbols.contains(&format!("::{}", item.symbol)),
            "Ruby bundled middleware constants must be unique"
        );
        imports.push_str(&format!("require_relative '{module}'\n"));
        symbols.push(format!("::{}", item.symbol));
        tree.insert(GeneratedFile::new(&item.path, &item.contents)?)?;
    }
    ensure!(
        client.matches(anchor).count() == 1,
        "Ruby client has ambiguous middleware registration"
    );
    client = client.replacen(
        anchor,
        &format!(
            "      @middleware = ([{}] + middleware.to_a).freeze",
            symbols.join(", ")
        ),
        1,
    );
    client = format!("{imports}{client}");
    tree.replace(GeneratedFile::new(client_path, client)?)?;
    Ok(())
}
