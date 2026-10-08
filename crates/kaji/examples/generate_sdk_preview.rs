//! Generates a complete multi-language Poolster SDK preview from Poolster Go compiler artifacts.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use poolster::{ProfileSet, generate_openapi};

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
            .package(poolster::rust::package("rust").with(poolster::rust::sdk()))
            .package(poolster::ts::package("typescript-fetch").with(poolster::ts::sdk().fetch()))
            .package(poolster::ts::package("typescript-axios").with(poolster::ts::sdk().axios()))
            .package(poolster::go::package("go").with(poolster::go::sdk()))
            .package(poolster::python::package("python").with(poolster::python::sdk()))
            .package(poolster::php::package("php").with(poolster::php::sdk()))
            .package(poolster::java::package("java").with(poolster::java::sdk()))
            .package(poolster::csharp::package("csharp").with(poolster::csharp::sdk()))
            .package(poolster::elixir::package("elixir").with(poolster::elixir::sdk())),
    )?;
    let count = tree.iter().count();
    tree.write_to(&output)?;
    println!("Wrote {count} generated files to {}", output.display());
    Ok(())
}
