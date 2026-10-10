use super::ident;
use anyhow::{Result, ensure};
use poolster_core::native::{ModelField, ModelKind, ModelType};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
#[derive(Default)]
pub(super) struct Models {
    pub source: String,
    pub files: BTreeMap<String, String>,
    names: BTreeSet<String>,
    pub mappings: BTreeMap<String, super::GraphqlScalarMapping>,
}
impl Models {
    pub fn named_type(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<()> {
        if let ModelKind::Object(fields) = &ty.kind {
            self.object(name, fields, input)
        } else {
            let target = self.ty(name, ty, input)?;
            ensure!(
                self.names.insert(name.into()),
                "GraphQL type naming collision {name}"
            );
            let declaration = format!("type {name} = {target}\n");
            self.source.push_str(&declaration);
            self.files
                .insert(super::filename("model", name), declaration);
            Ok(())
        }
    }
    pub fn object(&mut self, name: &str, fields: &[ModelField], input: bool) -> Result<()> {
        ensure!(
            ![
                "Client",
                "GraphQLError",
                "GraphQLErrors",
                "GraphQLResponse",
                "HTTPError",
                "Optional",
                "ScalarCodec",
                "Subscription",
                "IncrementalStream",
                "GraphQLIncrementalEvent",
                "GraphQLIncrementalSnapshot"
            ]
            .contains(&name),
            "GraphQL type conflicts with runtime {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let mut body = format!("type {name} struct {{\n");
        let mut marshal =
            format!("func (v {name}) MarshalJSON() ([]byte,error) {{ fields:=map[string]any{{}}\n");
        let mut names = BTreeSet::new();
        for field in fields {
            let field_name = ident(&field.name);
            ensure!(
                names.insert(field_name.clone()),
                "GraphQL field naming collision {field_name}"
            );
            let mut ty = field.ty.clone();
            if field.optional {
                ty.nullable = false
            };
            let target = self.ty(&format!("{name}{field_name}"), &ty, input)?;
            let target = if field.optional {
                format!("Optional[{target}]")
            } else {
                target
            };
            let json = serde_json::to_string(&field.name)?;
            writeln!(body, "{field_name} {target} `json:{json}`").unwrap();
            if field.optional {
                writeln!(
                    marshal,
                    "if v.{field_name}.Set {{ fields[{json}]=v.{field_name}.Value }}"
                )
                .unwrap()
            } else {
                writeln!(marshal, "fields[{json}]=v.{field_name}").unwrap()
            }
        }
        body.push_str("}\n");
        self.source.push_str(&body);
        if input {
            marshal.push_str("return json.Marshal(fields)\n}\n");
            self.source.push_str(&marshal);
            body.push_str(&marshal);
        }
        self.files.insert(super::filename("model", name), body);
        Ok(())
    }
    fn union(&mut self, name: &str, alternatives: &[ModelType]) -> Result<()> {
        ensure!(
            self.names.insert(name.into()),
            "GraphQL type naming collision {name}"
        );
        let variants = union_variants(alternatives)?;
        let key = variants[0].0.clone();
        let mut body = format!(
            "type {name}Variant interface {{ is{name}() }}\ntype {name} struct {{ Value {name}Variant }}\nfunc (v *{name}) UnmarshalJSON(data []byte) error {{ var tag map[string]json.RawMessage; if err:=json.Unmarshal(data,&tag); err!=nil {{return err}}; var kind string; if err:=json.Unmarshal(tag[{key:?}],&kind); err!=nil {{return fmt.Errorf(\"missing or invalid GraphQL typename: %w\",err)}}; switch kind {{\n"
        );
        let mut marshal = format!(
            "func (v {name}) MarshalJSON() ([]byte,error) {{ switch value:=v.Value.(type) {{\n"
        );
        for (_, label, fields) in variants {
            let variant = format!("{name}{}", ident(&label));
            self.object(&variant, &fields, false)?;
            writeln!(body,"case {label:?}: var value {variant}; if err:=json.Unmarshal(data,&value); err!=nil {{return err}}; v.Value=&value; return nil").unwrap();
            writeln!(marshal,"case *{variant}: if value == nil || value.{} != {label:?} {{return nil,fmt.Errorf(\"invalid GraphQL typename for {variant}\")}}; return json.Marshal(value)",ident(&key)).unwrap();
            let marker = format!("func (*{variant}) is{name}() {{}}\n");
            self.source.push_str(&marker);
            self.files
                .get_mut(&super::filename("model", &variant))
                .unwrap()
                .push_str(&marker);
        }
        body.push_str("default: return fmt.Errorf(\"unknown GraphQL typename %q\",kind)\n}\n}\n");
        marshal.push_str(
            "default: return nil,fmt.Errorf(\"missing GraphQL union alternative\")\n}\n}\n",
        );
        body.push_str(&marshal);
        self.source.push_str(&body);
        self.files.insert(super::filename("model", name), body);
        Ok(())
    }
    pub(super) fn ty(&mut self, name: &str, ty: &ModelType, input: bool) -> Result<String> {
        let base = match &ty.kind {
            ModelKind::Scalar(s) if self.mappings.contains_key(s) => {
                let mapping = &self.mappings[s];
                if input {
                    mapping.input.clone()
                } else {
                    mapping.output.clone()
                }
            }
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "string".into(),
                "Int" => "int32".into(),
                "Float" => "float64".into(),
                "Boolean" => "bool".into(),
                _ => "json.RawMessage".into(),
            },
            ModelKind::Named(s) => ident(s),
            ModelKind::Enum(_) | ModelKind::Literal(_) => "string".into(),
            ModelKind::List(item) => format!("[]{}", self.ty(&format!("{name}Item"), item, input)?),
            ModelKind::Object(fields) => {
                self.object(name, fields, input)?;
                name.into()
            }
            ModelKind::Union(alternatives) => {
                ensure!(!input, "GraphQL union inputs are unsupported");
                self.union(name, alternatives)?;
                name.into()
            }
        };
        Ok(if ty.nullable {
            format!("*{base}")
        } else {
            base
        })
    }
}

fn union_variants(alternatives: &[ModelType]) -> Result<Vec<(String, String, Vec<ModelField>)>> {
    ensure!(
        !alternatives.is_empty(),
        "GraphQL union has no alternatives"
    );
    let mut values = BTreeSet::new();
    let mut key = None;
    alternatives
        .iter()
        .map(|ty| {
            let ModelKind::Object(fields) = &ty.kind else {
                anyhow::bail!("GraphQL union alternatives must be selected objects")
            };
            let (field, value) = fields
                .iter()
                .find_map(|f| {
                    if let ModelKind::Literal(v) = &f.ty.kind {
                        (!f.optional && !f.ty.nullable).then_some((f, v))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "GraphQL abstract selections require a nonoptional __typename discriminator"
                    )
                })?;
            ensure!(
                key.as_ref().is_none_or(|k| k == &field.name),
                "GraphQL union discriminator aliases must agree"
            );
            key = Some(field.name.clone());
            ensure!(
                values.insert(value.clone()),
                "duplicate GraphQL typename {value}"
            );
            Ok((field.name.clone(), value.clone(), fields.clone()))
        })
        .collect()
}
