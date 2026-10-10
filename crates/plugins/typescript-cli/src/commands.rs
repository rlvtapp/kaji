//! Commands implementation for generated typescript-cli packages.
use super::*;

/// Keep small CLIs source-compatible, but isolate large command metadata from
/// startup/runtime code. Bound chunks by both serialized bytes and operation count.
pub(super) fn render_command_modules(
    api: &Api,
    command_name: &str,
    config: &Value,
) -> Vec<(String, String)> {
    let operations = api
        .operations
        .iter()
        .map(|op| operation_json(api, op, command_name))
        .collect::<Vec<_>>();
    let literal = serde_json::to_string_pretty(&operations).unwrap();
    let index = render_index(api, command_name, config);
    if operations.len() <= 50 && literal.len() <= 128 * 1024 {
        return vec![("src/index.ts".into(), index)];
    }
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0usize;
    for operation in operations {
        let size = serde_json::to_string_pretty(&operation).unwrap().len();
        if !current.is_empty() && (current.len() >= 50 || bytes + size > 128 * 1024) {
            chunks.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes += size;
        current.push(operation);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    let mut imports = String::new();
    let mut references = Vec::new();
    let mut files = Vec::new();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let name = format!("operations{index:04}");
        imports.push_str(&format!(
            "import {{ {name} }} from './commands/chunk_{index:04}.js';\n"
        ));
        references.push(format!("...{name}"));
        files.push((format!("src/commands/chunk_{index:04}.ts"), format!("{NOTICE}import type {{ Operation }} from '../runtime.js';\nexport const {name}: readonly Operation[] = {};\n",serde_json::to_string_pretty(&chunk).unwrap())));
    }
    let index = index.replace(
        &format!("const operations: readonly Operation[] = {literal};"),
        &format!(
            "const operations: readonly Operation[] = [{}];",
            references.join(", ")
        ),
    );
    files.insert(0, ("src/index.ts".into(), format!("{imports}{index}")));
    files
}

pub(super) fn render_index(api: &Api, command_name: &str, config: &Value) -> String {
    let operations = api
        .operations
        .iter()
        .map(|operation| operation_json(api, operation, command_name))
        .collect::<Vec<_>>();
    let template = r#"import { Command } from 'commander';
import { extension } from './poolster.extension.js';
import { authLogin, authLogout, authProfiles, authSetKey, authSetToken, authStatus, authUseProfile, formatCliError, runOperation, type CliConfig, type Operation } from './runtime.js';

const config = __CONFIG__ as const satisfies CliConfig;
const operations: readonly Operation[] = __OPERATIONS__;

const program = new Command().name(__COMMAND_NAME__).description(__API_NAME__).version(__API_VERSION__);

const auth = program.command('auth').description('Manage API credentials');
auth.command('set-token [token]').description('Store an API token in the local credential store').option('--profile <name>', 'credential profile').option('--scheme <name>', 'OpenAPI security scheme').action(async (token, options) => authSetToken(config, options.profile, options.scheme, token));
auth.command('set-key [key]').description('Store an API key; infers the OpenAPI API-key scheme').option('--profile <name>', 'credential profile').option('--scheme <name>', 'only needed when the API has multiple key schemes').action(async (key, options) => authSetKey(config, options.profile, options.scheme, key));
auth.command('login').description('Sign in with OAuth').option('--profile <name>', 'credential profile').option('--flow <device|browser>', 'OAuth flow').action(async (options) => authLogin(config, options.profile, options.flow, extension));
auth.command('status').description('Show local credential status').option('--profile <name>', 'credential profile').action(async (options) => authStatus(config, options.profile));
auth.command('profiles').description('List credential profiles').action(async () => authProfiles(config));
auth.command('use <name>').description('Use a profile by default').action(async (name) => authUseProfile(config, name));
auth.command('logout').description('Remove a local credential profile').option('--profile <name>', 'credential profile').action(async (options) => authLogout(config, options.profile));

for (const operation of operations) {
  let command = program;
  for (const name of operation.command) {
    command = command.commands.find((candidate) => candidate.name() === name) ?? command.command(name);
  }
  command.description(`${operation.method} ${operation.path}`);
  command.option('--profile <name>', 'credential profile');
  command.option('--base-url <url>', 'API base URL');
  command.option('--json', 'print compact JSON');
  command.option('--data <json>', 'JSON request body (for complex bodies)');
  command.option('--data-file <file>', 'JSON request body file (for complex bodies)');
  for (const parameter of operation.parameters) {
    command.option(`--${parameter.option} <value>`, parameter.description ?? parameter.name);
  }
  for (const field of operation.bodyFields) {
    if (field.array) command.option(`--${field.option} <value>`, field.description ?? field.name, (value: string, previous: string[] = []) => [...previous, value]);
    else command.option(`--${field.option} <value>`, field.description ?? field.name);
    if (field.file) command.option(`--${field.option}-file <file>`, `read ${field.name} from a file`);
  }
  command.action(async (options) => runOperation(config, operation, options, extension));
}

program.parseAsync().catch((error: unknown) => {
  console.error(formatCliError(error));
  process.exitCode = 1;
});
"#;
    format!(
        "{NOTICE}{}",
        template
            .replace(
                "__CONFIG__",
                &serde_json::to_string_pretty(config).expect("serializable CLI config")
            )
            .replace(
                "__OPERATIONS__",
                &serde_json::to_string_pretty(&operations).expect("serializable CLI operations")
            )
            .replace(
                "__COMMAND_NAME__",
                &serde_json::to_string(command_name).expect("serializable command name")
            )
            .replace(
                "__API_NAME__",
                &serde_json::to_string(&api.name).expect("serializable API name")
            )
            .replace(
                "__API_VERSION__",
                &serde_json::to_string(&api.version).expect("serializable API version")
            )
    )
}

#[allow(dead_code)]
pub(super) fn render_index_legacy(api: &Api, command_name: &str, config: &Value) -> String {
    let operations = api
        .operations
        .iter()
        .map(|operation| operation_json(api, operation, command_name))
        .collect::<Vec<_>>();
    let config = serde_json::to_string_pretty(config).expect("serializable CLI config");
    let operations =
        serde_json::to_string_pretty(&operations).expect("serializable CLI operations");
    format!(
        "{NOTICE}import {{ Command }} from 'commander';\nimport {{ authLogin, authLogout, authSetToken, authStatus, runOperation, type CliConfig, type Operation }} from './runtime.js';\n\nconst config = {config} as const satisfies CliConfig;\nconst operations = {operations} as const satisfies readonly Operation[];\n\nconst program = new Command().name({command_name:?}).description({:?}).version({:?});\n\nconst auth = program.command('auth').description('Manage API credentials');\nauth.command('set-token <token>').description('Store an API token in the local credential store').option('--profile <name>', 'credential profile', 'default').option('--scheme <name>', 'OpenAPI security scheme').action(async (token, options) => authSetToken(config, options.profile, options.scheme, token));\nauth.command('login').description('Sign in with OAuth').option('--profile <name>', 'credential profile', 'default').option('--flow <device|browser>', 'OAuth flow').action(async (options) => authLogin(config, options.profile, options.flow));\nauth.command('status').description('Show local credential status').option('--profile <name>', 'credential profile', 'default').action(async (options) => authStatus(config, options.profile));\nauth.command('logout').description('Remove a local credential profile').option('--profile <name>', 'credential profile', 'default').action(async (options) => authLogout(config, options.profile));\n\nfor (const operation of operations) {{\n  let command = program;\n  for (const name of operation.command) {{\n    command = command.commands.find((candidate) => candidate.name() === name) ?? command.command(name);\n  }}\n  command.description(`${{operation.method}} ${{operation.path}}`);\n  command.option('--profile <name>', 'credential profile', 'default');\n  command.option('--base-url <url>', 'API base URL');\n  command.option('--json', 'print compact JSON');\n  command.option('--data <json>', 'JSON request body (for complex bodies)');\n  command.option('--data-file <file>', 'JSON request body file (for complex bodies)');\n  for (const parameter of operation.parameters) {{\n    command.option(`--${{parameter.option}} <value>`, parameter.description ?? parameter.name);\n  }}\n  for (const field of operation.bodyFields) {{\n    if (field.array) command.option(`--${{field.option}} <value>`, field.description ?? field.name, (value: string, previous: string[] = []) => [...previous, value]);\n    else command.option(`--${{field.option}} <value>`, field.description ?? field.name);\n    if (field.file) command.option(`--${{field.option}}-file <file>`, `read ${{field.name}} from a file`);\n  }}\n  command.action(async (options) => runOperation(config, operation, options));\n}}\n\nprogram.parseAsync().catch((error: unknown) => {{\n  console.error(error instanceof Error ? error.message : String(error));\n  process.exitCode = 1;\n}});\n",
        api.name, api.version
    )
}
