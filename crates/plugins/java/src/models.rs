//! Java model declarations and byte-budget chunking.

use super::*;

pub(super) fn render_model(schema: &Schema, package: &str, open_enums: bool) -> String {
    let name = type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => render_object_model(&name, fields, additional_properties, package),
        SchemaKind::String if !schema.value.enum_values.is_empty() => {
            if open_enums {
                render_open_enum(&name, &schema.value, package)
            } else {
                render_enum(&name, &schema.value, package)
            }
        }
        _ => render_value_model(&name, &schema.value, package),
    }
}

pub(super) fn render_model_parts(
    schema: &Schema,
    package: &str,
    open_enums: bool,
    index: usize,
) -> Result<Vec<(String, String)>> {
    let name = type_name(&schema.name);
    let whole = render_model(schema, package, open_enums);
    let SchemaKind::Object {
        fields,
        additional_properties,
    } = &schema.value.kind
    else {
        return Ok(vec![(format!("{name}.java"), whole)]);
    };
    if fields.len() <= 200 || whole.len() <= 128 * 1024 {
        return Ok(vec![(format!("{name}.java"), whole)]);
    }
    let names = native_names::field_names(
        fields,
        field_name,
        &[
            "clone",
            "getClass",
            "toString",
            "hashCode",
            "equals",
            "wait",
            "notify",
            "notifyAll",
            "finalize",
        ],
    );
    let declarations = fields.iter().map(|field| {
        let native = &names[&field.name]; let ty = java_type(&field.value);
        let omit = if field.required { "" } else { "@JsonInclude(JsonInclude.Include.NON_NULL) " };
        format!("    {omit}@JsonProperty({:?}) private {ty} {native};\n    public {ty} {native}() {{ return {native}; }}\n    public {name} {native}({ty} value) {{ this.{native} = value; return ({name}) this; }}\n", field.name)
    }).collect::<Vec<_>>();
    let imports = format!(
        "package {package}.model;\nimport com.fasterxml.jackson.annotation.*;\nimport com.fasterxml.jackson.databind.JsonNode;\nimport java.util.*;\n{NOTICE}"
    );
    let units = declarations
        .iter()
        .zip(fields)
        .map(|(source, field)| poolster_core::source_layout::SourceUnit {
            bytes: source.len() + field.name.len() + if field.required { 64 } else { 256 },
            resource: None,
        })
        .collect::<Vec<_>>();
    let groups = poolster_core::source_layout::SourceLayout::default()
        .groups(&units, imports.len() + 512)?;
    let mut files = Vec::new();
    for (part, indices) in groups.iter().enumerate() {
        let holder = format!("PoolsterModelPart{index:05}_{part:03}");
        let parent = if part == 0 {
            String::new()
        } else {
            format!(" extends PoolsterModelPart{index:05}_{:03}", part - 1)
        };
        let mut source = format!("{imports}public abstract class {holder}{parent} {{\n");
        for index in indices {
            source.push_str(&declarations[*index]);
        }
        let known = indices
            .iter()
            .map(|index| format!("{:?}", fields[*index].name))
            .collect::<Vec<_>>()
            .join(", ");
        let fallback = if part == 0 {
            String::new()
        } else {
            " || super.poolsterIsDeclaredProperty(kajiWirePropertyName)".into()
        };
        let _ = writeln!(
            source,
            "    protected boolean kajiIsDeclaredProperty(String kajiWirePropertyName) {{ return Set.of({known}).contains(kajiWirePropertyName){fallback}; }}\n}}"
        );
        files.push((format!("{holder}.java"), source));
    }
    let mut source = imports;
    if matches!(additional_properties, AdditionalProperties::Forbidden) {
        source.push_str("@JsonIgnoreProperties(ignoreUnknown = true)\n");
    }
    let _ = writeln!(
        source,
        "public final class {name} extends PoolsterModelPart{index:05}_{:03} {{",
        groups.len() - 1
    );
    if !matches!(additional_properties, AdditionalProperties::Forbidden) {
        let ty = match additional_properties {
            AdditionalProperties::Schema { value } => java_type(value),
            _ => "Object".into(),
        };
        let _ = writeln!(
            source,
            "    private final Map<String,{ty}> kajiExtra = new LinkedHashMap<>();\n    @JsonAnyGetter public Map<String,{ty}> kajiAdditionalProperties() {{ return Collections.unmodifiableMap(kajiExtra); }}\n    @JsonAnySetter public void kajiAdditionalProperty(String kajiWirePropertyName, {ty} value) {{ if (kajiIsDeclaredProperty(kajiWirePropertyName)) throw new IllegalArgumentException(\"additional property shadows declared field\"); kajiExtra.put(kajiWirePropertyName,value); }}"
        );
    }
    source.push_str("}\n");
    files.push((format!("{name}.java"), source));
    Ok(files)
}

