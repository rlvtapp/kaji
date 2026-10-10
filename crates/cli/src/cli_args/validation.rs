//! Direct generation mode and option compatibility validation.
use super::*;

pub(super) fn finish(mut options: Generate, explicit_client_style: bool) -> Result<Action> {
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
