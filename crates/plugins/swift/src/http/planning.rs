//! Planning emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn native_api(api: &Api) -> Api {
    let resource_types: Vec<_> = operation_groups(api)
        .keys()
        .map(|resource| format!("{resource}Resource"))
        .collect();
    native_names::prepare(
        api,
        type_name,
        function_name,
        &[
            "String",
            "Bool",
            "Int",
            "Int64",
            "UInt",
            "UInt64",
            "Float",
            "Double",
            "Data",
            "URL",
            "URLComponents",
            "URLRequest",
            "URLResponse",
            "HTTPURLResponse",
            "URLSession",
            "URLQueryItem",
            "JSONDecoder",
            "JSONEncoder",
            "JSONValue",
            "Error",
            "TimeInterval",
            "Locale",
            "TimeZone",
            "DateFormatter",
            "ISO8601DateFormatter",
            "CharacterSet",
            "UUID",
            "Decimal",
            "Date",
            "Task",
            "Never",
            "Any",
            "Optional",
            "Array",
            "Dictionary",
            "Set",
            "PoolsterClient",
            "PoolsterClientOptions",
            "PoolsterAPIError",
            "PoolsterTransport",
            "PoolsterCodingKey",
        ]
        .into_iter()
        .chain(resource_types.iter().map(String::as_str))
        .collect::<Vec<_>>(),
    )
}