fn render_object_model(
    name: &str,
    fields: &[Field],
    additional_properties: &AdditionalProperties,
    package: &str,
) -> String {
    let names = native_names::field_names(
        fields,
        field_name,
        &[
            "clone",
            "getClass",
            "toString",
            "hashCode",
            "equals",
            "wait",
            "notify",
            "notifyAll",
            "finalize",
            "kajiIsDeclaredProperty",
            "kajiWirePropertyName",
        ],
    );
    if fields.len() > 200 {
        let mut source = format!(
            "package {package}.model;\nimport com.fasterxml.jackson.annotation.*;\nimport com.fasterxml.jackson.databind.JsonNode;\nimport java.util.*;\npublic final class {name} {{\n"
        );
        if matches!(additional_properties, AdditionalProperties::Forbidden) {
            source = source.replace(
                "public final class",
                "@JsonIgnoreProperties(ignoreUnknown = true)\npublic final class",
            );
        }
        for field in fields {
            let native = &names[&field.name];
            let ty = java_type(&field.value);
            let omit = if field.required {
                ""
            } else {
                "@JsonInclude(JsonInclude.Include.NON_NULL) "
            };
            let _ = writeln!(
                source,
                "    {omit}@JsonProperty({:?}) private {ty} {native};\n    public {ty} {native}() {{ return {native}; }}\n    public {name} {native}({ty} value) {{ this.{native} = value; return this; }}",
                field.name
            );
        }
        if !matches!(additional_properties, AdditionalProperties::Forbidden) {
            let ty = match additional_properties {
                AdditionalProperties::Schema { value } => java_type(value),
                _ => "Object".into(),
            };
            let known = fields
                .iter()
                .map(|field| format!("{:?}", field.name))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                source,
                "    private final Map<String,{ty}> kajiExtra = new LinkedHashMap<>();\n    @JsonAnyGetter public Map<String,{ty}> kajiAdditionalProperties() {{ return Collections.unmodifiableMap(kajiExtra); }}\n    @JsonAnySetter public void kajiAdditionalProperty(String kajiWirePropertyName, {ty} value) {{ if (Set.of({known}).contains(kajiWirePropertyName)) throw new IllegalArgumentException(\"additional property shadows declared field\"); kajiExtra.put(kajiWirePropertyName,value); }}"
            );
        }
        source.push_str("}\n");
        return source;
    }
    let open = !matches!(additional_properties, AdditionalProperties::Forbidden);
    let mut output = format!(
        "package {package}.model;\n\nimport com.fasterxml.jackson.annotation.JsonAnyGetter;\nimport com.fasterxml.jackson.annotation.JsonAnySetter;\nimport com.fasterxml.jackson.annotation.JsonIgnoreProperties;\nimport com.fasterxml.jackson.annotation.JsonInclude;\nimport com.fasterxml.jackson.annotation.JsonProperty;\nimport com.fasterxml.jackson.databind.JsonNode;\nimport java.util.Collections;\nimport java.util.LinkedHashMap;\nimport java.util.List;\nimport java.util.Map;\n\n{NOTICE}\n"
    );
    if !open {
        output.push_str("@JsonIgnoreProperties(ignoreUnknown = true)\n");
    }
    let _ = writeln!(output, "public record {name}(");
    let mut components = fields
        .iter()
        .map(|field| {
            let omit = if field.required {
                ""
            } else {
                "        @JsonInclude(JsonInclude.Include.NON_NULL)\n"
            };
            format!(
                "{omit}        @JsonProperty({:?}) {} {}",
                field.name,
                java_type(&field.value),
                names[&field.name].clone()
            )
        })
        .collect::<Vec<_>>();
    let mut extra_name = "additionalProperties".to_owned();
    while fields
        .iter()
        .any(|field| names[&field.name].clone() == extra_name)
    {
        extra_name.push('_');
    }
    if open {
        let value_type = match additional_properties {
            AdditionalProperties::Schema { value } => java_type(value),
            _ => "Object".into(),
        };
        components.push(format!(
            "        @JsonAnyGetter @JsonAnySetter Map<String, {value_type}> {extra_name}"
        ));
    }
    output.push_str(&components.join(",\n"));
    if open {
        let _ = writeln!(
            output,
            "\n) {{\n    public {name} {{\n        {extra_name} = {extra_name} == null ? Map.of() : Collections.unmodifiableMap(new LinkedHashMap<>({extra_name}));"
        );
        for field in fields {
            let _ = writeln!(
                output,
                "        if ({extra_name}.containsKey({:?})) throw new IllegalArgumentException(\"additional property shadows declared field\");",
                field.name
            );
        }
        output.push_str("    }\n");
        // Preserve the prior convenience constructor for previously untyped open objects.
        if !matches!(additional_properties, AdditionalProperties::Schema { .. }) {
            let args = fields
                .iter()
                .map(|field| format!("{} {}", java_type(&field.value), names[&field.name].clone()))
                .collect::<Vec<_>>()
                .join(", ");
            let values = fields
                .iter()
                .map(|field| names[&field.name].clone())
                .chain(std::iter::once("Map.of()".into()))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(output, "    public {name}({args}) {{ this({values}); }}");
        }
        output.push_str("}\n");
    } else {
        output.push_str("\n) {}\n");
    }
    output
}

