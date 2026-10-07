use anyhow::{Result, ensure};
use kaji_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    let runtimes = tree
        .iter()
        .filter(|(path, _)| path.file_name().is_some_and(|name| name == "runtime.py"))
        .map(|(path, text)| (path.to_path_buf(), text.to_owned()))
        .collect::<Vec<_>>();
    ensure!(
        runtimes.len() == 1,
        "Python bundled middleware requires one native SDK runtime"
    );
    let (runtime_path, mut runtime) = runtimes.into_iter().next().unwrap();
    let parent = runtime_path.parent().unwrap();
    let async_path = parent.join("async_runtime.py");
    let mut asynchronous = tree.get(&async_path).map(str::to_owned);
    let mut sync_imports = String::new();
    let mut async_imports = String::new();
    let mut sync_names = Vec::new();
    let mut async_names = Vec::new();
    for (index, item) in middleware.iter().enumerate() {
        item.validate()?;
        ensure!(
            item.path.parent() == Some(parent)
                && item.path.extension().is_some_and(|ext| ext == "py"),
            "Python middleware must be a .py module beside runtime.py"
        );
        ensure!(
            tree.get(&item.path).is_none(),
            "Python middleware source path collides with a generated file"
        );
        let module = item
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap();
        ensure!(
            identifier(module) && !keyword(module) && !keyword(&item.symbol),
            "Python middleware requires native non-keyword identifiers"
        );
        ensure!(
            asynchronous.is_some() || item.async_symbol.is_none(),
            "async_symbol requires generated Python async output"
        );
        sync_imports.push_str(&format!(
            "from .{module} import {} as _kaji_bundled_{index}\n",
            item.symbol
        ));
        sync_names.push(format!("_kaji_bundled_{index},"));
        if asynchronous.is_some() {
            let symbol = item.async_symbol.as_ref().ok_or_else(|| anyhow::anyhow!("Async Python output requires an explicit async_symbol for each bundled middleware"))?;
            ensure!(
                !keyword(symbol),
                "Python async middleware symbol cannot be a keyword"
            );
            async_imports.push_str(&format!(
                "from .{module} import {symbol} as _kaji_bundled_async_{index}\n"
            ));
            async_names.push(format!("_kaji_bundled_async_{index},"));
        }
        tree.insert(GeneratedFile::new(&item.path, &item.contents)?)?;
    }
    let anchor = "        self.middleware = tuple(middleware)";
    ensure!(
        runtime.matches(anchor).count() == 1,
        "Python runtime does not support bundled native middleware"
    );
    runtime = runtime.replacen(
        anchor,
        &format!(
            "        self.middleware = ({}) + tuple(middleware)",
            sync_names.join(" ")
        ),
        1,
    );
    runtime = runtime.replacen(
        "class BaseClient:",
        &format!("{sync_imports}\nclass BaseClient:"),
        1,
    );
    tree.replace(GeneratedFile::new(runtime_path, runtime)?)?;
    if let Some(ref mut output) = asynchronous {
        let anchor = "        self.async_middleware = tuple(async_middleware)";
        ensure!(
            output.matches(anchor).count() == 1,
            "Python async runtime does not support bundled middleware"
        );
        *output = output.replacen(
            "        if self.middleware:",
            "        if kwargs.get('middleware'):",
            1,
        );
        *output = output.replacen(
            anchor,
            &format!(
                "        self.async_middleware = ({}) + tuple(async_middleware)",
                async_names.join(" ")
            ),
            1,
        );
        *output = output.replacen(
            "class AsyncBaseClient",
            &format!("{async_imports}\nclass AsyncBaseClient"),
            1,
        );
        tree.replace(GeneratedFile::new(async_path, output.clone())?)?;
    }
    Ok(())
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
        })
}
fn keyword(value: &str) -> bool {
    [
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ]
    .contains(&value)
}
