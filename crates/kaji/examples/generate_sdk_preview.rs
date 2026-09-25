//! Generates a complete multi-language Kaji SDK preview from an OpenAPI file.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use kaji::{ProfileSet, generate_openapi_file};

fn main() -> Result<()> {
    let mut arguments = env::args_os().skip(1);
    let openapi = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: generate_sdk_preview <openapi.json|openapi.yaml> <output-dir>")?;
    let output = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: generate_sdk_preview <openapi.json|openapi.yaml> <output-dir>")?;
    if arguments.next().is_some() {
        bail!("usage: generate_sdk_preview <openapi.json|openapi.yaml> <output-dir>");
    }

    let tree = generate_openapi_file(
        &openapi,
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
