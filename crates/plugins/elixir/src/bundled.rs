use anyhow::{Result, ensure};
use kaji_core::{GeneratedFile, GeneratedTree, customization::BundledMiddleware};

pub(crate) fn bundle(tree: &mut GeneratedTree, middleware: &[BundledMiddleware]) -> Result<()> {
    if middleware.is_empty() {
        return Ok(());
    }
    let clients: Vec<_> = tree
        .iter()
        .filter(|(path, _)| {
            path.to_str()
                .is_some_and(|p| p.starts_with("lib/") && p.ends_with("/client.ex"))
        })
        .map(|(path, _)| path.to_owned())
        .collect();
    ensure!(
        clients.len() == 1,
        "Elixir bundled middleware requires exactly one generated client"
    );
    let client_path = &clients[0];
    let mut source = tree.get(client_path).unwrap().to_owned();
    let marker = "middleware: Keyword.get(options, :middleware, [])";
    ensure!(
        source.matches(marker).count() == 1 && source.contains("defp execute_transport"),
        "Elixir bundled middleware requires the default client runtime"
    );
    let mut references = Vec::new();
    for layer in middleware {
        ensure!(
            layer.async_symbol.is_none(),
            "Elixir middleware does not accept async_symbol"
        );
        let path = layer
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Elixir middleware path must be UTF-8"))?;
        let name = path
            .strip_prefix("lib/")
            .and_then(|p| p.strip_suffix(".ex"))
            .ok_or_else(|| anyhow::anyhow!("Elixir middleware path must be lib/<name>.ex"))?;
        ensure!(
            !name.contains('/')
                && !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Elixir middleware file must be directly under lib"
        );
        ensure!(
            layer
                .symbol
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
                && layer
                    .symbol
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Elixir middleware symbol must be a root module name"
        );
        ensure!(
            tree.get(&layer.path).is_none(),
            "Elixir middleware source path collision"
        );
        references.push(format!("&{}.handle/2", layer.symbol));
    }
    source = source.replacen(
        marker,
        &format!(
            "middleware: [{}] ++ Keyword.get(options, :middleware, [])",
            references.join(", ")
        ),
        1,
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    fn generated() -> GeneratedTree {
        use crate::PackageExt;
        let policy = BundledMiddleware {path:"lib/policy.ex".into(), symbol:"AuthorPolicy".into(), async_symbol:None,
            contents:"defmodule AuthorPolicy do\n  def handle(_request, _next), do: {:ok, struct(Finch.Response, status: 200, body: \"author\")}\nend\n".into()};
        let api = kaji_core::Api {
            name: "demo".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        kaji_core::engine::Packages::new()
            .package(
                crate::package("sdk")
                    .name("probe")
                    .with(crate::sdk())
                    .middleware(policy),
            )
            .generate(&api, None)
            .unwrap()
    }
    #[test]
    fn bundles_policy_with_default_registration() {
        let tree = generated();
        assert!(tree.get("sdk/lib/probe/client.ex").unwrap().contains(
            "middleware: [&AuthorPolicy.handle/2] ++ Keyword.get(options, :middleware, [])"
        ));
        assert!(
            tree.get("sdk/lib/policy.ex")
                .unwrap()
                .contains("defmodule AuthorPolicy")
        );
    }
    #[test]
    #[ignore = "requires Elixir toolchain"]
    fn author_policy_executes_without_registration() {
        let tree = generated();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("multipart_body.ex"),
            tree.get("sdk/lib/probe/multipart_body.ex").unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("client.ex"),
            tree.get("sdk/lib/probe/client.ex").unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.path().join("policy.ex"),
            tree.get("sdk/lib/policy.ex").unwrap(),
        )
        .unwrap();
        std::fs::write(root.path().join("probe.exs"),r#"
defmodule Finch.Request do
  defstruct [:method, :url, :body, headers: []]
  @type t :: %__MODULE__{}
end
defmodule Finch.Response do
  defstruct status: 200, headers: [], body: ""
end
defmodule Finch do
  def build(method, url, headers, body), do: struct(Finch.Request, method: method, url: url, headers: headers, body: body)
  def request(_, _, _), do: raise("default transport unexpectedly called")
  def stream(_, _, _, _, _), do: raise("default stream unexpectedly called")
end
defmodule Probe.ApiError do
  defexception [:status, :body, :headers]
end
defmodule Probe.JSON do
  def decode(body), do: {:ok, body}
  def to_wire(body), do: body
end
Code.compile_file("policy.ex")
Code.compile_file("multipart_body.ex")
Code.compile_file("client.ex")
{:ok, client} = Probe.Client.new(base_url: "https://unused.example")
{:ok, "author"} = Probe.Client.request(client, :get, "/label", [], [], nil, :json, :text)
"#).unwrap();
        let output = std::process::Command::new("elixir")
            .arg("probe.exs")
            .current_dir(root.path())
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
