use super::*;

#[derive(Default)]
pub(super) struct NamedTypes {
    pub(super) models: BTreeSet<String>,
    pub(super) enums: BTreeSet<String>,
    pub(super) enum_wire_types: BTreeMap<String, String>,
    pub(super) wrappers: BTreeSet<String>,
}

impl NamedTypes {
    pub(super) fn from_api(api: &Api) -> Self {
        let mut types = Self::default();
        for schema in &api.schemas {
            let name = type_name(&schema.name);
            match schema.value.kind {
                SchemaKind::Object { .. } => {
                    types.models.insert(name);
                }
                _ if is_backed_enum(&schema.value) => {
                    types.enum_wire_types.insert(
                        name.clone(),
                        if schema
                            .value
                            .enum_values
                            .iter()
                            .all(|value| value.as_i64().is_some())
                        {
                            "int"
                        } else {
                            "string"
                        }
                        .into(),
                    );
                    types.enums.insert(name);
                }
                _ => {
                    types.wrappers.insert(name);
                }
            }
        }
        types
    }

    pub(super) fn is_class(&self, name: &str) -> bool {
        self.models.contains(name) || self.enums.contains(name) || self.wrappers.contains(name)
    }
}

pub(super) fn composer_json(api: &Api, package_name: &str, namespace: &str) -> String {
    let autoload_namespace = format!("{}\\\\", namespace.replace('\\', "\\\\"));
    format!(
        "{{\n  \"name\": \"{package_name}\",\n  \"description\": \"Generated PHP SDK for {}\",\n  \"type\": \"library\",\n  \"require\": {{\n    \"php\": \">=8.2\",\n    \"psr/http-client\": \"^1.0\",\n    \"psr/http-factory\": \"^1.0\",\n    \"psr/http-message\": \"^1.0 || ^2.0\",\n    \"nyholm/psr7\": \"^1.8\"\n  }},\n  \"autoload\": {{\n    \"psr-4\": {{\n      \"{autoload_namespace}\": \"src/\"\n    }}\n  }}\n}}\n",
        escape_json(&api.name),
    )
}

pub(super) fn api_exception(namespace: &str) -> String {
    format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Exceptions;\n\nuse RuntimeException;\n\nfinal class ApiException extends RuntimeException\n{{\n    public function __construct(\n        string $message,\n        public readonly int $statusCode,\n        public readonly string $responseBody = '',\n    ) {{\n        parent::__construct($message, $statusCode);\n    }}\n}}\n"
    )
}

pub(super) fn render_model(schema: &Schema, namespace: &str, named_types: &NamedTypes) -> String {
    let name = type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => render_object_model(&name, fields, additional_properties, namespace, named_types),
        _ if is_backed_enum(&schema.value) => render_enum(&name, &schema.value, namespace),
        _ => render_value_model(&name, &schema.value, namespace, named_types),
    }
}

