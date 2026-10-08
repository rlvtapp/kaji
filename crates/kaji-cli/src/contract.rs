//! Read-only inspection of non-OpenAPI contract formats.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use kaji_inputs::default_registry;

const HELP: &str = "Usage:
  kaji contract plugins [--format human|json]
  kaji contract inspect <file> --input-format <graphql|asyncapi|arazzo|protobuf|capnproto> [--provider <id>] [--format human|json]

Loads native contracts through registered input plugins. Cap'n Proto requires capnp.
";

pub fn run(arguments: Vec<OsString>) -> Result<()> {
    let mut args = arguments.into_iter();
    let command = args
        .next()
        .context("contract requires inspect; run kaji contract --help")?;
    if command == "--help" || command == "-h" {
        print!("{HELP}");
        return Ok(());
    }
    ensure!(
        command == "inspect" || command == "plugins",
        "contract supports inspect; run kaji contract --help"
    );
    let listing = command == "plugins";
    let mut provider = None;
    let mut path = None;
    let mut input_format = None;
    let mut json = false;
    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--help" | "-h" => {
                print!("{HELP}");
                return Ok(());
            }
            "--input-format" => {
                ensure!(
                    input_format.is_none(),
                    "--input-format may be specified once"
                );
                let value = args.next().context("--input-format requires a value")?;
                let value = value.to_string_lossy();
                input_format = Some(
                    match value.as_ref() {
                        "proto" => "protobuf",
                        "capnp" => "capnproto",
                        other => other,
                    }
                    .to_owned(),
                );
            }
            "--provider" => {
                ensure!(provider.is_none(), "--provider may be specified once");
                provider = Some(
                    args.next()
                        .context("--provider requires an identifier")?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--provider must be UTF-8"))?,
                );
            }
            "--format" => {
                let value = args.next().context("--format requires human or json")?;
                json = match value.to_string_lossy().as_ref() {
                    "human" => false,
                    "json" => true,
                    _ => bail!("--format requires human or json"),
                };
            }
            flag if flag.starts_with('-') => bail!("unknown contract inspect option {flag}"),
            _ => {
                ensure!(
                    path.is_none(),
                    "contract inspect accepts exactly one source file"
                );
                path = Some(PathBuf::from(argument));
            }
        }
    }
    let registry = default_registry()?;
    if listing {
        ensure!(
            path.is_none() && input_format.is_none() && provider.is_none(),
            "contract plugins accepts only --format"
        );
        let plugins = registry.plugins();
        if json {
            println!("{}", serde_json::to_string_pretty(&plugins)?);
        } else {
            for plugin in plugins {
                println!("{}  {}", plugin.provider, plugin.format);
            }
        }
        return Ok(());
    }
    let path = path.context("contract inspect requires a source file")?;
    let format = input_format.context("contract inspect requires --input-format")?;
    let loaded = registry.load(&format, provider.as_deref(), &path)?;
    let summary = &loaded.contract.summary;
    if json {
        let report = serde_json::json!({
            "provider": loaded.provider,
            "source": loaded.source,
            "summary": summary,
            "diagnostics": loaded.contract.diagnostics,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{} ({})", summary.title, summary.format);
        println!("Provider: {}", loaded.provider);
        if let Some(version) = &summary.version {
            println!("Version: {version}");
        }
        println!(
            "Types: {}\nOperations: {}",
            summary.types.len(),
            summary.operations.len()
        );
        for operation in &summary.operations {
            println!("  {} {}", operation.kind, operation.name);
        }
        for diagnostic in &loaded.contract.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(source: &str) -> Vec<OsString> {
        source.split_whitespace().map(OsString::from).collect()
    }

    #[test]
    fn requires_explicit_format_before_loading() {
        let error = run(args("inspect missing.graphql")).unwrap_err();
        assert!(error.to_string().contains("--input-format"));
    }

    #[test]
    fn rejects_unknown_commands_and_extra_sources() {
        assert!(run(args("generate schema.graphql")).is_err());
        assert!(run(args("inspect a.graphql b.graphql --input-format graphql")).is_err());
        assert!(
            run(args(
                "inspect a.graphql --input-format graphql --format xml"
            ))
            .is_err()
        );
    }
}
