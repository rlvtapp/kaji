use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn collect(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || matches!(
                name.as_ref(),
                "assets"
                    | "node_modules"
                    | "target"
                    | "dist"
                    | "build"
                    | "__pycache__"
                    | "kaji-openapi"
                    | "kaji-openapi.exe"
            )
        {
            continue;
        }
        let kind = entry.file_type().expect("source file type");
        if kind.is_dir() {
            collect(root, &path, files);
        } else if kind.is_file() {
            // Source bundles never include binaries or developer caches.
            if fs::metadata(&path).expect("source metadata").len() <= 2 * 1024 * 1024 {
                files.push(path.strip_prefix(root).unwrap().to_owned());
            }
        }
    }
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut files = Vec::new();
    for directory in [
        "crates", "packages", "docs", "schemas", "openapi", "scripts",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(directory).display());
        collect(&root, &root.join(directory), &mut files);
    }
    for file in ["Cargo.toml", "Cargo.lock", "LICENSE"] {
        println!("cargo:rerun-if-changed={}", root.join(file).display());
        files.push(file.into());
    }
    files.sort();
    let mut source = String::from("const SOURCES: &[(&str, &[u8])] = &[\n");
    for file in files {
        source.push_str(&format!(
            "({:?}, include_bytes!({:?})),\n",
            file.to_string_lossy().replace('\\', "/"),
            root.join(&file).to_string_lossy()
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ejected_sources.rs"),
        source,
    )
    .unwrap();
}
