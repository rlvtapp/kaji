//! Manifest implementation for generated rust-cli packages.

pub(super) fn cargo_toml(package: &str, command: &str, version: &str) -> String {
    format!(
        "[workspace]\n\n[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2024\"\ndescription = \"Generated API CLI\"\n\n[[bin]]\nname = {command:?}\npath = \"src/main.rs\"\n\n[dependencies]\nanyhow = \"1\"\nclap = {{ version = \"4\", features = [\"std\"] }}\ndialoguer = \"0.11\"\nreqwest = {{ version = \"0.12\", default-features = false, features = [\"blocking\", \"json\", \"rustls-tls\"] }}\nserde_json = \"1\"\n"
    )
}
