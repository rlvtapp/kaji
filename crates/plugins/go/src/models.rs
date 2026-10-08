use super::*;

pub(super) fn model_field_names(fields: &[poolster_core::Field]) -> BTreeMap<String, String> {
    let reserved: BTreeSet<String> = fields
        .iter()
        .map(|field| go_type_name(&field.name))
        .collect();
    let mut used = BTreeSet::new();
    fields
        .iter()
        .map(|field| {
            let base = go_type_name(&field.name);
            let mut name = base.clone();
            let mut suffix = 2;
            while used.contains(&name) || (name != base && reserved.contains(&name)) {
                name = format!("{base}{suffix}");
                suffix += 1;
            }
            used.insert(name.clone());
            (field.name.clone(), name)
        })
        .collect()
}

pub(super) fn valid_json_tag_name(name: &str) -> bool {
    !name.is_empty()
        && name != "-"
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "!#$%&()*+./:;<=>?@[]^_{|}~ -".contains(ch))
}

pub(super) fn render_schema(output: &mut String, schema: &Schema) {
    let name = go_type_name(&schema.name);
    match &schema.value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let field_names = model_field_names(fields);
            let custom_fields: Vec<_> = fields
                .iter()
                .filter(|field| !valid_json_tag_name(&field.name))
                .collect();
            let _ = writeln!(output, "type {name} struct {{");
            for field in fields {
                let field_name = &field_names[&field.name];
                let mut field_type = go_type(&field.value);
                if !field.required && !field_type.starts_with('*') {
                    field_type = format!("*{field_type}");
                }
                let omitempty = if field.required { "" } else { ",omitempty" };
                let tag = if valid_json_tag_name(&field.name) {
                    format!("{}{omitempty}", field.name)
                } else {
                    "-".into()
                };
                let _ = writeln!(output, "\t{field_name} {field_type} `json:\"{tag}\"`");
            }
            let extra_type = match additional_properties {
                AdditionalProperties::Schema { value } => Some(go_type(value)),
                AdditionalProperties::Any | AdditionalProperties::Unspecified => {
                    Some("json.RawMessage".into())
                }
                AdditionalProperties::Forbidden => None,
            };
            // Preserve declared wire fields and allocate the synthetic extension bag separately.
            let mut extra_name = "AdditionalProperties".to_owned();
            let mut suffix = 2;
            while field_names.values().any(|name| *name == extra_name) {
                extra_name = format!("AdditionalProperties{suffix}");
                suffix += 1;
            }
            let nullable = fields
                .iter()
                .filter(|field| !field.required && (field.value.nullable || field.value.nullish))
                .collect::<Vec<_>>();
            if let Some(extra_type) = &extra_type {
                let _ = writeln!(
                    output,
                    "\t{extra_name} map[string]{extra_type} `json:\"-\"`"
                );
            }
            if !nullable.is_empty() {
                output.push_str("\tpoolsterNullFields map[string]bool\n");
            }
            output.push_str("}\n");
            if extra_type.is_some() || !nullable.is_empty() || !custom_fields.is_empty() {
                let _ = writeln!(
                    output,
                    "func (model *{name}) UnmarshalJSON(data []byte) error {{\n type plain {name}; var decoded plain; if err:=json.Unmarshal(data,&decoded);err!=nil {{return err}}; *model={name}(decoded)\n var fields map[string]json.RawMessage; if err:=json.Unmarshal(data,&fields);err!=nil {{return err}}"
                );
                for field in &custom_fields {
                    let key = serde_json::to_string(&field.name).unwrap();
                    let ident = &field_names[&field.name];
                    let _ = writeln!(
                        output,
                        "if raw,ok:=fields[{key}];ok {{if err:=json.Unmarshal(raw,&model.{ident});err!=nil {{return err}}}}"
                    );
                }
                if !nullable.is_empty() {
                    output.push_str("model.poolsterNullFields=map[string]bool{}\n");
                    for field in &nullable {
                        let key = serde_json::to_string(&field.name).unwrap();
                        let _ = writeln!(
                            output,
                            "if raw,ok:=fields[{key}];ok && string(raw)==\"null\" {{model.poolsterNullFields[{key}]=true}}"
                        );
                    }
                }
                if let Some(extra_type) = &extra_type {
                    let _ = writeln!(output, "model.{extra_name}=map[string]{extra_type}{{}}");
                    for field in fields {
                        let key = serde_json::to_string(&field.name).unwrap();
                        let _ = writeln!(output, "delete(fields,{key})");
                    }
                    let _ = writeln!(
                        output,
                        "for key,raw:=range fields {{var value {extra_type};if err:=json.Unmarshal(raw,&value);err!=nil {{return err}};model.{extra_name}[key]=value}}"
                    );
                }
                output.push_str("return nil\n}\n");
                let _ = writeln!(
                    output,
                    "func (model {name}) MarshalJSON() ([]byte,error) {{\n type plain {name};encoded,err:=json.Marshal(plain(model));if err!=nil {{return nil,err}};var fields map[string]json.RawMessage;if err:=json.Unmarshal(encoded,&fields);err!=nil {{return nil,err}}"
                );
                for field in &custom_fields {
                    let key = serde_json::to_string(&field.name).unwrap();
                    let ident = &field_names[&field.name];
                    if !field.required {
                        let _ = writeln!(output, "if model.{ident}!=nil {{");
                    }
                    let _ = writeln!(
                        output,
                        "raw{ident},err:=json.Marshal(model.{ident});if err!=nil {{return nil,err}};fields[{key}]=raw{ident}"
                    );
                    if !field.required {
                        output.push_str("}\n");
                    }
                }
                if extra_type.is_some() {
                    let _ = writeln!(
                        output,
                        "for key,value:=range model.{extra_name} {{if _,exists:=fields[key];exists {{continue}};raw,err:=json.Marshal(value);if err!=nil {{return nil,err}};fields[key]=raw}}\n"
                    );
                }
                for field in &nullable {
                    let key = serde_json::to_string(&field.name).unwrap();
                    let ident = &field_names[&field.name];
                    let _ = writeln!(
                        output,
                        "if model.poolsterNullFields[{key}] && model.{ident}==nil {{fields[{key}]=json.RawMessage(\"null\")}}"
                    );
                }
                output.push_str("return json.Marshal(fields)\n}\n");
            }
        }
        SchemaKind::String if !schema.value.enum_values.is_empty() => {
            let _ = writeln!(output, "type {name} string\n");
            output.push_str("const (\n");
            let preferred: Vec<_> = schema
                .value
                .enum_values
                .iter()
                .map(|value| format!("{name}{}", go_type_name(value.as_str().unwrap_or_default())))
                .collect();
            let reserved: BTreeSet<_> = preferred.iter().cloned().collect();
            let mut used = BTreeSet::new();
            for (index, value) in schema.value.enum_values.iter().enumerate() {
                let value = value.as_str().unwrap_or_default();
                let base = &preferred[index];
                let mut constant = base.clone();
                let mut suffix = 2;
                while used.contains(&constant)
                    || (constant != *base && reserved.contains(&constant))
                {
                    constant = format!("{base}{suffix}");
                    suffix += 1;
                }
                used.insert(constant.clone());
                let _ = writeln!(output, "\t{constant} {name} = {value:?}");
                if value.is_empty() {
                    let _ = writeln!(output, "\t_ = {index}");
                }
            }
            output.push_str(")\n");
        }
        _ => {
            let _ = writeln!(output, "type {name} = {}", go_type(&schema.value));
        }
    }
}

pub(super) fn has_multipart(operation: &Operation) -> bool {
    operation.request_body.as_ref().is_some_and(|body| {
        body.media_types.iter().any(|media| {
            media
                .content_type
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .starts_with("multipart/")
        })
    })
}
