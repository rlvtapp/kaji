//! Docs implementation for generated rust-cli packages.
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
    let mut groups = std::collections::BTreeMap::<String, Vec<&Operation>>::new();
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
                for (name, kind, array, file, required) in body_flags(api, operation) {
                    output.push_str(&format!(
                        "| --{} | {}{} | {} | Request body field. |\n",
                        kebab_case(&name),
                        kind,
                        if array { "[]" } else { "" },
                        if required { "Yes" } else { "No" }
                    ));
                    if file {
                        output.push_str(&format!(
                            "| --{}-file | path | No | Read {} from a local file. |\n",
                            kebab_case(&name),
                            name
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