fn render_enum(name: &str, value: &SchemaValue, package: &str) -> String {
    let mut output = format!(
        "package {package}.model;\n\nimport com.fasterxml.jackson.annotation.JsonCreator;\nimport com.fasterxml.jackson.annotation.JsonValue;\n\n{NOTICE}\npublic enum {name} {{\n"
    );
    let mut used = BTreeSet::new();
    for (index, item) in value.enum_values.iter().enumerate() {
        let raw = item
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| item.to_string());
        let separator = if index + 1 == value.enum_values.len() {
            ";"
        } else {
            ","
        };
        let mut constant = enum_name(&raw, index);
        while !used.insert(constant.clone()) {
            constant.push('_');
        }
        let _ = writeln!(output, "    {constant}({raw:?}){separator}");
    }
    output.push_str("\n    private final String value;\n\n    ");
    let _ = writeln!(output, "{name}(String value) {{ this.value = value; }}");
    output.push_str("\n    @JsonValue\n    public String value() { return value; }\n\n    @JsonCreator\n    public static ");
    let _ = writeln!(output, "{name} fromValue(String value) {{");
    output.push_str("        for (var candidate : values()) {\n            if (candidate.value.equals(value)) return candidate;\n        }\n        throw new IllegalArgumentException(\"Unknown enum value: \" + value);\n    }\n}\n");
    output
}

fn render_open_enum(name: &str, value: &SchemaValue, package: &str) -> String {
    let mut output = format!(
        "package {package}.model;\n\nimport com.fasterxml.jackson.annotation.JsonCreator;\nimport com.fasterxml.jackson.annotation.JsonValue;\nimport java.util.Objects;\n\n{NOTICE}/** Extensible wire value; unknown response values are preserved. */\npublic final class {name} {{\n"
    );
    let mut constants = Vec::new();
    let mut used = BTreeSet::new();
    for (index, item) in value.enum_values.iter().enumerate() {
        let raw = item
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| item.to_string());
        let mut constant = enum_name(&raw, index);
        while !used.insert(constant.clone()) {
            constant.push('_');
        }
        let _ = writeln!(
            output,
            "    public static final {name} {constant} = new {name}({raw:?});"
        );
        constants.push(constant);
    }
    let _ = writeln!(
        output,
        "\n    private final String value;\n    private {name}(String value) {{ this.value = Objects.requireNonNull(value); }}"
    );
    output.push_str("\n    @JsonValue\n    public String value() { return value; }\n");
    let _ = writeln!(
        output,
        "    public static {name}[] values() {{ return new {name}[] {{ {} }}; }}",
        constants.join(", ")
    );
    let _ = writeln!(
        output,
        "\n    @JsonCreator\n    public static {name} fromValue(String value) {{"
    );
    output.push_str("        for (var candidate : values()) {\n            if (candidate.value.equals(value)) return candidate;\n        }\n");
    let _ = writeln!(output, "        return new {name}(value);\n    }}");
    let _ = writeln!(
        output,
        "\n    /** Reject values not declared by the API when strict request validation is desired. */\n    public static {name} fromKnownValue(String value) {{\n        for (var candidate : values()) {{\n            if (candidate.value.equals(value)) return candidate;\n        }}\n        throw new IllegalArgumentException(\"Unknown enum value: \" + value);\n    }}"
    );
    let _ = writeln!(
        output,
        "\n    public boolean isKnown() {{\n        for (var candidate : values()) if (candidate.value.equals(value)) return true;\n        return false;\n    }}\n    @Override public boolean equals(Object other) {{ return other instanceof {name} candidate && value.equals(candidate.value); }}\n    @Override public int hashCode() {{ return value.hashCode(); }}\n    @Override public String toString() {{ return value; }}\n}}"
    );
    output
}

fn render_value_model(name: &str, value: &SchemaValue, package: &str) -> String {
    format!(
        "package {package}.model;\n\nimport com.fasterxml.jackson.annotation.JsonValue;\nimport com.fasterxml.jackson.annotation.JsonCreator;\nimport com.fasterxml.jackson.databind.JsonNode;\nimport java.util.List;\nimport java.util.Map;\n\n{NOTICE}\n/** Wrapper for the {name} schema. */\npublic record {name}(@JsonValue {} value) {{\n    @JsonCreator(mode = JsonCreator.Mode.DELEGATING)\n    public {name} {{}}\n}}\n",
        java_type(value)
    )
}
