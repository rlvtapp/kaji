fn main() -> anyhow::Result<()> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "generated".into());
    kaji_custom_plugin_example::generate(&kaji_core::Api {
        name: "Example API".into(),
        ..Default::default()
    })?
    .write_to(std::path::Path::new(&output))?;
    Ok(())
}
