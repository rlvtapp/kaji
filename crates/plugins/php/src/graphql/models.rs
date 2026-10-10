use super::*;
fn php_value(value: &serde_json::Value, indentation: usize) -> String {
    let nested = " ".repeat(indentation + 4);
    let closing = " ".repeat(indentation);
    match value {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::String(v) => quote(v),
        serde_json::Value::Array(values) if !values.is_empty() => format!(
            "[\n{}\n{closing}]",
            values
                .iter()
                .map(|value| format!("{nested}{},", php_value(value, indentation + 4)))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        serde_json::Value::Object(values) if !values.is_empty() => format!(
            "[\n{}\n{closing}]",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{nested}{} => {},",
                    quote(key),
                    php_value(value, indentation + 4)
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => "[]".into(),
        v => v.to_string(),
    }
}
fn doc_type(spec: &serde_json::Value) -> String {
    let base = match spec["kind"].as_str().unwrap() {
        "object" => format!("\\{}", spec["class"].as_str().unwrap()),
        "list" => format!("list<{}>", doc_type(&spec["item"])),
        "enum" | "literal" => spec["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| quote(v.as_str().unwrap()))
            .collect::<Vec<_>>()
            .join("|"),
        "union" => spec["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(doc_type)
            .collect::<Vec<_>>()
            .join("|"),
        _ => match spec["name"].as_str().unwrap() {
            "ID" if spec["input"] == true => "string|int",
            "ID" | "String" => "string",
            "Int" => "int",
            "Float" => "float",
            "Boolean" => "bool",
            _ => "mixed",
        }
        .into(),
    };
    if spec["nullable"] == true && base != "mixed" {
        format!("{base}|null")
    } else {
        base
    }
}
pub(super) struct Models {
    pub(super) namespace: String,
    pub(super) files: BTreeMap<String, String>,
    pub(super) names: BTreeSet<String>,
}
impl Models {
    fn ty(
        &mut self,
        name: &str,
        ty: &ModelType,
        input: bool,
    ) -> Result<(String, serde_json::Value)> {
        let (php, mut spec) = match &ty.kind {
            ModelKind::Scalar(s) => (
                match s.as_str() {
                    "ID" if input => "string|int",
                    "String" | "ID" => "string",
                    "Int" => "int",
                    "Float" => "float",
                    "Boolean" => "bool",
                    _ => "mixed",
                }
                .to_string(),
                serde_json::json!({"kind":"scalar","name":s,"input":input}),
            ),
            ModelKind::Enum(v) => (
                "string".into(),
                serde_json::json!({"kind":"enum","values":v}),
            ),
            ModelKind::Literal(v) => (
                "string".into(),
                serde_json::json!({"kind":"literal","values":[v]}),
            ),
            ModelKind::Named(n) => (
                ident(n)?,
                serde_json::json!({"kind":"object","class":format!("{}\\{n}",self.namespace)}),
            ),
            ModelKind::Object(fields) => {
                self.object(name, fields, input)?;
                (
                    name.into(),
                    serde_json::json!({"kind":"object","class":format!("{}\\{name}",self.namespace)}),
                )
            }
            ModelKind::List(item) => {
                let (_, spec) = self.ty(&format!("{name}Item"), item, input)?;
                (
                    "array".into(),
                    serde_json::json!({"kind":"list","item":spec}),
                )
            }
            ModelKind::Union(members) => {
                ensure!(!members.is_empty(), "empty GraphQL union");
                let discriminator = members.first().and_then(|m| {
                    if let ModelKind::Object(fields) = &m.kind {
                        fields
                            .iter()
                            .find(|f| matches!(f.ty.kind, ModelKind::Literal(_)))
                            .map(|f| f.name.clone())
                    } else {
                        None
                    }
                });
                let mut literals = BTreeSet::new();
                ensure!(
                    discriminator
                        .as_ref()
                        .is_some_and(|key| members.iter().all(|member| {
                            if let ModelKind::Object(fields) = &member.kind {
                                fields.iter().find(|f| &f.name == key).is_some_and(|f| {
                                    if let ModelKind::Literal(value) = &f.ty.kind {
                                        literals.insert(value.clone())
                                    } else {
                                        false
                                    }
                                })
                            } else {
                                false
                            }
                        })),
                    "PHP GraphQL union selections require a selected __typename discriminator (aliases supported)"
                );
                let mut specs = vec![];
                for (i, item) in members.iter().enumerate() {
                    specs.push(self.ty(&format!("{name}Variant{i}"), item, input)?.1);
                }
                (
                    "mixed".into(),
                    serde_json::json!({"kind":"union","members":specs}),
                )
            }
        };
        spec["nullable"] = ty.nullable.into();
        let php = if ty.nullable && php != "mixed" {
            if php.contains('|') {
                format!("{php}|null")
            } else {
                format!("?{php}")
            }
        } else {
            php
        };
        Ok((php, spec))
    }
    pub(super) fn object(&mut self, name: &str, fields: &[ModelField], input: bool) -> Result<()> {
        ident(name)?;
        ensure!(
            self.names.insert(name.to_ascii_lowercase()),
            "PHP GraphQL generated type collision {name}"
        );
        let mut source = String::new();
        let mut props = vec![];
        let mut decode = vec![];
        let mut encode = vec![];
        let mut used = BTreeSet::new();
        let mut docs = vec![];
        let mut specs = Vec::new();
        for field in fields {
            let key = ident(&field.name)?;
            ensure!(used.insert(key.clone()), "duplicate field {key}");
            let (php, spec) = self.ty(&format!("{name}{}", upper(&key)), &field.ty, input)?;
            let doc = doc_type(&spec);
            let q = quote(&key);
            specs.push(format!("        {q} => {},", php_value(&spec, 8)));
            let spec = format!("self::FIELD_TYPES[{q}]");
            props.push((field.optional, key.clone(), php, doc.clone()));
            docs.push(format!(
                "@param {} ${key}",
                if field.optional {
                    format!("Presence<{doc}>|null")
                } else {
                    doc
                }
            ));
            if field.optional {
                decode.push(format!("            {key}: array_key_exists({q}, $data)\n                ? Presence::of(decodeValue($data[{q}], {spec}))\n                : Presence::missing(),"));
                encode.push(format!("        if ($this->{key}->present) {{\n            $out[{q}] = encodeValue($this->{key}->value);\n            decodeValue($out[{q}], {spec});\n        }}"));
            } else {
                decode.push(format!("            {key}: decodeValue(\n                array_key_exists({q}, $data)\n                    ? $data[{q}]\n                    : throw new \\UnexpectedValueException('Missing field {key}'),\n                {spec},\n            ),"));
                encode.push(format!(
                    "        $out[{q}] = encodeValue($this->{key});\n        decodeValue($out[{q}], {spec});"
                ));
            }
        }
        props.sort_by_key(|v| v.0);
        // Optional constructor arguments accept null as an omission sentinel, then normalize to Presence.
        let mut constructor = vec![];
        let mut declarations = String::new();
        let mut init = String::new();
        for (optional, key, php, doc) in props {
            if optional {
                writeln!(
                    declarations,
                    "    /** @var Presence<{doc}> */\n    public Presence ${key};"
                )
                .unwrap();
                constructor.push(format!("        ?Presence ${key} = null,"));
                writeln!(
                    init,
                    "        $this->{key} = ${key} ?? Presence::missing();"
                )
                .unwrap();
            } else {
                constructor.push(format!("        public {php} ${key},"));
            }
        }
        let implementation = if input {
            " implements \\JsonSerializable"
        } else {
            ""
        };
        writeln!(source, "final readonly class {name}{implementation}\n{{")?;
        source.push_str(&declarations);
        if !docs.is_empty() {
            writeln!(source, "    /**")?;
            for doc in docs {
                writeln!(source, "     * {doc}")?;
            }
            writeln!(source, "     */")?;
        }
        writeln!(
            source,
            "    public function __construct(\n{}\n    ) {{\n{init}    }}",
            constructor.join("\n")
        )?;
        writeln!(
            source,
            "\n    public static function fromArray(array $data): self\n    {{\n        return new self(\n{}\n        );\n    }}",
            decode.join("\n")
        )?;
        writeln!(
            source,
            "\n    public const FIELD_TYPES = [\n{}\n    ];",
            specs.join("\n")
        )?;
        if input {
            writeln!(
                source,
                "\n    public function jsonSerialize(): mixed\n    {{\n        $out = [];\n{}\n        return (object) $out;\n    }}",
                encode.join("\n")
            )?;
        }
        source.push_str("}\n");
        self.files.insert(name.into(), source);
        Ok(())
    }
}
