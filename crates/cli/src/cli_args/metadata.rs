use super::*;

pub(super) fn parse_discover(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut query = None;
    let mut limit = 20;
    let mut format = DiscoverFormat::Human;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--limit" => {
                let value = args.next().context("--limit requires a positive integer")?;
                limit = value
                    .to_string_lossy()
                    .parse()
                    .context("--limit requires a positive integer")?;
                if limit == 0 {
                    bail!("--limit requires a positive integer")
                }
            }
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => DiscoverFormat::Human,
                    "json" => DiscoverFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                }
            }
            value if value.starts_with('-') => bail!("unknown option {value}; run poolster --help"),
            _ => {
                if query
                    .replace(
                        argument
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("discover query must be UTF-8 text"))?,
                    )
                    .is_some()
                {
                    bail!("discover accepts exactly one query")
                }
            }
        }
    }
    let query = query.context("discover requires a query")?;
    if query.trim().is_empty() {
        bail!("discover query cannot be empty")
    }
    Ok(Action::Discover(Discover {
        query,
        limit,
        format,
    }))
}

pub(super) fn parse_download(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut id = None;
    let mut version = None;
    let mut output = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--version" => {
                version = Some(
                    args.next()
                        .context("--version requires a value")?
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("--version requires UTF-8 text"))?,
                );
            }
            "--output" | "-o" => {
                output = Some(args.next().context("--output requires a file path")?.into());
            }
            value if value.starts_with('-') => bail!("unknown option {value}; run poolster --help"),
            _ => {
                if id
                    .replace(
                        argument
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("API id must be UTF-8 text"))?,
                    )
                    .is_some()
                {
                    bail!("download accepts exactly one API id")
                }
            }
        }
    }
    let id = id.context("download requires an API id")?;
    if id.trim().is_empty() {
        bail!("API id cannot be empty")
    }
    let output = output.context("download requires --output <openapi-file>")?;
    Ok(Action::Download(Download {
        id,
        version,
        output,
    }))
}

pub(super) fn parse_show(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut source = None;
    let mut compiler = None;
    let mut paths = PathSelection::default();
    let mut format = ShowFormat::Human;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument.to_string_lossy();
        if !flag.starts_with('-') {
            if source.replace(argument.into()).is_some() {
                bail!("show accepts exactly one OpenAPI file")
            }
            continue;
        }
        match flag.as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--openapi-compiler" => {
                compiler = Some(
                    args.next()
                        .context("--openapi-compiler requires a value")?
                        .into(),
                );
            }
            "--include-path" => paths.include.push(
                args.next()
                    .context("--include-path requires a pattern")?
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--include-path requires UTF-8 text"))?,
            ),
            "--exclude-path" => paths.exclude.push(
                args.next()
                    .context("--exclude-path requires a pattern")?
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("--exclude-path requires UTF-8 text"))?,
            ),
            "--format" => {
                format = match args
                    .next()
                    .context("--format requires human or json")?
                    .to_string_lossy()
                    .as_ref()
                {
                    "human" => ShowFormat::Human,
                    "json" => ShowFormat::Json,
                    value => bail!("--format must be human or json, got {value:?}"),
                }
            }
            "--json" => format = ShowFormat::Json,
            value => bail!("unknown show option {value}; run poolster --help"),
        }
    }
    validate_path_selection(&paths)?;
    Ok(Action::Show(Show {
        source: source.context("show requires an OpenAPI file")?,
        compiler,
        paths,
        format,
    }))
}

pub(super) fn parse_update(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut output = PathBuf::from(".");
    let mut force = false;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--help" | "-h" => return Ok(Action::Help),
            "--output" | "-o" => {
                output = args.next().context("--output requires a directory")?.into();
            }
            "--force" => force = true,
            value if value.starts_with('-') => {
                bail!("unknown update option {value}; run poolster --help")
            }
            _ => bail!("update accepts only --output and --force"),
        }
    }
    Ok(Action::Update(Update { output, force }))
}

pub(super) fn parse_auth(args: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut args = args.into_iter();
    let command = args
        .next()
        .context("auth requires login, logout, or status")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("auth command must be UTF-8 text"))?;
    match command.as_str() {
        "status" => {
            if args.next().is_some() {
                bail!("auth status does not accept arguments")
            }
            Ok(Action::Auth(Auth::Status))
        }
        "logout" => {
            let profile = args
                .next()
                .context("auth logout requires a profile name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("auth profile must be UTF-8 text"))?;
            if args.next().is_some() {
                bail!("auth logout accepts exactly one profile name")
            }
            Ok(Action::Auth(Auth::Logout { profile }))
        }
        "login" => {
            let profile = args
                .next()
                .context("auth login requires a profile name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("auth profile must be UTF-8 text"))?;
            let flag = args
                .next()
                .context("auth login requires --token-env <name>")?;
            if flag != "--token-env" {
                bail!("auth login requires --token-env <name>")
            }
            let token_env = args
                .next()
                .context("--token-env requires an environment variable name")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("--token-env requires UTF-8 text"))?;
            if args.next().is_some() {
                bail!("auth login accepts only --token-env <name>")
            }
            Ok(Action::Auth(Auth::Login { profile, token_env }))
        }
        "--help" | "-h" | "help" => Ok(Action::Help),
        _ => bail!("auth requires login, logout, or status"),
    }
}
