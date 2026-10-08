use super::*;

pub(super) fn render_pyproject(package_name: &str, version: &str) -> String {
    format!(
        "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n\n[project]\nname = {package_name:?}\nversion = {:?}\ndescription = \"Generated Python client for {}\"\nrequires-python = \">=3.10\"\nlicense = {{ text = \"MIT\" }}\nclassifiers = [\n  \"Programming Language :: Python :: 3\",\n  \"Programming Language :: Python :: 3 :: Only\",\n  \"Typing :: Typed\",\n]\n\n[tool.hatch.build.targets.wheel]\npackages = [\"src/{}\"]\n",
        python_package_version(version),
        package_name,
        python_module_name(package_name),
    )
}

pub(super) fn render_models_init(api: &Api) -> String {
    let mut output = format!(
        "{NOTICE}\nfrom __future__ import annotations\n\nfrom dataclasses import MISSING as _MISSING, fields as _fields, is_dataclass as _is_dataclass\nfrom typing import Any as _Any\n\n\ndef _to_wire(value: _Any) -> _Any:\n    \"\"\"Convert generated dataclasses into JSON-compatible values.\"\"\"\n    if _is_dataclass(value) and not isinstance(value, type):\n        result = {{}}\n        present = next((getattr(value, item.name) for item in _fields(value) if item.metadata.get(\"present_fields\")), None)\n        for item in _fields(value):\n            current = getattr(value, item.name)\n            if item.metadata.get(\"additional_properties\"):\n                result.update(_to_wire(current))\n        for item in _fields(value):\n            current = getattr(value, item.name)\n            if not item.metadata.get(\"internal\") and not item.metadata.get(\"additional_properties\") and (current is not None or (present is not None and item.metadata.get(\"wire_name\", item.name) in present) or (item.default is _MISSING and item.default_factory is _MISSING)):\n                result[item.metadata.get(\"wire_name\", item.name)] = _to_wire(current)\n        return result\n    if isinstance(value, list):\n        return [_to_wire(item) for item in value]\n    if isinstance(value, dict):\n        return {{key: _to_wire(item) for key, item in value.items()}}\n    return value\n\n"
    );
    for index in 0..api.schemas.len().div_ceil(100) {
        let _ = writeln!(output, "from .chunks.exports_{index:03} import *");
    }
    output
}

pub(super) fn render_model_exports(schemas: &[Schema]) -> String {
    let mut output = format!("{NOTICE}from __future__ import annotations\n\n");
    for schema in schemas {
        let name = python_type_name(&schema.name);
        let file = schema_file_name(&schema.name);
        let _ = writeln!(output, "from ..{file} import {name}");
    }
    output
}

pub(super) fn render_resources_init(resources: usize) -> String {
    let mut output = format!("{NOTICE}from __future__ import annotations\n\n");
    for index in 0..resources.div_ceil(100) {
        let _ = writeln!(output, "from .chunks.exports_{index:03} import *");
    }
    output
}

pub(super) fn render_resource_exports(resources: &[String]) -> String {
    let mut output = format!("{NOTICE}from __future__ import annotations\n\n");
    for resource in resources {
        let file = schema_file_name(resource);
        let class = format!("{}Resource", pascal_case(resource));
        let _ = writeln!(output, "from ..{file} import {class}");
    }
    output
}

pub(super) fn render_model(schema: &Schema) -> String {
    let mut output = format!(
        "{NOTICE}from __future__ import annotations\n\nfrom dataclasses import dataclass, field as _poolster_field\nfrom typing import Any, Literal, TYPE_CHECKING\nfrom ._model_codec import decode_model_value\n\nif TYPE_CHECKING:\n    from typing import TypeAlias\n\n"
    );
    let mut references = std::collections::BTreeSet::new();
    python_references(&schema.value, &mut references);
    references.remove(&python_type_name(&schema.name));
    if !references.is_empty() {
        output.push_str("if TYPE_CHECKING:\n");
        for reference in references {
            let _ = writeln!(output, "    from . import {reference}");
        }
        output.push('\n');
    }
    render_schema(&mut output, schema);
    output
}

pub(super) fn python_references(
    value: &SchemaValue,
    names: &mut std::collections::BTreeSet<String>,
) {
    match &value.kind {
        SchemaKind::Reference { reference } => {
            names.insert(python_type_name(
                reference.rsplit('/').next().unwrap_or(reference),
            ));
        }
        SchemaKind::Array { items } => python_references(items, names),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                python_references(&field.value, names);
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                python_references(value, names);
            }
        }
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            for variant in variants {
                python_references(variant, names);
            }
        }
        SchemaKind::Not { schema } => python_references(schema, names),
        _ => {}
    }
}