pub(super) fn render_object_model(
    name: &str,
    fields: &[poolster_core::Field],
    additional_properties: &AdditionalProperties,
    namespace: &str,
    named_types: &NamedTypes,
) -> String {
    let open = !matches!(additional_properties, AdditionalProperties::Forbidden);
    let mut extra_name = "additionalProperties".to_owned();
    while fields
        .iter()
        .any(|field| php_field_identifier(fields, field) == extra_name)
    {
        extra_name.push('_');
    }
    let mut presence_name = "poolsterPresentFields".to_owned();
    while fields
        .iter()
        .any(|field| php_field_identifier(fields, field) == presence_name)
        || presence_name == extra_name
    {
        presence_name.push('_');
    }
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Models;\n\nuse JsonSerializable;\n\n{NOTICE}\nfinal class {name} implements JsonSerializable\n{{\n    private ?array ${presence_name} = null;\n\n    public function __construct(\n"
    );
    for field in fields {
        let field_name = php_field_identifier(fields, field);
        let type_name = nullable_type(
            &php_type(&field.value, named_types),
            field.value.nullable || !field.required,
        );
        let default = if field.required { "" } else { " = null" };
        let _ = writeln!(
            output,
            "        public readonly {type_name} ${field_name}{default},"
        );
    }
    if open {
        let value_type = match additional_properties {
            AdditionalProperties::Schema { value } => php_type(value, named_types),
            _ => "mixed".into(),
        };
        let _ = writeln!(
            output,
            "        public readonly array ${extra_name} = [], // array<string, {value_type}>"
        );
    }
    output.push_str("    ) {\n    }\n\n    /** @param array<string, mixed> $data */\n    public static function fromArray(array $data): self\n    {\n        $instance = new self(\n");
    for field in fields {
        let field_name = php_field_identifier(fields, field);
        let source = format!("$data[{}]", php_string(&field.name));
        let value = from_value(&source, &field.value, named_types);
        if field.required {
            if field.value.nullable {
                let _ = writeln!(
                    output,
                    "            {field_name}: {source} === null ? null : {value},"
                );
            } else {
                let _ = writeln!(output, "            {field_name}: {value},");
            }
        } else {
            let _ = writeln!(
                output,
                "            {field_name}: array_key_exists({}, $data) && {source} !== null ? {value} : null,",
                php_string(&field.name)
            );
        }
    }
    let known = fields
        .iter()
        .map(|field| php_string(&field.name))
        .collect::<Vec<_>>()
        .join(", ");
    if open {
        let extras = format!("array_diff_key($data, array_fill_keys([{known}], true))");
        let extras = match additional_properties {
            AdditionalProperties::Schema { value } => format!(
                "array_map(static fn(mixed $item): mixed => {}, {extras})",
                from_value("$item", value, named_types)
            ),
            _ => extras,
        };
        let _ = writeln!(output, "            {extra_name}: {extras},");
    }
    let _ = writeln!(
        output,
        "        );\n        $instance->{presence_name} = array_keys($data);\n        return $instance;\n    }}\n\n    /** Return a copy with explicit nulls present at the given wire keys. */\n    public function withPresentFields(array $fields): self\n    {{\n        foreach ($fields as $field) {{\n            if (!is_string($field) || !in_array($field, [{known}], true)) throw new \\InvalidArgumentException('unknown model wire field');\n        }}\n        $copy = clone $this;\n        $copy->{presence_name} = array_values(array_unique(array_merge($this->{presence_name} ?? [], $fields)));\n        return $copy;\n    }}\n\n    public function jsonSerialize(): object\n    {{\n        $value = [];"
    );
    if open {
        let _ = writeln!(
            output,
            "        $value = array_diff_key($this->{extra_name}, array_fill_keys([{known}], true));"
        );
    }
    for field in fields {
        let generated = php_field_identifier(fields, field);
        let key = php_string(&field.name);
        if field.required {
            let _ = writeln!(output, "        $value[{key}] = $this->{generated};");
        } else {
            let _ = writeln!(
                output,
                "        if ($this->{generated} !== null || in_array({key}, $this->{presence_name} ?? [], true)) {{ $value[{key}] = $this->{generated}; }}"
            );
        }
    }
    output.push_str("        return (object) $value;\n    }\n}\n");
    output
}

pub(super) fn render_enum(name: &str, value: &SchemaValue, namespace: &str) -> String {
    let kind = if value
        .enum_values
        .iter()
        .all(|value| value.as_i64().is_some())
    {
        "int"
    } else {
        "string"
    };
    let mut output = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Models;\n\n{NOTICE}\nenum {name}: {kind}\n{{\n"
    );
    let mut used = BTreeSet::new();
    let mut backing_values = BTreeSet::new();
    for (index, value) in value.enum_values.iter().enumerate() {
        let case = unique_name(enum_case(value, index), &mut used);
        let backing = if kind == "int" {
            value.as_i64().unwrap_or_default().to_string()
        } else {
            php_string(value.as_str().unwrap_or_default())
        };
        if backing_values.insert(backing.clone()) {
            let _ = writeln!(output, "    case {case} = {backing};");
        }
    }
    output.push_str("}\n");
    output
}

pub(super) fn render_value_model(
    name: &str,
    value: &SchemaValue,
    namespace: &str,
    named_types: &NamedTypes,
) -> String {
    let type_name = php_type(value, named_types);
    format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\Models;\n\nuse JsonSerializable;\n\n{NOTICE}\n/** Wrapper for the {name} schema. */\nfinal class {name} implements JsonSerializable\n{{\n    public function __construct(public readonly {type_name} $value)\n    {{\n    }}\n\n    public static function from(mixed $value): self\n    {{\n        return new self($value);\n    }}\n\n    public function jsonSerialize(): mixed\n    {{\n        return $this->value;\n    }}\n}}\n"
    )
}
