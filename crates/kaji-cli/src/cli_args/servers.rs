use super::*;

pub(super) fn parse_check(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut source = None;
    let mut compiler = None;
    let mut format = CheckFormat::Human;
    let mut severity_overrides = BTreeMap::new();
    let mut fail_on = CheckFailureThreshold::Error;
    let mut baseline = None;
    let mut write_baseline = None;
    let mut ignored_rules = BTreeSet::new();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("check accepts exactly one OpenAPI file")
            }
            continue;
        }
        match flag.as_ref() {
            "--openapi-compiler" => {
                compiler = Some(
                    args.next()
                        .context("--openapi-compiler requires a value")?
                        .into(),
                );
            }
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => CheckFormat::Human,
                    "json" => CheckFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                };
            }
            "--json" => format = CheckFormat::Json,
            "--severity" => {
                let value = args
                    .next()
                    .context("--severity requires <rule=warning|error>")?
                    .to_string_lossy()
                    .into_owned();
                let (rule, level) = value.split_once('=').with_context(|| {
                    "--severity requires <rule=warning|error>, for example --severity missing-operation-id=warning"
                })?;
                if !CHECK_RULES.contains(&rule) {
                    bail!("unknown check rule {rule:?}")
                }
                severity_overrides.insert(rule.into(), CheckSeverity::parse(level)?);
            }
            "--fail-on" => {
                fail_on = CheckFailureThreshold::parse(
                    &args
                        .next()
                        .context("--fail-on requires warning, error, or none")?
                        .to_string_lossy(),
                )?;
            }
            "--baseline" => {
                baseline = Some(args.next().context("--baseline requires a file")?.into());
            }
            "--write-baseline" => {
                write_baseline = Some(
                    args.next()
                        .context("--write-baseline requires a file")?
                        .into(),
                );
            }
            "--ignore" => {
                let rule = args
                    .next()
                    .context("--ignore requires a rule name")?
                    .to_string_lossy()
                    .into_owned();
                if !CHECK_RULES.contains(&rule.as_str()) {
                    bail!("unknown check rule {rule:?}")
                }
                ignored_rules.insert(rule);
            }
            _ => bail!("unknown check option {flag}; run poolster --help"),
        }
    }
    Ok(Action::Check(Check {
        source: source.context("check requires an OpenAPI file")?,
        compiler,
        format,
        severity_overrides,
        fail_on,
        baseline,
        write_baseline,
        ignored_rules,
    }))
}

pub(super) fn parse_mcp(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let collected = args.into_iter().collect::<Vec<_>>();
    if collected.len() == 1 && collected[0] == "generator" {
        return Ok(Action::McpGenerator);
    }
    let mut source = None;
    let mut base_url = None;
    let mut compiler = None;
    let mut args = collected.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("mcp accepts exactly one OpenAPI file")
            }
            continue;
        }
        if !matches!(flag.as_ref(), "--base-url" | "--openapi-compiler") {
            bail!("unknown mcp option {flag}; run poolster --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        if flag == "--base-url" {
            let value = value
                .into_string()
                .map_err(|_| anyhow::anyhow!("--base-url requires UTF-8 text"))?;
            let parsed =
                reqwest::Url::parse(&value).context("--base-url must be an absolute URL")?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("--base-url must use http or https")
            }
            base_url = Some(value.trim_end_matches('/').to_owned());
        } else {
            compiler = Some(value.into());
        }
    }
    let source = source.context("mcp requires an OpenAPI file")?;
    let base_url = base_url.context("mcp requires --base-url")?;
    Ok(Action::Mcp(Mcp {
        source,
        base_url,
        compiler,
    }))
}

pub(super) fn parse_mock(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        bail!("mock requires a subcommand; use poolster mock serve --help")
    };
    if command != "serve" {
        bail!("unknown mock subcommand {command:?}; use poolster mock serve")
    }
    let mut source = None;
    let mut port = 4010;
    let mut compiler = None;
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !flag.starts_with('-') {
            if source.replace(PathBuf::from(argument)).is_some() {
                bail!("mock serve accepts exactly one OpenAPI file")
            }
            continue;
        }
        if !matches!(flag.as_ref(), "--port" | "--openapi-compiler") {
            bail!("unknown mock serve option {flag}; run poolster --help")
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        if flag == "--port" {
            port = value
                .into_string()
                .map_err(|_| anyhow::anyhow!("--port requires UTF-8 text"))?
                .parse()
                .context("--port must be a valid TCP port")?;
            if port == 0 {
                bail!("--port must be nonzero")
            }
        } else {
            compiler = Some(value.into());
        }
    }
    Ok(Action::MockServe(MockServe {
        source: source.context("mock serve requires an OpenAPI file")?,
        port,
        compiler,
    }))
}

pub(super) fn parse_init(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut init = Init {
        config: PathBuf::from("poolster.json"),
        input: PathBuf::from("openapi.yaml"),
        output: PathBuf::from("generated"),
        name: default_api_name(),
        version: default_sdk_version(),
    };
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if flag == "--help" || flag == "-h" {
            return Ok(Action::Help);
        }
        if !matches!(
            flag.as_ref(),
            "--config" | "--input" | "--output" | "--name" | "--sdk-version"
        ) {
            bail!("unknown init option {flag}; run poolster --help");
        }
        let value = args
            .next()
            .with_context(|| format!("{flag} requires a value"))?;
        let value = value
            .into_string()
            .map_err(|_| anyhow::anyhow!("{flag} requires UTF-8 text"))?;
        if value.trim().is_empty() {
            bail!("{flag} cannot be empty");
        }
        match flag.as_ref() {
            "--config" => init.config = value.into(),
            "--input" => init.input = value.into(),
            "--output" => init.output = value.into(),
            "--name" => init.name = value,
            "--sdk-version" => init.version = value,
            _ => unreachable!(),
        }
    }
    Ok(Action::Init(init))
}
