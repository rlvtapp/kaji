//! Models emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn render_model_for_api(api: &Api, schema: &Schema) -> String {
    fn recursive(
        api: &Api,
        value: &SchemaValue,
        target: &str,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        match &value.kind {
            SchemaKind::Reference { reference } => {
                let name = reference.rsplit('/').next().unwrap_or(reference);
                if name == target {
                    return true;
                }
                seen.insert(name.to_owned())
                    && api
                        .schemas
                        .iter()
                        .find(|schema| schema.name == name)
                        .is_some_and(|schema| recursive(api, &schema.value, target, seen))
            }
            SchemaKind::Object { fields, .. } => fields
                .iter()
                .any(|field| recursive(api, &field.value, target, seen)),
            // Arrays and dictionaries already provide value-type indirection in Swift.
            _ => false,
        }
    }
    if let SchemaKind::Object {
        fields,
        additional_properties,
    } = &schema.value.kind
    {
        if recursive(api, &schema.value, &schema.name, &mut Default::default()) {
            return render_object_kind(
                &type_name(&schema.name),
                fields,
                additional_properties,
                true,
            );
        }
    }
    render_model(schema)
}

pub(crate) fn render_model(schema: &Schema) -> String {
    let name = type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => render_object(&name, fields, additional_properties),
        SchemaKind::String if !schema.value.enum_values.is_empty() => {
            render_enum(&name, &schema.value.enum_values)
        }
        _ => format!(
            "{NOTICE}\nimport Foundation\n\npublic typealias {name} = {}\n",
            swift_type(&schema.value, false)
        ),
    }
}

pub(crate) fn render_object(
    name: &str,
    fields: &[Field],
    additional: &AdditionalProperties,
) -> String {
    render_object_kind(name, fields, additional, false)
}

pub(crate) fn render_object_kind(
    name: &str,
    fields: &[Field],
    additional: &AdditionalProperties,
    recursive: bool,
) -> String {
    let names = native_names::field_names(fields, identifier, &["encode", "init", "self"]);
    let open = !matches!(additional, AdditionalProperties::Forbidden);
    let mut extra = "additionalProperties".to_owned();
    while fields
        .iter()
        .any(|field| names[&field.name].clone().trim_matches('`') == extra)
    {
        extra.push('_');
    }
    let mut present = "_poolsterPresentFields".to_owned();
    while fields
        .iter()
        .any(|field| names[&field.name].clone().trim_matches('`') == present)
        || present == extra
    {
        present.push('_');
    }
    let extra_type = match additional {
        AdditionalProperties::Schema { value } => swift_type(value, false),
        _ => "JSONValue".into(),
    };
    let kind = if recursive { "final class" } else { "struct" };
    let property = if recursive { "let" } else { "var" };
    let presence = if recursive {
        format!("private let {present}: Set<String>")
    } else {
        format!("private var {present}: Set<String> = []")
    };
    let mut output = format!(
        "{NOTICE}\nimport Foundation\n\npublic {kind} {name}: Codable, Sendable {{\n  {presence}\n"
    );
    for field in fields {
        let _ = writeln!(
            output,
            "    public {property} {}: {}",
            names[&field.name].clone(),
            swift_type(&field.value, !field.required)
        );
    }
    if open {
        let _ = writeln!(
            output,
            "    public {property} {extra}: [String: {extra_type}]"
        );
    }
    let mut args = fields
        .iter()
        .map(|field| {
            format!(
                "{}: {}{}",
                names[&field.name].clone(),
                swift_type(&field.value, !field.required),
                if field.required { "" } else { " = nil" }
            )
        })
        .collect::<Vec<_>>();
    if open {
        args.push(format!("{extra}: [String: {extra_type}] = [:]"));
    }
    let _ = writeln!(output, "\n    public init({}) {{\n  ", args.join(", "));
    if recursive {
        let _ = writeln!(output, "        self.{present} = []");
    }
    for field in fields {
        let generated = names[&field.name].clone();
        let _ = writeln!(output, "        self.{generated} = {generated}");
    }
    if open {
        let _ = writeln!(output, "        self.{extra} = {extra}");
    }
    output.push_str("    }\n\n    public init(from decoder: Decoder) throws {\n        let container = try decoder.container(keyedBy: PoolsterCodingKey.self)\n");
    let _ = writeln!(
        output,
        "        self.{present} = Set(container.allKeys.map(\\.stringValue))"
    );
    for field in fields {
        let generated = names[&field.name].clone();
        let key = format!("PoolsterCodingKey({:?})", field.name);
        let ty = swift_type(&field.value, !field.required);
        let base = ty.strip_suffix('?').unwrap_or(&ty);
        if field.required && ty.ends_with('?') {
            let _ = writeln!(
                output,
                "        guard container.contains({key}) else {{ throw DecodingError.keyNotFound({key}, .init(codingPath: decoder.codingPath, debugDescription: \"Missing required property\")) }}"
            );
        }
        let decode = if ty.ends_with('?') {
            "decodeIfPresent"
        } else {
            "decode"
        };
        let _ = writeln!(
            output,
            "        self.{generated} = try container.{decode}({base}.self, forKey: {key})"
        );
    }
    let known = fields
        .iter()
        .map(|field| format!("{:?}", field.name))
        .collect::<Vec<_>>()
        .join(", ");
    if open {
        let _ = writeln!(
            output,
            "        var poolsterExtra: [String: {extra_type}] = [:]\n        let known: Set<String> = [{known}]\n        for key in container.allKeys where !known.contains(key.stringValue) {{\n            poolsterExtra.updateValue(try container.decode({extra_type}.self, forKey: key), forKey: key.stringValue)\n        }}\n        self.{extra} = poolsterExtra"
        );
    }
    output.push_str("    }\n\n    public func encode(to encoder: Encoder) throws {\n        var container = encoder.container(keyedBy: PoolsterCodingKey.self)\n");
    if open {
        let _ = writeln!(
            output,
            "        let known: Set<String> = [{known}]\n        for (key, value) in {extra} where !known.contains(key) {{ try container.encode(value, forKey: PoolsterCodingKey(key)) }}"
        );
    }
    for field in fields {
        let generated = format!("self.{}", names[&field.name]);
        let key = format!("PoolsterCodingKey({:?})", field.name);
        let ty = swift_type(&field.value, !field.required);
        if ty.ends_with('?') {
            let condition = if field.required {
                "true".to_owned()
            } else {
                format!("{present}.contains({:?})", field.name)
            };
            let _ = writeln!(
                output,
                "        if let value = {generated} {{ try container.encode(value, forKey: {key}) }} else if {condition} {{ try container.encodeNil(forKey: {key}) }}"
            );
        } else {
            let _ = writeln!(
                output,
                "        try container.encode({generated}, forKey: {key})"
            );
        }
    }
    output.push_str("    }\n}\n");
    output
}

pub(crate) fn render_enum(name: &str, values: &[Value]) -> String {
    let mut output = format!(
        "{NOTICE}\nimport Foundation\n\npublic enum {name}: String, Codable, Sendable {{\n  "
    );
    let mut used = BTreeSet::new();
    let mut wire_values = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let raw = value.as_str().unwrap_or_default();
        if !wire_values.insert(raw) {
            continue;
        }
        let base = enum_case(raw, index);
        let mut case = identifier(&base);
        let mut suffix = 2;
        while !used.insert(case.clone()) {
            case = identifier(&format!("{base}{suffix}"));
            suffix += 1;
        }
        let _ = writeln!(output, "    case {case} = {raw:?}");
    }
    output.push_str("}\n");
    output
}
