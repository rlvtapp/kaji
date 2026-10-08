//! Cap'n Proto inspection uses the official compiler's schema descriptors.
use anyhow::{Context, Result, bail};
use capnp::schema_capnp::{code_generator_request, node};
use kaji_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};
use std::{io::Cursor, path::Path, process::Command};

#[derive(Debug, Clone)]
pub struct CapnProtoDocument {
    /// Serialized CodeGeneratorRequest retains all schema nodes and imports.
    pub schema_request: Vec<u8>,
    summary: ContractSummary,
}

pub fn load(path: &Path) -> Result<CapnProtoDocument> {
    load_with_includes(path, &[])
}

/// Compile with explicit roots for absolute schema imports.
pub fn load_with_includes(
    path: &Path,
    includes: &[std::path::PathBuf],
) -> Result<CapnProtoDocument> {
    let path = path
        .canonicalize()
        .with_context(|| format!("reading Cap'n Proto input {}", path.display()))?;
    let parent = path
        .parent()
        .context("Cap'n Proto input needs a parent directory")?;
    let mut command = Command::new("capnp");
    command
        .args(["compile", "-o-"])
        .arg("--src-prefix")
        .arg(parent)
        .arg("-I")
        .arg(parent);
    for include in includes {
        command.arg("-I").arg(include);
    }
    let output = command.arg(&path).output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound { anyhow::anyhow!("Cap'n Proto inspection requires the official `capnp` compiler on PATH; install Cap'n Proto and retry") } else { anyhow::anyhow!(error).context("running Cap'n Proto compiler") }
    })?;
    if !output.status.success() {
        bail!(
            "Cap'n Proto compilation failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let title = path
        .file_name()
        .context("Cap'n Proto input needs a filename")?
        .to_string_lossy()
        .into_owned();
    CapnProtoDocument::from_schema_request(output.stdout, title)
}

fn summarize(bytes: &[u8], title: String) -> Result<ContractSummary> {
    let message = capnp::serialize::read_message(
        &mut Cursor::new(bytes),
        capnp::message::ReaderOptions::new(),
    )
    .context("decoding Cap'n Proto compiler schema request")?;
    let request = message.get_root::<code_generator_request::Reader<'_>>()?;
    let mut types = Vec::new();
    let mut operations = Vec::new();
    for schema in request.get_nodes()? {
        let name = schema.get_display_name()?.to_str()?.to_owned();
        match schema.which()? {
            node::Struct(_) | node::Enum(_) => types.push(name),
            node::Interface(interface) => {
                types.push(name.clone());
                for method in interface.get_methods()? {
                    operations.push(OperationSummary {
                        name: format!("{name}.{}", method.get_name()?.to_str()?),
                        kind: if method.get_result_struct_type() == 0x995f9a3377c0b16e {
                            "capability_streaming".into()
                        } else {
                            "capability_rpc".into()
                        },
                    });
                }
            }
            _ => {}
        }
    }
    types.sort();
    operations.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ContractSummary {
        format: "capnproto".into(),
        title,
        version: None,
        types,
        operations,
    })
}

impl CapnProtoDocument {
    /// Inspect an official compiler descriptor request without requiring the compiler.
    pub fn from_schema_request(schema_request: Vec<u8>, title: String) -> Result<Self> {
        let summary = summarize(&schema_request, title)?;
        Ok(Self {
            schema_request,
            summary,
        })
    }
    pub fn summary(&self) -> ContractSummary {
        self.summary.clone()
    }
}

impl kaji_core::engine::Contract for CapnProtoDocument {
    const NAME: &'static str = "kaji.capnproto";
}

/// Native capnproto input provider.
pub struct CapnProtoInput;
impl kaji_core::input::InputPlugin for CapnProtoInput {
    fn id(&self) -> &str {
        "capnproto.capnp"
    }
    fn format(&self) -> &str {
        "capnproto"
    }
    fn load(&self, path: &std::path::Path) -> anyhow::Result<kaji_core::input::InputContract> {
        let document = load(path)?;
        let mut input = kaji_core::input::InputContract::new(document.summary());

        input.publish(document)?;
        Ok(input)
    }
}

#[cfg(test)]
mod descriptor_edge_tests {
    use super::*;

    fn encode(message: &capnp::message::Builder<capnp::message::HeapAllocator>) -> Vec<u8> {
        let mut bytes = Vec::new();
        capnp::serialize::write_message(&mut bytes, message).unwrap();
        bytes
    }

