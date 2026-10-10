//! Errors emission for the elixir HTTP SDK.
use crate::*;

pub(crate) fn render_api_error(_api: &Api, module: &str) -> String {
    format!(
        "{NOTICE}\ndefmodule {module}.ApiError do\n  @moduledoc \"An HTTP response outside the 2xx range.\"\n  defexception [:status, :body, :headers]\n\n  @type t :: %__MODULE__{{status: pos_integer(), body: term(), headers: list()}}\n\n  @impl true\n  def message(%__MODULE__{{status: status}}), do: \"Poolster API request failed with status #{{status}}\"\nend\n"
    )
}

pub(crate) fn render_declared_errors(module: &str, operations: &[Operation]) -> String {
    let mut output = NOTICE.to_owned();
    for operation in operations {
        for response in operation
            .responses
            .iter()
            .filter(|response| is_error_status(&response.status))
        {
            let error = declared_error_name(operation, &response.status);
            let _ = writeln!(
                output,
                "\ndefmodule {module}.Errors.{error} do\n  @moduledoc \"Declared {} response for `{}`.\"\n  defexception [:status, :body, :headers]\n\n  @type t :: %__MODULE__{{status: pos_integer(), body: term(), headers: list()}}\n\n  @impl true\n  def message(%__MODULE__{{status: status}}), do: \"Poolster API request failed with status #{{status}}\"\nend",
                escape_elixir_string(&response.status),
                escape_elixir_string(&operation.id),
            );
        }
    }
    output
}

pub(crate) fn is_error_status(status: &str) -> bool {
    status == "default"
        || status
            .parse::<u16>()
            .is_ok_and(|status| (400..600).contains(&status))
}

pub(crate) fn declared_error_name(operation: &Operation, status: &str) -> String {
    let suffix = if status == "default" {
        "Default".into()
    } else {
        format!("Status{status}")
    };
    format!("{}{}Error", pascal_case(&operation.id), suffix)
}

pub(crate) fn operation_error_types(module: &str, operation: &Operation) -> String {
    let entries = operation
        .responses
        .iter()
        .filter(|response| is_error_status(&response.status))
        .map(|response| {
            let key = response
                .status
                .parse::<u16>()
                .map(|status| status.to_string())
                .unwrap_or_else(|_| ":default".into());
            format!(
                "{key} => {module}.Errors.{}",
                declared_error_name(operation, &response.status)
            )
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "%{}".into()
    } else {
        format!("%{{{}}}", entries.join(", "))
    }
}
