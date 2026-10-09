use super::*;
use metadata::{parse_auth, parse_discover, parse_download, parse_show, parse_update};
use servers::{parse_check, parse_init, parse_mcp, parse_mock};

mod metadata;
mod servers;

pub(super) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = arguments.into_iter();
    let Some(command) = args.next() else {
        return Ok(Action::Help);
    };
    if command == "--help" || command == "-h" || command == "help" {
        return Ok(Action::Help);
    }
    if command == "--version" || command == "-V" {
        return Ok(Action::Version);
    }
    if command == "init" {
        return parse_init(args);
    }
    if command == "contract" {
        return Ok(Action::Contract(args.collect()));
    }
    if command == "migrate" {
        return Ok(Action::Migrate(args.collect()));
    }
    if command == "eject" {
        return Ok(Action::Eject(args.collect()));
    }
    if command == "sdk" {
        return sdk_automation::parse(args).map(Action::Sdk);
    }
    if command == "mcp" {
        return parse_mcp(args);
    }
    if command == "mock" {
        return parse_mock(args);
    }
    if command == "check" {
        return parse_check(args);
    }
    if command == "show" {
        return parse_show(args);
    }
    if command == "update" {
        return parse_update(args);
    }
    if command == "auth" {
        return parse_auth(args);
    }
    if command == "discover" {
        return parse_discover(args);
    }
    if command == "download" {
        return parse_download(args);
    }
    if command == "languages" {
        if args.next().is_some() {
            bail!("languages does not accept arguments")
        }
        return Ok(Action::Languages);
    }
    if command != "generate" {
        bail!("unknown command {:?}; run poolster --help", command)
    }
    let mut options = Generate {
        native_output: NativeOutputConfig::default(),
        native_input: None,
        source: None,
        config: None,
        config_packages: None,
        artifacts: None,
        output: PathBuf::new(),
        languages: Vec::new(),
        name: "API".into(),
        version: "0.1.0".into(),
        style: SdkClientStyle::Namespaced,
        raw: false,
        typescript_transport: None,
        client_name: None,
        compiler: None,
        path_selection: PathSelection::default(),
        config_sha256: None,
        source_sha256: None,
        jobs: 0,
        color: ColorChoice::Auto,
        check: false,
        json_changes: false,
    };
    let mut native_format = None;
    let mut explicit_client_style = false;
    let mut native_provider = None;
    let mut native_options = poolster_core::input::InputOptions::default();
    while let Some(argument) = args.next() {
        let text = argument.to_string_lossy();
        if text == "--help" || text == "-h" {
            return Ok(Action::Help);
        }
        if !text.starts_with('-') {
            if options
                .source
                .replace(OpenApiInput::Path(PathBuf::from(argument)))
                .is_some()
            {
                bail!("only one OpenAPI source may be supplied")
            }
            continue;
        }
        let flag = text.as_ref();
        if flag == "--raw-sdk" {
            options.raw = true;
            continue;
        }
        if flag == "--check" {
            options.check = true;
            continue;
        }
        if flag == "--json" {
            options.json_changes = true;
            continue;
        }
        if flag == "--format" {
            let value = args.next().context("--format requires human or json")?;
            options.json_changes = match value.to_str() {
                Some("json") => true,
                Some("human") => false,
                _ => bail!("--format requires human or json"),
            };
            continue;
        }
        if !matches!(
            flag,
            "--input-format"
                | "--provider"
                | "--operation"
                | "--import-root"
                | "--broker-config"
                | "--workflow-source"
                | "--module"
                | "--protoc"
                | "--protoc-gen-go"
                | "--protoc-gen-go-grpc"
                | "--go-package"
                | "--output"
                | "-o"
                | "--language"
                | "-l"
                | "--name"
                | "--sdk-version"
                | "--client-style"
                | "--typescript-transport"
                | "--typescript-surface"
                | "--typescript-client-name"
                | "--jobs"
                | "--artifacts"
                | "--openapi-compiler"
                | "--include-path"
                | "--exclude-path"
                | "--config"
                | "--color"
        ) {
            bail!("unknown option {flag}; run poolster --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        match flag {
            "--input-format" => {
                native_format = Some(
                    value
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--input-format requires UTF-8"))?,
                )
            }
            "--provider" => {
                native_provider = Some(
                    value
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--provider requires UTF-8"))?,
                )
            }
            "--module" => {
                options.native_output.module = Some(
                    value
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--module requires UTF-8"))?,
                )
            }
            "--protoc" => options.native_output.toolchain.protoc = Some(value.into()),
            "--protoc-gen-go" => options.native_output.toolchain.protoc_gen_go = Some(value.into()),
            "--protoc-gen-go-grpc" => {
                options.native_output.toolchain.protoc_gen_go_grpc = Some(value.into())
            }
            "--go-package" => {
                let value = value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--go-package requires UTF-8 file=mapping"))?;
                let (file, mapping) = value
                    .split_once('=')
                    .context("--go-package requires file=mapping")?;
                ensure!(
                    !file.is_empty() && !mapping.is_empty(),
                    "--go-package requires nonempty file=mapping"
                );
                ensure!(
                    options
                        .native_output
                        .go_packages
                        .insert(file.into(), mapping.into())
                        .is_none(),
                    "duplicate Go package mapping {file:?}"
                );
            }
            "--operation" => native_options.operation_files.push(value.into()),
            "--import-root" => native_options
                .import_roots
                .push(std::env::current_dir()?.join(value)),
            "--broker-config" => {
                let path = PathBuf::from(value);
                native_options.broker = Some(
                    serde_json::from_slice(
                        &std::fs::read(&path)
                            .with_context(|| format!("read broker config {}", path.display()))?,
                    )
                    .context("broker config must be JSON")?,
                );
            }
            "--workflow-source" => {
                let value = value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--workflow-source requires UTF-8 name=path"))?;
                let (name, path) = value
                    .split_once('=')
                    .context("--workflow-source requires name=path")?;
                ensure!(
                    !name.is_empty() && !path.is_empty(),
                    "--workflow-source requires nonempty name=path"
                );
                ensure!(
                    native_options
                        .workflow_sources
                        .insert(name.into(), std::env::current_dir()?.join(path))
                        .is_none(),
                    "duplicate workflow source {name:?}"
                );
            }
            "--output" | "-o" => options.output = value.into(),
            "--artifacts" => options.artifacts = Some(value.into()),
            "--openapi-compiler" => options.compiler = Some(value.into()),
            "--config" => options.config = Some(value.into()),
            _ => {
                let value = value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("{flag} requires UTF-8 text"))?;
                if value.trim().is_empty() {
                    bail!("{flag} cannot be empty")
                }
                match flag {
                    "--language" | "-l" => {
                        for target in value.split(',') {
                            let targets = if target == "all" {
                                SDK_LANGUAGES
                            } else {
                                std::slice::from_ref(&target)
                            };
                            for target in targets {
                                if !LANGUAGES.contains(target) {
                                    bail!("unknown target {target:?}; run poolster languages")
                                }
                                if !options.languages.iter().any(|existing| existing == target) {
                                    options.languages.push((*target).into());
                                }
                            }
                        }
                    }
                    "--name" => options.name = value,
                    "--sdk-version" => options.version = value,
                    "--typescript-transport" => {
                        options.typescript_transport = Some(TypeScriptTransport::parse(&value)?);
                    }
                    "--typescript-client-name" => options.client_name = Some(value),
                    "--include-path" => options.path_selection.include.push(value),
                    "--exclude-path" => options.path_selection.exclude.push(value),
                    "--color" => options.color = ColorChoice::parse(&value)?,
                    "--jobs" => {
                        options.jobs =
                            value.parse().context("--jobs must be a positive integer")?;
                        if options.jobs == 0 {
                            bail!("--jobs must be a positive integer")
                        }
                    }
                    "--client-style" => {
                        explicit_client_style = true;
                        options.style = match value.as_str() {
                            "flat" => SdkClientStyle::Flat,
                            "namespaced" | "idiomatic" => SdkClientStyle::Namespaced,
                            _ => bail!("--client-style must be flat or namespaced"),
                        }
                    }
                    "--typescript-surface" => {
                        options.raw = match value.as_str() {
                            "raw" => true,
                            "client" => false,
                            _ => bail!("--typescript-surface must be client or raw"),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    if let Some(format) = native_format {
        let path = match options.source.take() {
            Some(OpenApiInput::Path(path)) => path,
            _ => bail!("--input-format requires a native source file"),
        };
        options.native_input = Some(NativeInputConfig {
            format,
            provider: native_provider,
            path,
            options: native_options,
        });
    } else if native_provider.is_some()
        || native_options != poolster_core::input::InputOptions::default()
    {
        bail!("provider and operation/import options require --input-format");
    }
    ensure!(
        options.native_output == NativeOutputConfig::default()
            || options
                .native_input
                .as_ref()
                .is_some_and(|input| input.format == "protobuf"),
        "gRPC output options require --input-format protobuf"
    );
    let direct_mode = options.native_input.is_some()
        || options.source.is_some()
        || options.artifacts.is_some()
        || !options.output.as_os_str().is_empty()
        || !options.languages.is_empty()
        || options.raw
        || options.typescript_transport.is_some()
        || options.client_name.is_some()
        || options.compiler.is_some()
        || !options.path_selection.include.is_empty()
        || !options.path_selection.exclude.is_empty()
        || options.jobs != 0;
    if options.config.is_some() || !direct_mode {
        if direct_mode {
            bail!("--config cannot be combined with direct generation options")
        }
        options.config.get_or_insert_with(default_config_path);
        return Ok(Action::Generate(Box::new(options)));
    }
    if options.output.as_os_str().is_empty() {
        bail!("--output is required")
    }
    if options.languages.is_empty() {
        bail!("at least one --language is required")
    }
    if (options.source.is_some() || options.native_input.is_some()) == options.artifacts.is_some() {
        bail!("supply exactly one OpenAPI file or --artifacts directory")
    }
    if options.artifacts.is_some() && options.compiler.is_some() {
        bail!("--openapi-compiler cannot be used with --artifacts")
    }
    validate_path_selection(&options.path_selection)?;
    if options
        .native_input
        .as_ref()
        .is_some_and(|input| input.format == "graphql")
        && options.raw
        && explicit_client_style
    {
        bail!("GraphQL --raw-sdk and --client-style are mutually exclusive");
    }
    if ((options.raw
        && options
            .native_input
            .as_ref()
            .is_none_or(|input| input.format != "graphql"))
        || options.typescript_transport.is_some()
        || options.client_name.is_some())
        && !options
            .languages
            .iter()
            .any(|target| target == "typescript")
    {
        bail!("TypeScript options require a TypeScript target")
    }
    Ok(Action::Generate(Box::new(options)))
}

fn default_config_path() -> PathBuf {
    PathBuf::from("poolster.json")
}
