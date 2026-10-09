use super::*;
use poolster_core::native::ModelField;
fn descriptor(name: &str, ty: &ModelType) -> serde_json::Value {
    let mut value = serde_json::to_value(ty).unwrap();
    match &ty.kind {
        ModelKind::Object(fields) => {
            value["model_class"] = name.into();
            for (i, field) in fields.iter().enumerate() {
                value["kind"]["Object"][i]["ty"] =
                    descriptor(&format!("{name}{}", pascal_case(&field.name)), &field.ty);
            }
        }
        ModelKind::List(item) => value["kind"]["List"] = descriptor(&format!("{name}Item"), item),
        ModelKind::Union(members) => {
            for (i, item) in members.iter().enumerate() {
                value["kind"]["Union"][i] = descriptor(&format!("{name}Variant{i}"), item);
            }
        }
        _ => {}
    };
    value
}
pub(super) struct Models {
    pub files: BTreeMap<String, (String, String)>,
    pub names: BTreeSet<String>,
    pub inputs: BTreeMap<String, String>,
}
impl Models {
    pub fn ty(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<String> {
        let base = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "String",
                "Int" => "Integer",
                "Float" => "Float",
                "Boolean" => "bool",
                _ => "untyped",
            }
            .into(),
            ModelKind::Enum(v) => v
                .iter()
                .map(|v| ruby_string(v))
                .collect::<Vec<_>>()
                .join(" | "),
            ModelKind::Literal(v) => ruby_string(v),
            ModelKind::Named(n) => self
                .inputs
                .get(n)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown Ruby GraphQL input type {n}"))?,
            ModelKind::List(item) => {
                format!("Array[{}]", self.ty(&format!("{name}Item"), item, input)?)
            }
            ModelKind::Union(members) => {
                ensure!(!members.is_empty(), "empty GraphQL union");
                let mut types = vec![];
                for (i, ty) in members.iter().enumerate() {
                    types.push(self.ty(&format!("{name}Variant{i}"), ty, input)?);
                }
                types.join(" | ")
            }
            ModelKind::Object(_) => {
                self.model(name, ty, input)?;
                name.into()
            }
        };
        Ok(if ty.nullable {
            format!("({base})?")
        } else {
            base
        })
    }
    pub fn model(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<()> {
        ensure!(
            name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "invalid Ruby GraphQL model name {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "Ruby GraphQL model collision {name}"
        );
        let ModelKind::Object(fields) = &ty.kind else {
            anyhow::bail!("Ruby GraphQL result must be an object")
        };
        let mut getters = String::new();
        let mut sig = String::new();
        let mut used = BTreeSet::new();
        for ModelField {
            name: field,
            ty,
            optional,
            ..
        } in fields
        {
            let getter = field_method(field)?;
            ensure!(
                used.insert(getter.clone()),
                "Ruby GraphQL field naming collision {getter}"
            );
            let ty = self.ty(&format!("{name}{}", pascal_case(field)), ty, input)?;
            writeln!(
                getters,
                "    def {getter}; @value[{}]; end",
                ruby_string(field)
            )?;
            writeln!(
                sig,
                "    def {getter}: () -> {}",
                if *optional { format!("({ty})?") } else { ty }
            )?;
        }
        let mut source = String::new();
        let mut signatures = String::new();
        writeln!(
            source,
            "  class {name} < Model\n    TYPE = JSON.parse({}).freeze\n    def initialize(value = {{}})\n      super(self.class::TYPE, value, {input})\n    end\n{getters}  end\n  Model.register({name}::TYPE, {name})",
            ruby_string(&serde_json::to_string(&descriptor(name, ty))?)
        )?;
        writeln!(
            signatures,
            "  class {name} < Model\n    def initialize: (?Hash[String | Symbol, untyped]) -> void\n{sig}  end"
        )?;
        let original = self
            .inputs
            .iter()
            .find(|(_, class)| class.as_str() == name)
            .map(|(original, _)| original.clone());
        if let Some(original) = original {
            writeln!(
                source,
                "  INPUT_CLASSES[{}] = {name}",
                ruby_string(&original)
            )?;
        }
        self.files.insert(name.into(), (source, signatures));
        Ok(())
    }
}
fn field_method(name: &str) -> Result<String> {
    let mut name = snake_case(name);
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid Ruby GraphQL field {name}"
    );
    if [
        "class",
        "initialize",
        "to_h",
        "present",
        "send",
        "object_id",
        "respond_to",
        "validate",
        "hash",
        "inspect",
        "method",
        "methods",
        "public_send",
        "nil",
        "new",
        "end",
        "def",
        "module",
        "private",
        "protected",
        "public",
        "freeze",
        "frozen",
        "clone",
        "dup",
        "tap",
        "then",
        "display",
    ]
    .contains(&name.as_str())
    {
        name = format!("field_{name}");
    }
    Ok(name)
}
