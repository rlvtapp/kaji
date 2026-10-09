//! Go gRPC code generation using the pinned official Protobuf tools.
use anyhow::{Context, Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
    native::rpc::RpcContract,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const PROTOC_VERSION: &str = "34.2";
pub const GO_PROTOBUF_VERSION: &str = "1.36.11";
pub const GO_GRPC_PLUGIN_VERSION: &str = "1.6.2";
pub const GO_GRPC_VERSION: &str = "1.83.2";

#[derive(Clone, Debug)]
pub struct GrpcToolchain {
    pub protoc: PathBuf,
    pub protoc_gen_go: PathBuf,
    pub protoc_gen_go_grpc: PathBuf,
}
impl Default for GrpcToolchain {
    fn default() -> Self {
        Self {
            protoc: "protoc".into(),
            protoc_gen_go: "protoc-gen-go".into(),
            protoc_gen_go_grpc: "protoc-gen-go-grpc".into(),
        }
    }
}
/// Official message types, gRPC client stubs and server interfaces in one module.
pub struct Grpc {
    meta: Meta,
    input: Option<Handle<RpcContract>>,
    module_path: String,
    tools: GrpcToolchain,
    go_packages: BTreeMap<String, String>,
}
pub fn grpc(module_path: impl Into<String>) -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
        module_path: module_path.into(),
        tools: Default::default(),
        go_packages: BTreeMap::new(),
    }
}
impl Grpc {
    pub fn input(mut self, input: Handle<RpcContract>) -> Self {
        self.input = Some(input);
        self
    }
    pub fn toolchain(mut self, tools: GrpcToolchain) -> Self {
        self.tools = tools;
        self
    }
    /// Explicit equivalent of protoc's Mfile.proto=import/path;package mapping.
    pub fn go_package(mut self, file: impl Into<String>, package: impl Into<String>) -> Self {
        self.go_packages.insert(file.into(), package.into());
        self
    }
}
fn portable_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\r', '\n', ' ', '='])
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}
fn validate(
    contract: &RpcContract,
    module: &str,
    mappings: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    ensure!(
        portable_name(module),
        "gRPC Go module must be a portable import path"
    );
    ensure!(
        !contract.root_files.is_empty(),
        "gRPC input has no root files"
    );
    ensure!(
        !contract.services.is_empty(),
        "gRPC input has no RPC services"
    );
    ensure!(
        !contract.descriptor_set.is_empty(),
        "gRPC input has no descriptor set"
    );
    let mut generation = Vec::new();
    for root in &contract.root_files {
        ensure!(
            contract.files.iter().any(|f| &f.name == root),
            "missing root descriptor {root}"
        );
    }
    for name in mappings.keys() {
        ensure!(
            contract.files.iter().any(|f| &f.name == name),
            "Go package mapping refers to unknown file {name}"
        );
    }
    let mut package_names = BTreeMap::new();
    for file in &contract.files {
        ensure!(
            portable_name(&file.name) && file.name.ends_with(".proto"),
            "unsafe Protobuf file name {:?}",
            file.name
        );
        // Google's built-ins come from the pinned protobuf runtime, not this module.
        if file.name.starts_with("google/protobuf/") && file.source.is_none() {
            ensure!(
                !mappings.contains_key(&file.name),
                "overriding built-in Go package mappings is unsupported: {}",
                file.name
            );
            continue;
        }
        let mapping = mappings
            .get(&file.name)
            .or(file.go_package.as_ref())
            .with_context(|| {
                format!(
                    "{} needs option go_package or an explicit Go package mapping",
                    file.name
                )
            })?;
        let mut parts = mapping.split(';');
        let import = parts.next().unwrap();
        let package = parts.next();
        ensure!(
            parts.next().is_none() && portable_name(import),
            "invalid Go package mapping for {}",
            file.name
        );
        if let Some(package) = package {
            ensure!(
                !package.is_empty()
                    && package.chars().enumerate().all(|(i, c)| c == '_'
                        || c.is_ascii_alphabetic()
                        || (i > 0 && c.is_ascii_digit())),
                "invalid Go package name for {}",
                file.name
            );
        }
        ensure!(
            import == module || import.starts_with(&format!("{module}/")),
            "Go package for {} ({import}) lies outside module {module}",
            file.name
        );
        if let Some(package) = package {
            if let Some(old) = package_names.insert(import, package) {
                ensure!(old == package, "conflicting Go package names for {import}");
            }
        }
        generation.push(file.name.clone());
    }
    generation.sort();
    generation.dedup();
    ensure!(
        !generation.is_empty(),
        "gRPC input has no local files to generate"
    );
    Ok(generation)
}
fn tool_version(program: &Path, expected: &str) -> Result<PathBuf> {
    // Resolve executables before changing process directories; relative configured paths
    // remain relative to the caller's working directory.
    let executable = if program.is_absolute() {
        program.to_path_buf()
    } else if program.components().count() > 1 {
        fs::canonicalize(program)?
    } else {
        let executable_name = if cfg!(windows) && program.extension().is_none() {
            program.with_extension("exe")
        } else {
            program.to_path_buf()
        };
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join(&executable_name))
            .find(|candidate| candidate.is_file())
            .with_context(|| format!("find pinned gRPC generator {} on PATH", program.display()))?
    };
    let output = Command::new(&executable)
        .arg("--version")
        .output()
        .with_context(|| {
            format!(
                "start {} (install pinned gRPC generator {expected})",
                program.display()
            )
        })?;
    ensure!(
        output.status.success(),
        "{} --version failed",
        program.display()
    );
    let actual = String::from_utf8(output.stdout)?.trim().to_owned();
    ensure!(
        actual == expected,
        "gRPC tool version mismatch for {}: expected {expected:?}, got {actual:?}",
        program.display()
    );
    Ok(executable)
}
impl Plugin<crate::Go> for Grpc {
    fn kind(&self) -> &'static str {
        "go-grpc"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.input)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Go>) -> Result<()> {
        let contract = cx.inputs.get::<RpcContract>()?;
        let generation = validate(contract, &self.module_path, &self.go_packages)?;
        let protoc = tool_version(&self.tools.protoc, &format!("libprotoc {PROTOC_VERSION}"))?;
        let go = tool_version(
            &self.tools.protoc_gen_go,
            &format!("protoc-gen-go v{GO_PROTOBUF_VERSION}"),
        )?;
        let grpc = tool_version(
            &self.tools.protoc_gen_go_grpc,
            &format!("protoc-gen-go-grpc {GO_GRPC_PLUGIN_VERSION}"),
        )?;
        let temporary = tempfile::tempdir()?;
        let descriptors = temporary.path().join("input.pb");
        let output = temporary.path().join("output");
        fs::create_dir(&output)?;
        fs::write(&descriptors, &contract.descriptor_set)?;
        let mut command = Command::new(protoc);
        command
            .arg(format!("--descriptor_set_in={}", descriptors.display()))
            .arg(format!("--plugin=protoc-gen-go={}", go.display()))
            .arg(format!("--plugin=protoc-gen-go-grpc={}", grpc.display()))
            .arg(format!("--go_out={}", output.display()))
            .arg(format!("--go-grpc_out={}", output.display()))
            .arg(format!("--go_opt=module={}", self.module_path))
            .arg(format!("--go-grpc_opt=module={}", self.module_path));
        for (file, package) in &self.go_packages {
            command
                .arg(format!("--go_opt=M{file}={package}"))
                .arg(format!("--go-grpc_opt=M{file}={package}"));
        }
        command.args(&generation);
        let result = command
            .output()
            .context("run pinned official Go gRPC generators")?;
        ensure!(
            result.status.success(),
            "Go gRPC code generation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        fn emit(
            directory: &Path,
            root: &Path,
            files: &mut poolster_core::engine::Emitter<'_>,
        ) -> Result<()> {
            let mut paths = fs::read_dir(directory)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            paths.sort();
            for path in paths {
                let metadata = fs::symlink_metadata(&path)?;
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "gRPC generator emitted a symlink"
                );
                if metadata.is_dir() {
                    emit(&path, root, files)?;
                } else {
                    ensure!(
                        metadata.is_file() && path.extension().is_some_and(|e| e == "go"),
                        "gRPC generator emitted unsupported output"
                    );
                    files.emit(GeneratedFile::new(
                        path.strip_prefix(root)?,
                        fs::read_to_string(&path)?,
                    )?)?;
                }
            }
            Ok(())
        }
        emit(&output, &output, &mut cx.files)?;
        let manifest = include_str!("../templates/grpc_go.mod.tmpl")
            .replace("{{MODULE_PATH}}", &self.module_path)
            .replace("{{GRPC_VERSION}}", GO_GRPC_VERSION)
            .replace("{{PROTOBUF_VERSION}}", GO_PROTOBUF_VERSION);
        cx.files.emit(GeneratedFile::new("go.mod", manifest)?)?;
        cx.files.emit(GeneratedFile::new(
            "go.sum",
            include_str!("../templates/grpc_go.sum.tmpl"),
        )?)?;
        for file in &contract.files {
            if let Some(source) = &file.source {
                cx.files.emit(GeneratedFile::new(
                    Path::new("proto").join(&file.name),
                    source,
                )?)?;
            }
        }
        cx.files.emit(GeneratedFile::new("README.md",format!("# Go gRPC package\n\nGenerated by Poolster using protoc {PROTOC_VERSION}, protoc-gen-go {GO_PROTOBUF_VERSION}, and protoc-gen-go-grpc {GO_GRPC_PLUGIN_VERSION}.\n\nThe generated `.pb.go` files expose official message types. `_grpc.pb.go` files expose client constructors, server interfaces, registration functions and all unary/client/server/bidirectional streaming methods. Embed the generated `Unimplemented<Service>Server` in your server implementation. Configure transport credentials when constructing a `grpc.ClientConn`; transport security, status errors, contexts and deadlines follow grpc-go.\n\nRun `go test ./...` with Go 1.25 or newer. The generated module includes pinned direct/indirect dependencies and checksums; normal compilation can download those dependencies but does not rewrite its manifests. Source documents remain under `proto/`. Regenerate through the same Poolster recipe with the pinned toolchain; generated files use ordinary Poolster ownership/customization.\n\nMessage field presence, oneofs, maps, defaults and wire numbers are supplied to official generators through the retained FileDescriptorSet. Poolster RPC metadata describes services and streaming; it does not reinterpret Protobuf messages as HTTP schemas. Protobuf editions are not supported by the current input parser.\n"))?)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use poolster_core::native::rpc::{RpcFile, RpcService};
    fn contract() -> RpcContract {
        RpcContract {
            root_files: vec!["api.proto".into()],
            descriptor_set: vec![1],
            files: vec![RpcFile {
                name: "api.proto".into(),
                package: "demo".into(),
                syntax: "proto3".into(),
                go_package: Some("example.com/demo/api;api".into()),
                imports: vec![],
                source: Some(String::new()),
            }],
            services: vec![RpcService {
                full_name: "demo.Store".into(),
                file: "api.proto".into(),
                methods: vec![],
            }],
        }
    }
    #[test]
    fn malformed_contract_names_and_unknown_mappings_are_rejected() {
        let mut c = contract();
        c.files[0].name = "../outside.proto".into();
        assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
        let c = contract();
        assert!(
            validate(
                &c,
                "example.com/demo",
                &BTreeMap::from([("unknown.proto".into(), "example.com/demo/api".into())])
            )
            .is_err()
        );
        let mut c = c;
        c.descriptor_set.clear();
        assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
        let mut c = contract();
        c.services.clear();
        assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
    }

    #[test]
    fn mappings_and_modules_fail_before_tools_run() {
        let c = contract();
        assert_eq!(
            validate(&c, "example.com/demo", &BTreeMap::new()).unwrap(),
            vec!["api.proto"]
        );
        assert!(validate(&c, "../escape", &BTreeMap::new()).is_err());
        assert!(validate(&c, "example.com/other", &BTreeMap::new()).is_err());
        let mut c = c;
        c.files[0].go_package = None;
        assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
        assert!(
            validate(
                &c,
                "example.com/demo",
                &BTreeMap::from([("api.proto".into(), "example.com/demo/api;api".into())])
            )
            .is_ok()
        );
    }
}
