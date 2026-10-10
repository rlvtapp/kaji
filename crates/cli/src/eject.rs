//! Eject the exact maintained sources compiled into this CLI.
use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
include!(concat!(env!("OUT_DIR"), "/ejected_sources.rs"));

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn run(args: Vec<OsString>) -> Result<()> {
    let mut language = None;
    let mut output = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--language" | "-l") => {
                language = Some(
                    args.next()
                        .context("--language requires a value")?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("language must be UTF-8"))?,
                )
            }
            Some("--out" | "--output" | "-o") => {
                output = Some(PathBuf::from(
                    args.next().context("--out requires a directory")?,
                ))
            }
            Some("--help" | "-h") => {
                println!(
                    "Usage: poolster eject --language <target> --out <new-directory>\n\nWrites editable Rust renderer and runtime sources, core, CLI, plugins and compiler sources. Rebuild the source workspace to apply customizations."
                );
                return Ok(());
            }
            _ => bail!("unknown eject argument: {}", arg.to_string_lossy()),
        }
    }
    let language = language.context("eject requires --language")?;
    let output = output.context("eject requires --out")?;
    eject(&language, &output)?;
    println!(
        "Ejected source workspace to {}. See EJECTED.md for rebuilding and customizing {language}.",
        output.display()
    );
    Ok(())
}