    #[test]
    fn file_and_constant_nodes_are_not_types_or_operations() {
        let mut message = capnp::message::Builder::new_default();
        let request = message.init_root::<code_generator_request::Builder<'_>>();
        let mut nodes = request.init_nodes(2);
        let mut file = nodes.reborrow().get(0);
        file.set_display_name("empty.capnp");
        file.set_file(());
        let mut constant = nodes.get(1);
        constant.set_display_name("empty.capnp:answer");
        constant.init_const();
        let summary = summarize(&encode(&message), "empty.capnp".into()).unwrap();
        assert!(summary.types.is_empty());
        assert!(summary.operations.is_empty());
        assert_eq!(summary.title, "empty.capnp");
    }

    #[test]
    fn methods_and_types_are_sorted_deterministically() {
        let mut message = capnp::message::Builder::new_default();
        let request = message.init_root::<code_generator_request::Builder<'_>>();
        let mut nodes = request.init_nodes(2);
        let mut interface = nodes.reborrow().get(0);
        interface.set_display_name("schema.capnp:Zebra");
        let mut methods = interface.init_interface().init_methods(2);
        methods.reborrow().get(0).set_name("z");
        methods.get(1).set_name("a");
        let mut model = nodes.get(1);
        model.set_display_name("schema.capnp:Alpha");
        model.init_struct();
        let summary = summarize(&encode(&message), "schema.capnp".into()).unwrap();
        assert_eq!(summary.types, ["schema.capnp:Alpha", "schema.capnp:Zebra"]);
        assert_eq!(
            summary
                .operations
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>(),
            ["schema.capnp:Zebra.a", "schema.capnp:Zebra.z"]
        );
    }

    #[test]
    fn truncated_serialized_request_is_rejected() {
        let mut message = capnp::message::Builder::new_default();
        message
            .init_root::<code_generator_request::Builder<'_>>()
            .init_nodes(1)
            .get(0)
            .set_display_name("schema.capnp:Item");
        let bytes = encode(&message);
        assert!(summarize(&bytes[..bytes.len() - 8], "schema.capnp".into()).is_err());
    }

    #[test]
    fn invalid_utf8_schema_name_is_rejected() {
        let mut message = capnp::message::Builder::new_default();
        let request = message.init_root::<code_generator_request::Builder<'_>>();
        let schema = request.init_nodes(1).get(0);
        schema.init_display_name(1).as_bytes_mut()[0] = 0xff;
        assert!(summarize(&encode(&message), "schema.capnp".into()).is_err());
    }

    #[test]
    fn native_contract_preserves_request_and_publishes_summary() {
        let mut message = capnp::message::Builder::new_default();
        message
            .init_root::<code_generator_request::Builder<'_>>()
            .init_nodes(0);
        let bytes = encode(&message);
        let summary = summarize(&bytes, "empty.capnp".into()).unwrap();
        let document = CapnProtoDocument {
            schema_request: bytes.clone(),
            summary: summary.clone(),
        };
        let mut input = kaji_core::input::InputContract::new(summary.clone());
        input.publish(document).unwrap();
        let native = input.get::<CapnProtoDocument>().unwrap();
        assert_eq!(native.schema_request, bytes);
        assert_eq!(native.summary(), summary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_request_preserves_imported_types_and_capability_methods() {
        let mut message = capnp::message::Builder::new_default();
        {
            let request = message.init_root::<code_generator_request::Builder<'_>>();
            let mut nodes = request.init_nodes(3);
            let mut imported = nodes.reborrow().get(0);
            imported.set_id(1);
            imported.set_display_name("common.capnp:Item");
            imported.init_struct();
            let mut enumeration = nodes.reborrow().get(1);
            enumeration.set_id(2);
            enumeration.set_display_name("common.capnp:State");
            enumeration.init_enum();
            let mut interface = nodes.reborrow().get(2);
            interface.set_id(3);
            interface.set_display_name("service.capnp:Store");
            let mut method = interface.init_interface().init_methods(1).get(0);
            method.set_name("get");
            method.set_param_struct_type(1);
            method.set_result_struct_type(1);
        }
        let mut bytes = Vec::new();
        capnp::serialize::write_message(&mut bytes, &message).unwrap();
        let summary = summarize(&bytes, "service.capnp".into()).unwrap();
        assert_eq!(
            summary.types,
            [
                "common.capnp:Item",
                "common.capnp:State",
                "service.capnp:Store"
            ]
        );
        assert_eq!(
            summary.operations,
            [OperationSummary {
                name: "service.capnp:Store.get".into(),
                kind: "capability_rpc".into()
            }]
        );
    }

    #[test]
    fn rejects_invalid_binary_schema_request() {
        assert!(summarize(b"invalid", "broken.capnp".into()).is_err());
    }
}
