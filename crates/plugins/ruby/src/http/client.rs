use super::*;

pub(crate) fn render_client(api: &Api, module: &str, style: SdkClientStyle) -> String {
    render_client_impl(api, module, style, true)
}
pub(crate) fn render_client_impl(
    api: &Api,
    module: &str,
    style: SdkClientStyle,
    include_operations: bool,
) -> String {
    let mut out = format!(
        "{NOTICE}require \"net/http\"\nrequire \"uri\"\nrequire \"json\"\nrequire \"cgi\"\n\nmodule {module}\n"
    );
    out = out.replacen(
        &format!("module {module}\n"),
        &format!("require_relative \"response_validation\"\n\nmodule {module}\n"),
        1,
    );
    if !include_operations && style == SdkClientStyle::Namespaced {
        out.push_str("  ResourceFactories = {}\n\n");
    }
    out.push_str(include_str!("../../templates/multipart.rb.txt"));
    out.push_str("  class PoolsterCancellationError < StandardError; end\n  class PoolsterTimeoutError < Timeout::Error; end\n  class ApiError < StandardError\n    attr_reader :status, :body\n    def initialize(status, body)\n      @status = status\n      @body = body\n      super(\"API request failed with status #{status}\")\n    end\n  end\n\n  class Client\n    def initialize(base_url:, api_key: nil, bearer_token: nil, headers: {}, timeout: 30, transport: nil, middleware: [], validate_responses: false, max_attempts: 1, retry_base_delay: 0.5, retry_max_delay: 30, cancelled: nil, token_provider: nil)\n      @base_url = base_url.sub(%r{/$}, \"\")\n      @api_key = api_key\n      @bearer_token = bearer_token\n      @headers = headers.transform_keys(&:to_s)\n      @timeout = timeout\n      raise ArgumentError, \"invalid retry configuration\" unless max_attempts.is_a?(Integer) && max_attempts.between?(1, 10) && retry_base_delay.is_a?(Numeric) && retry_max_delay.is_a?(Numeric) && retry_base_delay.finite? && retry_max_delay.finite? && retry_base_delay >= 0 && retry_base_delay <= 60 && retry_max_delay >= 0 && retry_max_delay <= 60\n      @max_attempts = max_attempts\n      @retry_base_delay = retry_base_delay.to_f\n      @retry_max_delay = retry_max_delay.to_f\n      @cancelled = cancelled\n      @token_provider = token_provider\n      @transport = transport\n      @validate_responses = validate_responses\n      @middleware = middleware.to_a.dup.freeze\n      raise ArgumentError, \"middleware must be callable\" unless @middleware.all? { |item| item.respond_to?(:call) }\n");
    if include_operations && style == SdkClientStyle::Namespaced {
        for resource in resource_operations(api).keys() {
            let _ = writeln!(
                out,
                "      @{} = {}Resource.new(self)",
                ruby_resource_attribute(api, resource),
                pascal_case(resource)
            );
        }
    }
    if !include_operations && style == SdkClientStyle::Namespaced {
        out.push_str("      ResourceFactories.each { |name, factory| instance_variable_set(\"@#{name}\", factory.call(self)) }\n");
    }
    out.push_str("    end\n\n");
    if include_operations && style == SdkClientStyle::Namespaced {
        for resource in resource_operations(api).keys() {
            let _ = writeln!(
                out,
                "    attr_reader :{}",
                ruby_resource_attribute(api, resource)
            );
        }
        out.push('\n');
    }
    if include_operations {
        for operation in &api.operations {
            out.push_str(&render_operation(api, operation));
            if let Some(page) = pagination::render(api, operation) {
                out.push_str(&page);
            }
        }
    }
    if api
        .operations
        .iter()
        .any(|operation| pagination::render(api, operation).is_some())
    {
        out.push_str(pagination::HELPERS);
        out.push_str("    private :poolster_json_path, :poolster_with_body_value\n\n");
    }
    out.push_str(include_str!("../../templates/retry_runtime.rb.txt"));
    out.push_str(&render_runtime());
    out.push_str("  end\n\n");
    if include_operations && style == SdkClientStyle::Namespaced {
        for (resource, operations) in resource_operations(api) {
            out.push_str(&render_resource(api, &resource, &operations));
        }
    }
    out.push_str("end\n");
    out
}

pub(crate) fn render_runtime() -> String {
    include_str!("../../templates/request_runtime.rb.txt").into()
}

pub(crate) fn render_resource(api: &Api, resource: &str, operations: &[&Operation]) -> String {
    let class = format!("{}Resource", pascal_case(resource));
    let mut out =
        format!("  class {class}\n    def initialize(client)\n      @client = client\n    end\n\n");
    for operation in operations {
        let name = ruby_identifier(&snake_case(&operation.id));
        let args = operation
            .parameters
            .iter()
            .map(|parameter| {
                let name = ruby_parameter_identifier(operation, parameter);
                if parameter.required {
                    format!("{name}:")
                } else {
                    format!("{name}: nil")
                }
            })
            .chain(operation.request_body.as_ref().map(|body| {
                if body.required {
                    "body:".to_owned()
                } else {
                    "body: nil".to_owned()
                }
            }))
            .collect::<Vec<_>>()
            .join(", ");
        let options_name = ruby_request_options_name(operation);
        let args = if args.is_empty() {
            format!("{options_name}: nil")
        } else {
            format!("{args}, {options_name}: nil")
        };
        let forwards = operation
            .parameters
            .iter()
            .map(|parameter| {
                let id = ruby_parameter_identifier(operation, parameter);
                format!("{id}: {id}")
            })
            .chain(
                operation
                    .request_body
                    .as_ref()
                    .map(|_| "body: body".to_owned()),
            )
            .collect::<Vec<_>>()
            .join(", ");
        let forwards = if forwards.is_empty() {
            format!("{options_name}: {options_name}")
        } else {
            format!("{forwards}, {options_name}: {options_name}")
        };
        let _ = writeln!(
            out,
            "    def {name}({args})\n      @client.{name}({forwards})\n    end\n"
        );
        if pagination::render(api, operation).is_some() {
            let _ = writeln!(
                out,
                "    def {name}_pages({args}, &block)\n      @client.{name}_pages({forwards}, &block)\n    end\n"
            );
        }
    }
    out.push_str("  end\n\n");
    out
}