fn eject(language: &str, output: &Path) -> Result<()> {
    let plugin = match language {
        "rust" | "rust-cli" | "typescript" | "typescript-cli" | "go" | "python" | "php"
        | "symfony" | "terraform" | "postman" | "java" | "csharp" | "elixir" | "ruby" | "swift" => {
            language
        }
        _ => bail!("unsupported eject language: {language}"),
    };
    ensure!(
        fs::symlink_metadata(output).is_err(),
        "eject destination already exists: {}",
        output.display()
    );
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staged = tempfile::tempdir_in(parent)?;
    let mut manifest = Vec::new();
    for (name, bytes) in SOURCES {
        let path = staged.path().join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)?;
        manifest.push(serde_json::json!({"path": name, "sha256": digest_hex(bytes)}));
    }
    fs::write(
        staged.path().join("EJECTED-SOURCES.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version": env!("CARGO_PKG_VERSION"), "language":language, "files":manifest}),
        )?,
    )?;
    fs::write(
        staged.path().join("EJECTED.md"),
        format!(
            r#"# Your editable Poolster generator

This is the maintained source workspace embedded in Poolster {}. The selected renderer is `crates/plugins/{plugin}/src/`. All language crates are included because the CLI registers them through `crates/facade/src/lib.rs`; this keeps a complete, rebuildable plugin workspace. Runtime templates next to the Rust renderers are real generator inputs.

## Build and use

Install Rust and Go, then run from this directory:

```sh
cargo build --locked -p poolster-cli
(cd openapi && go build -o ../target/debug/poolster-openapi .)
./target/debug/poolster generate /absolute/path/to/openapi.yaml --language {language} --output /absolute/path/to/sdk
```

The first build downloads dependencies from Cargo/Go registries. No generation or publication is performed by eject. For release builds use `cargo build --locked --release -p poolster-cli` and place the Go compiler beside `target/release/poolster`. Use your existing `poolster.json` with `./target/debug/poolster generate --config /absolute/path/to/poolster.json`.

## Customize the generator

Edit the maintained renderers or runtime source templates in `crates/plugins/{plugin}/src/`, rebuild, then regenerate. Changes affect generated SDKs; customers do not install generator code. To add a plugin, create a Rust crate using `poolster-core`'s plugin interfaces, add its workspace/dependency entries, and register it in `crates/facade/src/lib.rs` and the CLI profile. Existing plugin `src/lib.rs` files provide working registration examples. `docs/internals/typed-plugins.md` and other bundled documentation describe the architecture.

`EJECTED-SOURCES.json` records original SHA-256 hashes. Keep your changes in version control and compare this manifest when upgrading. Eject never overwrites an existing destination. Source files are MIT licensed; retain `LICENSE`. This is source ejection, not a runtime template override flag: use the rebuilt CLI to consume your edits.
"#,
            env!("CARGO_PKG_VERSION")
        ),
    )?;
    // Rename the completed staging directory so failure cannot leave a partial workspace.
    fs::rename(staged.path(), output)
        .with_context(|| format!("install ejected sources at {}", output.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "rebuilds the ejected CLI; needs Cargo dependencies"]
    fn rebuilt_ejected_renderer_consumes_custom_source() -> Result<()> {
        let root = tempfile::tempdir()?;
        let out = root.path().join("sources");
        eject("ruby", &out)?;
        let renderer = out.join("crates/plugins/ruby/src/lib.rs");
        let original = fs::read_to_string(&renderer)?;
        ensure!(original.contains("# Generated by Poolster. Do not edit."));
        fs::write(
            renderer,
            original.replace(
                "# Generated by Poolster. Do not edit.",
                "# Customized in ejected renderer.",
            ),
        )?;
        let build = std::process::Command::new("cargo")
            .args(["build", "--locked", "--offline", "-p", "poolster-cli"])
            .current_dir(&out)
            .env("CARGO_TARGET_DIR", root.path().join("target"))
            .output()?;
        ensure!(
            build.status.success(),
            "ejected build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let binary_dir = root.path().join("target/debug");
        let compiler = std::process::Command::new("go")
            .args(["build", "-o"])
            .arg(binary_dir.join(if cfg!(windows) {
                "poolster-openapi.exe"
            } else {
                "poolster-openapi"
            }))
            .arg(".")
            .current_dir(out.join("openapi"))
            .env("GOCACHE", std::env::temp_dir().join("poolster-go-cache"))
            .output()?;
        ensure!(
            compiler.status.success(),
            "ejected compiler build failed: {}",
            String::from_utf8_lossy(&compiler.stderr)
        );
        let input = root.path().join("api.yaml");
        fs::write(
            &input,
            "openapi: 3.0.3\ninfo:\n  title: EjectedProbe\n  version: 1.0.0\npaths:\n  /probe:\n    get:\n      operationId: getProbe\n      responses:\n        '200':\n          description: OK\n          content:\n            application/json:\n              schema:\n                type: string\n",
        )?;
        let binary = binary_dir.join(if cfg!(windows) {
            "poolster.exe"
        } else {
            "poolster"
        });
        let sdk = root.path().join("sdk");
        let generation = std::process::Command::new(binary)
            .arg("generate")
            .arg(&input)
            .args(["--language", "ruby", "--output"])
            .arg(&sdk)
            .output()?;
        ensure!(
            generation.status.success(),
            "custom generation failed: {}",
            String::from_utf8_lossy(&generation.stderr)
        );
        let mut customized = false;
        fn inspect(path: &Path, customized: &mut bool) -> Result<()> {
            for entry in fs::read_dir(path)? {
                let path = entry?.path();
                if path.is_dir() {
                    inspect(&path, customized)?;
                } else if path.extension().is_some_and(|ext| ext == "rb") {
                    *customized |=
                        fs::read_to_string(path)?.contains("# Customized in ejected renderer.");
                }
            }
            Ok(())
        }
        inspect(&sdk, &mut customized)?;
        ensure!(customized, "generated Ruby did not consume edited renderer");
        Ok(())
    }

    #[test]
    fn bundle_is_complete_hashable_and_never_overwrites() -> Result<()> {
        let root = tempfile::tempdir()?;
        let out = root.path().join("sources");
        eject("ruby", &out)?;
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "LICENSE",
            "crates/cli/build.rs",
            "crates/plugins/ruby/src/lib.rs",
            "openapi/go.mod",
            "packages/internal/sdk-check/check.mjs",
        ] {
            ensure!(out.join(path).is_file(), "missing {path}");
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("EJECTED-SOURCES.json"))?)?;
        for file in manifest["files"].as_array().unwrap() {
            let bytes = fs::read(out.join(file["path"].as_str().unwrap()))?;
            assert_eq!(file["sha256"], digest_hex(bytes));
        }
        fs::write(out.join("LICENSE"), "user edit")?;
        assert!(eject("ruby", &out).is_err());
        assert_eq!(fs::read_to_string(out.join("LICENSE"))?, "user edit");
        assert!(eject("unsupported", &root.path().join("bad")).is_err());
        assert!(!root.path().join("bad").exists());
        Ok(())
    }
}
