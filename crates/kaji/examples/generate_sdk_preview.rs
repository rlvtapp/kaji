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
            .package(kaji::rust::package("rust").with(kaji::rust::sdk()))
            .package(kaji::ts::package("typescript-fetch").with(kaji::ts::sdk().fetch()))
            .package(kaji::ts::package("typescript-axios").with(kaji::ts::sdk().axios()))
            .package(kaji::go::package("go").with(kaji::go::sdk()))
            .package(kaji::python::package("python").with(kaji::python::sdk()))
            .package(kaji::php::package("php").with(kaji::php::sdk()))
            .package(kaji::java::package("java").with(kaji::java::sdk()))
            .package(kaji::dotnet::package("dotnet").with(kaji::dotnet::sdk()))
            .package(kaji::elixir::package("elixir").with(kaji::elixir::sdk())),
    )?;
    let count = tree.iter().count();
    tree.write_to(&output)?;
    println!("Wrote {count} generated files to {}", output.display());
    Ok(())
}