pub(super) fn python_decode_shape(value: &SchemaValue) -> String {
    match &value.kind {
        SchemaKind::Reference { reference } => format!(
            "(\"ref\", {:?})",
            python_type_name(reference.rsplit('/').next().unwrap_or(reference))
        ),
        SchemaKind::Array { items } => format!("(\"array\", {})", python_decode_shape(items)),
        SchemaKind::Object { fields, .. } => format!(
            "(\"object\", {{{}}})",
            fields
                .iter()
                .map(|field| format!("{:?}: {}", field.name, python_decode_shape(&field.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "None".into(),
    }
}

pub(super) fn render_schema(output: &mut String, schema: &Schema) {
    let name = python_type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let mut additional_field = "additional_properties".to_owned();
            while fields
                .iter()
                .any(|item| python_field_identifier(fields, item) == additional_field)
            {
                additional_field.insert(0, '_');
            }
            let mut present_field = "_poolster_present_fields".to_owned();
            while fields
                .iter()
                .any(|item| python_field_identifier(fields, item) == present_field)
                || present_field == additional_field
            {
                present_field.insert(0, '_');
            }
            output.push_str("@dataclass\n");
            let _ = writeln!(output, "class {name}:");
            // Required fields must precede fields with defaults in a dataclass.
            for required in [true, false] {
                for item in fields.iter().filter(|item| item.required == required) {
                    let field_name = python_field_identifier(fields, item);
                    let field_type = python_type(&item.value);
                    if item.required {
                        if field_name == item.name {
                            let _ = writeln!(output, "    {field_name}: {field_type}");
                        } else {
                            let _ = writeln!(
                                output,
                                "    {field_name}: {field_type} = _poolster_field(metadata={{\"wire_name\": {:?}}})",
                                item.name
                            );
                        }
                    } else if field_name == item.name {
                        let _ = writeln!(output, "    {field_name}: {field_type} | None = None");
                    } else {
                        let _ = writeln!(
                            output,
                            "    {field_name}: {field_type} | None = _poolster_field(default=None, metadata={{\"wire_name\": {:?}}})",
                            item.name
                        );
                    }
                }
            }
            if !matches!(additional_properties, AdditionalProperties::Forbidden) {
                let additional_type = match additional_properties {
                    AdditionalProperties::Schema { value } => python_type(value),
                    _ => "Any".to_owned(),
                };
                let _ = writeln!(
                    output,
                    "    {additional_field}: dict[str, {}] = _poolster_field(default_factory=dict, metadata={{\"additional_properties\": True}})",
                    additional_type
                );
            }
            let _ = writeln!(
                output,
                "    {present_field}: frozenset[str] | None = _poolster_field(default=None, init=False, repr=False, compare=False, metadata={{\"internal\": True, \"present_fields\": True}})"
            );
            output.push_str("\n    @classmethod\n");
            let _ = writeln!(
                output,
                "    def from_dict(cls, value: dict[str, Any]) -> \"{name}\":"
            );
            if fields.is_empty() && matches!(additional_properties, AdditionalProperties::Forbidden)
            {
                output.push_str("        return cls()\n");
            } else {
                output.push_str("        instance = cls(\n");
                for item in fields {
                    let field_name = python_field_identifier(fields, item);
                    let accessor = if item.required {
                        format!("value[{key:?}]", key = item.name)
                    } else {
                        format!("value.get({key:?})", key = item.name)
                    };
                    let shape = python_decode_shape(&item.value);
                    let accessor = if shape == "None" {
                        accessor
                    } else {
                        format!("decode_model_value({accessor}, {shape})")
                    };
                    let _ = writeln!(output, "            {field_name}={accessor},");
                }
                if !matches!(additional_properties, AdditionalProperties::Forbidden) {
                    let declared = fields
                        .iter()
                        .map(|item| format!("{:?}", item.name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let item = match additional_properties {
                        AdditionalProperties::Schema { value } => {
                            format!("decode_model_value(item, {})", python_decode_shape(value))
                        }
                        _ => "item".into(),
                    };
                    let _ = writeln!(
                        output,
                        "            {additional_field}={{key: {item} for key, item in value.items() if key not in [{declared}]}},"
                    );
                }
                let _ = writeln!(
                    output,
                    "        )\n        instance.{present_field} = frozenset(value)\n        return instance"
                );
            }
        }
        _ if !schema.value.enum_values.is_empty() => {
            let values = schema
                .value
                .enum_values
                .iter()
                .map(python_literal)
                .collect::<Vec<_>>();
            let future = if schema
                .value
                .extensions
                .get("x-poolster-open-enum")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                let mut primitives = std::collections::BTreeSet::new();
                for value in &schema.value.enum_values {
                    primitives.insert(if value.is_string() {
                        "str"
                    } else if value.is_boolean() {
                        "bool"
                    } else if value.is_i64() || value.is_u64() {
                        "int"
                    } else if value.is_number() {
                        "float"
                    } else {
                        "None"
                    });
                }
                format!(
                    " | {}",
                    primitives.into_iter().collect::<Vec<_>>().join(" | ")
                )
            } else {
                String::new()
            };
            let _ = writeln!(output, "{name} = Literal[{}]{future}", values.join(", "));
        }
        _ => {
            let mut references = std::collections::BTreeSet::new();
            python_references(&schema.value, &mut references);
            let annotation = python_type(&schema.value);
            if references.is_empty() {
                let _ = writeln!(output, "{name} = {annotation}");
            } else {
                // Future annotations do not defer a type alias assignment. A
                // PEP 613 forward alias also avoids circular model imports.
                let _ = writeln!(output, "{name}: TypeAlias = {annotation:?}");
            }
        }
    }
}
