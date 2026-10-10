use super::*;

/// Add native async operations while sharing the exact synchronous model package.
pub(crate) fn render_sdk_with_async(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    style: SdkClientStyle,
    enabled: bool,
) -> Result<GeneratedTree> {
    let mut tree = render_sdk(api, output_dir, package_name, style)?;
    if !enabled {
        return Ok(tree);
    }
    let root = output_dir.trim_matches('/');
    let distribution = package_name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}-sdk", kebab_case(&api.name)));
    let module = python_module_name(&distribution);
    let prefix = format!("{root}/src/{module}/");
    let mut files = Vec::new();
    for (path, contents) in tree.iter() {
        let path = path.to_string_lossy();
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        if relative.starts_with("operations_") {
            files.push((
                format!("{prefix}async_{relative}"),
                async_operation_source(contents),
            ));
        } else if let Some(resource) = relative.strip_prefix("resources/") {
            files.push((
                format!("{prefix}async_resources/{resource}"),
                async_resource_source(contents),
            ));
        }
    }
    for (path, contents) in files {
        tree.insert(GeneratedFile::new(path, contents)?)?;
    }
    tree.insert(GeneratedFile::new(
        format!("{prefix}async_runtime.py"),
        include_str!("../../templates/async_runtime.py"),
    )?)?;
    let facade = render_client_facade(api, style)
        .replace(
            "from .runtime import BaseClient",
            "from .async_runtime import AsyncBaseClient",
        )
        .replace("from .operations_", "from .async_operations_")
        .replace("from .resources.", "from .async_resources.")
        .replace("class Client(", "class AsyncClient(")
        .replace(", BaseClient)", ", AsyncBaseClient)")
        .replace(
            "class AsyncClient(BaseClient)",
            "class AsyncClient(AsyncBaseClient)",
        );
    tree.insert(GeneratedFile::new(
        format!("{prefix}async_client.py"),
        facade,
    )?)?;
    let init_path = format!("{prefix}__init__.py");
    let init = format!(
        "{}\nfrom .async_client import AsyncClient\n",
        tree.get(&init_path).unwrap()
    );
    tree.replace(GeneratedFile::new(init_path, init)?)?;
    let manifest_path = format!("{root}/pyproject.toml");
    let manifest = format!(
        "{}\n[project.optional-dependencies]\nasync = [\"httpx>=0.27,<1\"]\n",
        tree.get(&manifest_path).unwrap()
    );
    tree.replace(GeneratedFile::new(manifest_path, manifest)?)?;
    Ok(tree)
}

// These transformations apply only to Poolster's own generated operation grammar.
// Models, wire codecs, signatures and pagination declarations remain shared.
pub(crate) fn async_operation_source(source: &str) -> String {
    source
        .replace("Any, Iterator, cast", "Any, AsyncIterator, cast")
        .replace("Iterator[", "AsyncIterator[")
        .replace("    def ", "    async def ")
        .replace("result = self._request(", "result = await self._request(")
        .replace(
            "result = self._event_stream(",
            "result = await self._event_stream(",
        )
        .replace("response = self.", "response = await self.")
}
pub(crate) fn async_resource_source(source: &str) -> String {
    let source = source
        .replace("Any, Iterator", "Any, AsyncIterator")
        .replace("Iterator[", "AsyncIterator[");
    let mut output = String::new();
    for line in source.lines() {
        let line = if line.starts_with("    def ")
            && !line.contains("__init__")
            && !line.contains("_pages(")
        {
            line.replacen("    def ", "    async def ", 1)
        } else if line.contains("return self._client.") && !line.contains("_pages(") {
            line.replacen("return self._client.", "return await self._client.", 1)
        } else {
            line.to_owned()
        };
        output.push_str(&line);
        output.push('\n');
    }
    output
}
