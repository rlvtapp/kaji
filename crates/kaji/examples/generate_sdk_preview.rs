//! Generates a complete multi-language Kaji SDK preview from Kaji Go compiler artifacts.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use kaji::{ProfileSet, generate_openapi};

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1);
    let artifacts = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: generate_sdk_preview <openapi-artifacts> <output-dir> [name] [version]")?;
    let output = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: generate_sdk_preview <openapi-artifacts> <output-dir> [name] [version]")?;
    let name = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "Example API".into());
    let version = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "0.1.0".into());
    if arguments.next().is_some() {
        bail!("usage: generate_sdk_preview <openapi-artifacts> <output-dir> [name] [version]");
    }

    let tree = generate_openapi(
        &artifacts,
        name,
        version,
        ProfileSet::new("sdks")
            .rust()
            .typescript_fetch()
            .typescript_axios()
            .go()
            .python()
            .php()
            .java()
            .dotnet()
            .elixir(),
    )?;
    let count = tree.iter().count();
    tree.write_to(&output)?;
    println!("Wrote {count} generated files to {}", output.display());
    Ok(())
}
