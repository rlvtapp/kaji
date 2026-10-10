//! Docs implementation for generated typescript-cli packages.
use super::*;

pub(super) fn skill_command_parts(operation: &Operation, command_name: &str) -> Vec<String> {
    let mut parts = command_parts(operation);
    if parts.len() > 1
        && parts
            .first()
            .is_some_and(|part| part == &kebab_case(command_name))
    {
        parts.remove(0);
    }
    parts
}

pub(super) fn render_skill_references(api: &Api, command_name: &str) -> Vec<(String, String)> {
    let mut groups = BTreeMap::<String, Vec<&Operation>>::new();
    for operation in &api.operations {
        let parts = skill_command_parts(operation, command_name);
        let group = parts
            .first()
            .cloned()
            .unwrap_or_else(|| "operations".into());
        groups.entry(group).or_default().push(operation);
    }
    groups
        .into_iter()
        .map(|(group, operations)| {
            let mut output = format!(
                "# {group}\n\nDetailed command specifications for {command_name} {group}.\n\n"
            );
            for operation in operations {
                let command = skill_command_parts(operation, command_name).join(" ");
                output.push_str(&format!(
                    "## {command_name} {command}\n\n{} {}\n\n",
                    operation.method.as_str(), operation.path
                ));
                output.push_str("| Flag | Type | Required | Description |\n| --- | --- | --- | --- |\n");
                for parameter in &operation.parameters {
                    output.push_str(&format!(
                        "| --{} | string | {} | {} |\n",
                        kebab_case(&parameter.name),
                        if parameter.required { "Yes" } else { "No" },
                        parameter.description.as_deref().unwrap_or("—")
                    ));
                }
                for field in body_flags(api, operation) {
                    let option = field["option"].as_str().unwrap_or("value");
                    let kind = field["kind"].as_str().unwrap_or("string");
                    let array = field["array"].as_bool().unwrap_or(false);
                    let required = field["required"].as_bool().unwrap_or(false);
                    let description = field["description"].as_str().unwrap_or("—");
                    output.push_str(&format!(
                        "| --{option} | {kind}{} | {} | {} |\n",
                        if array { "[]" } else { "" },
                        if required { "Yes" } else { "No" },
                        description
                    ));
                    if field["file"].as_bool().unwrap_or(false) {
                        output.push_str(&format!(
                            "| --{option}-file | path | No | Read {option} from a local file. |\n"
                        ));
                    }
                }
                if operation.request_body.is_some() {
                    output.push_str(
                        "| --data | JSON | See body | Full JSON request body. |\n| --data-file | path | See body | Read the full JSON request body from a file. |\n",
                    );
                }
                if operation.parameters.is_empty() && operation.request_body.is_none() {
                    output.push_str("| — | — | — | This operation accepts no request flags. |\n");
                }
                output.push('\n');
            }
            (group, output)
        })
        .collect()
}

pub(super) fn render_readme(
    api: &Api,
    package_name: &str,
    command_name: &str,
    has_oauth: bool,
) -> String {
    let command_environment = env_name(command_name);
    let token_environment = format!("{command_environment}_TOKEN");
    let auth = if has_oauth {
        format!("```sh\nnpx {package_name} auth login\n```")
    } else {
        format!(
            "```sh\nnpx {package_name} auth set-token \"$API_TOKEN\" --scheme <security-scheme>\n```"
        )
    };
    format!(
        "# {command_name}\n\nGenerated command-line client for {}.\n\n## Install\n\n```sh\nnpm install\nnpm run build\nnpm link\n```\n\n## Authenticate\n\n{auth}\n\nCredentials are stored per profile in a user-private local credential file. In CI, set `{}` for the API origin and `{}` for an ephemeral token; the token environment variable takes precedence and is never written to disk.\n\n## Call an operation\n\n```sh\n{command_name} <operation-id-in-kebab-case> --help\n```\n\nRequest parameters are flags. Use `--data '{{}}'` or `--data-file request.json` for JSON request bodies. Every command also accepts `--profile`, `--base-url`, and `--json`.\n",
        api.name, command_environment, token_environment
    )
}
