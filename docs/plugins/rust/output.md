# Build a Rust output plugin

← [Rust plugins](README.md)

This executable example publishes a custom contract and consumes it to generate
an owned file. It uses a small documentation language so the contract graph is
visible without a full SDK renderer.

## Create a crate

Next to your Poolster checkout, create a Rust binary with these dependencies:

```toml
[dependencies]
anyhow = "1"
poolster-core = { path = "../poolster/crates/core" }
```

Replace the path with your checkout location. Put this in `src/main.rs`:

```rust
use anyhow::Result;
use poolster_core::{Api, GeneratedFile};
use poolster_core::engine::{
    Contract, Handle, Language, Meta, Package, Packages, Plugin,
    PluginContext, Provision, Requirement,
};

struct Documentation;
impl Language for Documentation {
    const NAME: &'static str = "documentation";
    type Settings = ();
    type Workspace = ();
}

struct Inventory(Vec<String>);
impl Contract for Inventory {
    const NAME: &'static str = "example.inventory.v1";
}

struct Source { meta: Meta }
impl Plugin<Documentation> for Source {
    fn kind(&self) -> &'static str { "example.source" }
    fn meta(&self) -> &Meta { &self.meta }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Inventory>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Documentation>) -> Result<()> {
        cx.publish(Inventory(vec!["Coffee".into(), "Tea".into()]))
    }
}

struct Report { meta: Meta, source: Handle<Inventory> }
impl Plugin<Documentation> for Report {
    fn kind(&self) -> &'static str { "example.report" }
    fn meta(&self) -> &Meta { &self.meta }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Documentation>) -> Result<()> {
        let inventory = cx.inputs.get::<Inventory>()?;
        cx.files.emit(GeneratedFile::new(
            "inventory.txt", inventory.0.join("\n") + "\n",
        )?)
    }
}

fn main() -> Result<()> {
    let source = Source { meta: Meta::new() };
    let report = Report { meta: Meta::new(), source: source.meta.handle() };
    let tree = Packages::new()
        .package(Package::<Documentation>::new("reports")
            .with(report).with(source))
        .generate(&Api::default(), None)?;
    assert_eq!(tree.get("reports/inventory.txt"), Some("Coffee\nTea\n"));
    tree.write_to(std::path::Path::new("generated"))?;
    Ok(())
}
```

Run `cargo run`. The file `generated/reports/inventory.txt` contains two lines.
The consumer was added first, but its requirement causes the producer to run first.
The empty `Api` supplies the legacy generation context; the custom contract in
this example is not an HTTP API and does not need decomposition into blocks.

## Adapt this to a maintained language

Implement `Plugin<poolster_plugin_typescript::TypeScript>` or another language
instead of `Plugin<Documentation>`, then install it in that language's package.
Use its workspace APIs when registering dependencies, exports or symbols. Writing
an extra source file alone does not add it to a package barrel automatically.

For a native-only pipeline, declare `supports_native_input` and use declared
contracts instead of legacy `cx.api`. Select the input provider and its handle as
shown in the [Rust SDK quickstart](../../rust/quickstart.md).

## Test the plugin

Assert the generated tree before writing: file paths, contents and contract-derived
behavior are observable without disk mutation. Exercise a substitute provider,
then generate twice to check stable paths and ownership. If the plugin emits
compilable source, compile the generated package and execute a representative
operation; an output snapshot does not check runtime semantics.

The [custom-plugin example](../../../examples/custom-plugin/README.md) includes a
provider substitution test you can run with `cargo test --manifest-path
examples/custom-plugin/Cargo.toml` from the Poolster checkout.

## Share it with other authors

Put the contract type in a shared crate and export it from both producer and
consumer APIs. Keep one `Meta` for each plugin instance and expose a typed
`Handle<Inventory>` from a factory or method. The stable contract name is useful
for diagnostics; it does not replace Rust type identity.

Use a new compatible contract version when you change its semantics. Two distinct
Rust types with the same stable contract name produce a compatibility diagnostic.
A plugin crate must be linked and registered in a host application; the stock CLI
does not dynamically discover arbitrary crates.

**Next:** [Block handlers](handlers.md) · [Custom contracts](contracts.md) ·
[Provider and language interfaces](interfaces.md)
