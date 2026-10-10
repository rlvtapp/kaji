//! Commands implementation for generated rust-cli packages.
use super::*;

pub(super) fn render_command_modules(
    api: &Api,
    command: &str,
    base_url: Option<&str>,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> Vec<(String, String)> {
    let operations = api
        .operations
        .iter()
        .map(|op| render_operation(api, op, command))
        .collect::<Vec<_>>();
    let literal = operations.join(",\n    ");
    let main = render_main(api, command, base_url, security_schemes);
    if operations.len() <= 50 && literal.len() <= 128 * 1024 {
        return vec![("src/main.rs".into(), main)];
    }
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0usize;
    for operation in operations {
        if !current.is_empty() && (current.len() >= 50 || bytes + operation.len() > 128 * 1024) {
            chunks.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes += operation.len();
        current.push(operation);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    let mut modules = String::new();
    let mut references = Vec::new();
    let mut files = Vec::new();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let name = format!("operations{index:04}");
        modules.push_str(&format!(
            "#[path = \"commands/chunk_{index:04}.rs\"] mod {name};\n"
        ));
        references.push(format!("{name}::OPERATIONS"));
        files.push((format!("src/commands/chunk_{index:04}.rs"), format!("{NOTICE}use super::Operation;\npub(super) const OPERATIONS: &[Operation] = &[{}];\n", chunk.join(",\n").replace("Parameter {", "super::Parameter {").replace("BodyField {", "super::BodyField {"))));
    }
    let main = main
        .replace(
            &format!("const OPERATIONS: &[Operation] = &[\n    {literal}\n];"),
            &format!(
                "const OPERATIONS: &[&[Operation]] = &[{}];",
                references.join(", ")
            ),
        )
        .replace(
            "for operation in OPERATIONS {",
            "for operation in OPERATIONS.iter().flat_map(|chunk| chunk.iter()) {",
        )
        .replace(
            "OPERATIONS.iter().find(",
            "OPERATIONS.iter().flat_map(|chunk| chunk.iter()).find(",
        );
    files.insert(0, ("src/main.rs".into(), format!("{modules}{main}")));
    files
}

pub(super) fn render_main(
    api: &Api,
    command: &str,
    base_url: Option<&str>,
    security_schemes: Option<&SecuritySchemeCatalog>,
) -> String {
    let operations = api
        .operations
        .iter()
        .map(|op| render_operation(api, op, command))
        .collect::<Vec<_>>()
        .join(",\n    ");
    let command_lit = literal(command);
    let api_name = literal(&format!("Generated CLI for {}", api.name));
    let security_schemes = render_security_schemes(security_schemes);
    let base_url = base_url
        .map(|url| format!("Some({})", literal(url)))
        .unwrap_or_else(|| "None".into());
    format!(
        r#"{NOTICE}mod poolster_extension;

use anyhow::{{bail, Context, Result}};
use clap::{{Arg, ArgAction, ArgMatches, Command}};
use reqwest::blocking::Client;
use reqwest::header::{{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE}};
use serde_json::{{Map, Value}};
use std::{{env, fs, io::IsTerminal}};
use dialoguer::{{Input, Password}};
use poolster_extension::{{AuthenticationResult, Extension, PoolsterExtension}};

#[derive(Clone, Copy)] struct Parameter {{ name: &'static str, option: &'static str, location: &'static str, required: bool }}
#[derive(Clone, Copy)] struct BodyField {{ name: &'static str, option: &'static str, kind: &'static str, array: bool, file: bool, required: bool }}
#[derive(Clone, Copy)] struct Operation {{ id: &'static str, command: &'static [&'static str], method: &'static str, path: &'static str, parameters: &'static [Parameter], body: &'static [BodyField], body_required: bool, security: &'static [&'static [&'static str]] }}
#[derive(Clone, Copy)] struct SecurityScheme {{ id: &'static str, kind: &'static str, name: &'static str, location: &'static str }}
const SECURITY_SCHEMES: &[SecurityScheme] = &[
    {security_schemes}
];
const OPERATIONS: &[Operation] = &[
    {operations}
];

pub(super) fn main() {{ if let Err(error) = run() {{ if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() && !std::env::args().any(|argument| argument == "--json") {{ eprintln!("\x1b[31m✖\x1b[0m {{error:#}}"); }} else {{ eprintln!("{{}}", serde_json::json!({{ "error": {{ "message": format!("{{error:#}}"), "code": "cli_error" }} }})); }} std::process::exit(1); }} }}
pub(super) fn run() -> Result<()> {{
    let matches = build_cli().get_matches();
    let interactive = is_interactive(&matches);
    let extension = PoolsterExtension::default();
    if let Some(auth) = matches.subcommand_matches("auth") {{ return run_auth(auth, &extension, interactive); }}
    let (operation, values) = selected_operation(&matches).context("Choose an API command; use --help to list commands.")?;
    if interactive {{ println!("\n\x1b[36m◆\x1b[0m {{}}  \x1b[2m{{}} {{}}\x1b[0m", operation.command.join(" "), operation.method, operation.path); }}
    let base_url = match values.get_one::<String>("base-url").cloned().or_else(|| env::var(format!("{{}}_BASE_URL", env_key({command_lit}))).ok()).or_else(|| {base_url}.map(str::to_owned)) {{ Some(value) => value, None if interactive => prompt("API base URL")?, None => bail!("Set --base-url or the generated *_BASE_URL environment variable.") }};
    let active_profile = default_profile()?;
    let profile = values.get_one::<String>("profile").map(String::as_str).unwrap_or(&active_profile);
    let mut headers = HeaderMap::new(); let mut query = Vec::new(); let mut path = operation.path.to_owned();
    for parameter in operation.parameters {{ let supplied = values.get_one::<String>(parameter.option).cloned(); let value = match supplied {{ Some(value) => Some(value), None if parameter.required && interactive => Some(prompt(&format!("--{{}}", parameter.option))?), None if parameter.required => bail!("Missing required --{{}}", parameter.option), None => None }}; if let Some(value) = value {{ match parameter.location {{ "path" => path = path.replace(&format!("{{{{{{}}}}}}", parameter.name), &url_encode(&value)), "query" => query.push((parameter.name, value)), "header" => {{ headers.insert(HeaderName::from_bytes(parameter.name.as_bytes())?, HeaderValue::from_str(&value)?); }}, _ => {{}} }} }} }}
    let body = request_body(values, operation, interactive)?; if body.is_some() {{ headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json")); }} headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    let context = poolster_extension::AuthContext {{ profile, command: operation.command, operation_id: operation.id }};
    if matches!(extension.authenticate(&mut headers, &mut query, &context)?, AuthenticationResult::UseOpenApi) {{ apply_openapi_security(operation, profile, &mut headers, &mut query, {command_lit})?; }}
    extension.before_request(&mut headers, &mut query, &context)?;
    let mut url = format!("{{}}/{{}}", base_url.trim_end_matches('/'), path.trim_start_matches('/')); if !query.is_empty() {{ url.push('?'); url.push_str(&query.into_iter().map(|(key, value)| format!("{{}}={{}}", url_encode(key), url_encode(&value))).collect::<Vec<_>>().join("&")); }}
    let response = Client::new().request(operation.method.parse()?, &url).headers(headers).json(&body).send()?;
    let status = response.status(); let text = response.text()?; extension.after_response(status.as_u16(), &text, &context)?; if !status.is_success() {{ bail!("{{status}}: {{text}}"); }}
    if interactive {{ println!("\x1b[32m✔ Request completed\x1b[0m\n"); }} if values.get_flag("json") || !interactive {{ println!("{{text}}"); }} else if let Ok(value) = serde_json::from_str::<Value>(&text) {{ println!("{{}}", serde_json::to_string_pretty(&value)?); }} else {{ println!("{{text}}"); }} Ok(())
}}
pub(super) fn is_interactive(matches: &ArgMatches) -> bool {{ std::io::stdin().is_terminal() && std::io::stdout().is_terminal() && !matches.get_flag("json") }}
pub(super) fn prompt(label: &str) -> Result<String> {{ let value = Input::<String>::new().with_prompt(format!("\x1b[36m◆\x1b[0m {{label}}")).interact_text()?; if value.trim().is_empty() {{ bail!("{{label}} is required.") }} Ok(value.trim().to_owned()) }}
pub(super) fn prompt_secret(label: &str) -> Result<String> {{ let value = Password::new().with_prompt(format!("\x1b[36m◆\x1b[0m {{label}}")).interact()?; if value.trim().is_empty() {{ bail!("{{label}} is required.") }} Ok(value.trim().to_owned()) }}
pub(super) fn build_cli() -> Command {{ let mut root = Command::new({command_lit}).about({api_name}).arg(Arg::new("base-url").long("base-url").global(true).value_name("URL")).arg(Arg::new("profile").long("profile").global(true).value_name("PROFILE")).arg(Arg::new("json").long("json").global(true).action(ArgAction::SetTrue)); root = root.subcommand(auth_command()); for operation in OPERATIONS {{ root = add_operation(root, operation); }} root }}
pub(super) fn auth_command() -> Command {{ let credential = |name| Command::new(name).arg(Arg::new("credential").required(false)).arg(Arg::new("scheme").long("scheme").value_name("OPENAPI_SCHEME")); Command::new("auth").about("Manage credentials").subcommand(credential("set-token")).subcommand(credential("set-key")).subcommand(Command::new("login")).subcommand(Command::new("status")).subcommand(Command::new("profiles")).subcommand(Command::new("use").arg(Arg::new("profile").required(true))).subcommand(Command::new("logout").arg(Arg::new("scheme").long("scheme").value_name("OPENAPI_SCHEME"))) }}
pub(super) fn add_operation(mut root: Command, operation: &'static Operation) -> Command {{ root = add_nested(root, operation.command, operation); root }}
pub(super) fn add_nested(mut command: Command, parts: &'static [&'static str], operation: &'static Operation) -> Command {{ if parts.len() == 1 {{ return command.subcommand(operation_command(parts[0], operation)); }} let existing = command.find_subcommand(parts[0]).cloned().unwrap_or_else(|| Command::new(parts[0])); command = command.subcommand(existing); let child = command.find_subcommand_mut(parts[0]).expect("added command"); *child = add_nested(std::mem::take(child), &parts[1..], operation); command }}
pub(super) fn operation_command(name: &'static str, operation: &'static Operation) -> Command {{ let mut command = Command::new(name).about(format!("{{}} {{}}", operation.method, operation.path)).arg(Arg::new("data").long("data").value_name("JSON")).arg(Arg::new("data-file").long("data-file").value_name("FILE")); for parameter in operation.parameters {{ command = command.arg(Arg::new(parameter.option).long(parameter.option).required(false).value_name("VALUE")); }} for field in operation.body {{ let arg = Arg::new(field.option).long(field.option).required(false).value_name("VALUE").action(if field.array {{ ArgAction::Append }} else {{ ArgAction::Set }}); command = command.arg(arg); if field.file {{ let file_option: &'static str = Box::leak(format!("{{}}-file", field.option).into_boxed_str()); command = command.arg(Arg::new(file_option).long(file_option).value_name("FILE")); }} }} command }}
pub(super) fn selected_operation<'a>(matches: &'a ArgMatches) -> Option<(&'static Operation, &'a ArgMatches)> {{ let mut current = matches; let mut parts = Vec::new(); while let Some((name, child)) = current.subcommand() {{ if name == "auth" {{ return None; }} parts.push(name); current = child; }} OPERATIONS.iter().find(|operation| operation.command == parts.as_slice()).map(|operation| (operation, current)) }}
pub(super) fn request_body(values: &ArgMatches, operation: &Operation, interactive: bool) -> Result<Option<Value>> {{ if let (Some(_), Some(_)) = (values.get_one::<String>("data"), values.get_one::<String>("data-file")) {{ bail!("Use --data or --data-file, not both."); }} if let Some(data) = values.get_one::<String>("data") {{ return Ok(Some(serde_json::from_str(data)?)); }} if let Some(file) = values.get_one::<String>("data-file") {{ return Ok(Some(serde_json::from_str(&fs::read_to_string(file)?)?)); }} if operation.body.is_empty() && operation.body_required {{ return if interactive {{ Ok(Some(serde_json::from_str(&prompt("--data (JSON request body)")?)?)) }} else {{ bail!("This operation requires --data or --data-file.") }}; }} let mut object = Map::new(); for field in operation.body {{ let file = values.get_one::<String>(&format!("{{}}-file", field.option)); if file.is_some() && values.contains_id(field.option) {{ bail!("Use --{{}} or --{{}}-file, not both.", field.option, field.option); }} if field.array {{ let entries = values.get_many::<String>(field.option).map(|values| values.map(|value| body_value(value, field.kind)).collect::<Result<Vec<_>>>()).transpose()?; if let Some(entries) = entries {{ object.insert(field.name.into(), Value::Array(entries)); }} else if field.required && interactive {{ let entries = prompt(&format!("--{{}} (comma separated)", field.option))?.split(',').map(|value| body_value(value.trim(), field.kind)).collect::<Result<Vec<_>>>()?; object.insert(field.name.into(), Value::Array(entries)); }} else if field.required {{ bail!("Missing required --{{}}", field.option); }} }} else {{ let value = if let Some(file) = file {{ fs::read_to_string(file)? }} else if let Some(value) = values.get_one::<String>(field.option) {{ value.clone() }} else if field.required && interactive {{ prompt(&format!("--{{}}", field.option))? }} else {{ String::new() }}; if value.is_empty() {{ if field.required {{ bail!("Missing required --{{}}", field.option); }} }} else {{ object.insert(field.name.into(), body_value(&value, field.kind)?); }} }} }} if object.is_empty() {{ if operation.body_required {{ bail!("This operation requires body flags, --data, or --data-file."); }} Ok(None) }} else {{ Ok(Some(Value::Object(object))) }} }}
pub(super) fn body_value(value: &str, kind: &str) -> Result<Value> {{ Ok(match kind {{ "integer" => Value::Number(value.parse::<i64>()?.into()), "number" => serde_json::Number::from_f64(value.parse()?).map(Value::Number).context("Expected a finite number")?, "boolean" => Value::Bool(value.parse()?), _ => Value::String(value.into()) }}) }}
pub(super) fn run_auth(matches: &ArgMatches, extension: &impl Extension, interactive: bool) -> Result<()> {{ let profile = matches.get_one::<String>("profile").cloned().unwrap_or(default_profile()?); match matches.subcommand() {{ Some((command @ ("set-token" | "set-key"), values)) => {{ let requested = values.get_one::<String>("scheme").map(String::as_str); let scheme = if command == "set-key" {{ infer_api_key_scheme(requested)? }} else {{ requested }}; let credential = match values.get_one::<String>("credential") {{ Some(value) => value.clone(), None if interactive => prompt_secret("Credential")?, None => bail!("credential is required when stdin is not a terminal") }}; save_credential(&profile, scheme, &credential)?; println!("\x1b[32m✔\x1b[0m Stored credential for profile {{profile}}."); }}, Some(("login", _)) => {{ let context = poolster_extension::AuthContext {{ profile: &profile, command: &["auth", "login"], operation_id: "login" }}; let token = extension.login(&context)?.context("No token returned. Implement login in src/poolster_extension.rs for this provider.")?; save_credential(&profile, None, &token)?; println!("\x1b[32m✔\x1b[0m Signed in to profile {{profile}}."); }}, Some(("status", _)) => println!("{{}}", if profile_has_credentials(&profile)? {{ "Credentials available" }} else {{ "Not signed in" }}), Some(("profiles", _)) => {{ for item in list_profiles()? {{ println!("{{item}}"); }} }}, Some(("use", values)) => {{ let selected = values.get_one::<String>("profile").context("profile is required")?; save_default_profile(selected)?; println!("Using profile {{selected}}."); }}, Some(("logout", values)) => {{ let path = credential_path(&profile, values.get_one::<String>("scheme").map(String::as_str))?; let _ = fs::remove_file(path); println!("Removed credential from profile {{profile}}."); }}, _ => bail!("Choose an auth command") }} Ok(()) }}
pub(super) fn credential_dir() -> Result<std::path::PathBuf> {{ let home = env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))).context("Cannot determine configuration directory")?; Ok(home.join({command_lit})) }}
pub(super) fn credential_path(profile: &str, scheme: Option<&str>) -> Result<std::path::PathBuf> {{ let suffix = scheme.map(|value| format!(".{{}}", env_key(value))).unwrap_or_default(); Ok(credential_dir()?.join(format!("{{profile}}{{suffix}}.token"))) }}
pub(super) fn load_credential(profile: &str, scheme: &str) -> Result<Option<String>> {{ for path in [credential_path(profile, Some(scheme))?, credential_path(profile, None)?] {{ match fs::read_to_string(path) {{ Ok(value) => return Ok(Some(value.trim().to_owned())), Err(error) if error.kind() == std::io::ErrorKind::NotFound => {{}}, Err(error) => return Err(error.into()), }} }} Ok(None) }}
pub(super) fn save_credential(profile: &str, scheme: Option<&str>, credential: &str) -> Result<()> {{ if credential.trim().is_empty() {{ bail!("Credential cannot be empty."); }} let path = credential_path(profile, scheme)?; fs::create_dir_all(path.parent().expect("credential parent"))?; fs::write(path, credential.trim())?; Ok(()) }}
pub(super) fn default_profile_path() -> Result<std::path::PathBuf> {{ Ok(credential_dir()?.join("active-profile")) }}
pub(super) fn default_profile() -> Result<String> {{ match fs::read_to_string(default_profile_path()?) {{ Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()), Ok(_) | Err(_) => Ok("default".into()), }} }}
pub(super) fn save_default_profile(profile: &str) -> Result<()> {{ let path = default_profile_path()?; fs::create_dir_all(path.parent().expect("credential parent"))?; fs::write(path, profile)?; Ok(()) }}
pub(super) fn list_profiles() -> Result<Vec<String>> {{ let dir = credential_dir()?; let mut profiles = std::collections::BTreeSet::new(); profiles.insert(default_profile()?); if let Ok(entries) = fs::read_dir(dir) {{ for entry in entries.flatten() {{ if let Some(name) = entry.file_name().to_str().and_then(|name| name.strip_suffix(".token")) {{ profiles.insert(name.split('.').next().unwrap_or(name).to_owned()); }} }} }} Ok(profiles.into_iter().collect()) }}
pub(super) fn profile_has_credentials(profile: &str) -> Result<bool> {{ let prefix = format!("{{profile}}."); let default_token = format!("{{profile}}.token"); Ok(fs::read_dir(credential_dir()?).ok().into_iter().flatten().flatten().filter_map(|entry| entry.file_name().into_string().ok()).any(|name| name == default_token || (name.starts_with(&prefix) && name.ends_with(".token")))) }}
pub(super) fn infer_api_key_scheme(requested: Option<&str>) -> Result<Option<&'static str>> {{ if let Some(scheme) = requested {{ let value = SECURITY_SCHEMES.iter().find(|candidate| candidate.id == scheme && candidate.kind == "api-key").context("--scheme must name an OpenAPI API-key scheme")?; return Ok(Some(value.id)); }} let matches = SECURITY_SCHEMES.iter().filter(|candidate| candidate.kind == "api-key").collect::<Vec<_>>(); match matches.as_slice() {{ [scheme] => Ok(Some(scheme.id)), [] => bail!("This OpenAPI document declares no API-key scheme."), _ => bail!("This API has multiple API-key schemes; pass --scheme."), }} }}
pub(super) fn apply_openapi_security(operation: &Operation, profile: &str, headers: &mut HeaderMap, query: &mut Vec<(&'static str, String)>, command: &str) -> Result<()> {{ if operation.security.is_empty() {{ return Ok(()); }} for alternative in operation.security {{ let mut credentials = Vec::new(); for scheme_id in *alternative {{ let Some(scheme) = SECURITY_SCHEMES.iter().find(|candidate| candidate.id == *scheme_id) else {{ continue; }}; let environment = format!("{{}}_{{}}_TOKEN", env_key(command), env_key(scheme.id)); let credential = env::var(&environment).ok().or_else(|| env::var(format!("{{}}_TOKEN", env_key(command))).ok()).or_else(|| load_credential(profile, scheme.id).ok().flatten()); let Some(credential) = credential else {{ credentials.clear(); break; }}; credentials.push((*scheme, credential)); }} if credentials.len() != alternative.len() {{ continue; }} for (scheme, credential) in credentials {{ if scheme.kind == "api-key" {{ match scheme.location {{ "query" => query.push((scheme.name, credential)), "cookie" => {{ headers.append(reqwest::header::COOKIE, HeaderValue::from_str(&format!("{{}}={{}}", scheme.name, url_encode(&credential)))?); }}, _ => {{ headers.insert(HeaderName::from_bytes(scheme.name.as_bytes())?, HeaderValue::from_str(&credential)?); }} }} }} else {{ headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {{credential}}"))?); }} }} return Ok(()); }} bail!("No credentials for this operation. Run `auth set-key <value> --scheme <name>` or set the generated <COMMAND>_<SCHEME>_TOKEN environment variable.") }}
pub(super) fn env_key(name: &str) -> String {{ name.chars().map(|character| if character.is_ascii_alphanumeric() {{ character.to_ascii_uppercase() }} else {{ '_' }}).collect() }}
pub(super) fn url_encode(value: &str) -> String {{ value.bytes().map(|byte| if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {{ (byte as char).to_string() }} else {{ format!("%{{byte:02X}}") }}).collect() }}
"#
    )
}
