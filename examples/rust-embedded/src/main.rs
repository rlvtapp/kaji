use std::path::Path;

use anyhow::Result;
use poolster::{go, prelude::*, ts};

fn main() -> Result<()> {
    let packages = ProfileSet::new("sdk")
        .package(
            ts::package("typescript")
                .name("@example/notes")
                .with(ts::sdk().fetch().client_name("Notes")),
        )
        .package(go::package("go").with(go::sdk()));

    let files = poolster::generate_openapi(
        Path::new(".poolster/openapi"),
        "Notes",
        "1.0.0",
        packages,
    )?;
    files.write_to("generated")?;
    Ok(())
}
