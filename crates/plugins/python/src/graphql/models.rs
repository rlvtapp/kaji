use super::*;
pub(super) struct Models {
    pub(super) files: BTreeMap<String, String>,
    pub(super) exports: Vec<(String, Vec<String>)>,
    pub(super) names: BTreeSet<String>,
    pub(super) input_names: BTreeSet<String>,
    pub(super) paths: BTreeSet<String>,
}
impl Models {
    pub(super) fn fields(&mut self, name: &str, fields: &[ModelField]) -> Result<String> {
        ensure!(
            !matches!(
                name,
                "None"
                    | "True"
                    | "False"
                    | "class"
                    | "def"
                    | "return"
                    | "and"
                    | "or"
                    | "not"
                    | "if"
                    | "else"
                    | "elif"
                    | "for"
                    | "while"
                    | "in"
                    | "is"
                    | "import"
                    | "from"
                    | "with"
                    | "as"
                    | "try"
                    | "except"
                    | "finally"
                    | "raise"
                    | "pass"
                    | "break"
                    | "continue"
                    | "lambda"
                    | "yield"
                    | "global"
                    | "nonlocal"
                    | "assert"
                    | "del"
                    | "async"
                    | "await"
                    | "Any"
                    | "TypedDict"
                    | "Optional"
                    | "List"
                    | "Union"
                    | "Literal"
                    | "Client"
                    | "Transport"
                    | "GraphqlResponse"
            ),
            "GraphQL Python type name conflicts with language/runtime name: {name}"
        );
        ensure!(
            name.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "invalid GraphQL Python type name {name}"
        );
        ensure!(
            self.names.insert(name.into()),
            "GraphQL Python type collision: {name}"
        );
        ensure!(
            !name.starts_with("_exports_")
                && !name.starts_with("_aliases_")
                && !matches!(
                    name,
                    "__init__" | "_parts" | "_sys" | "_exports" | "_name" | "_module" | "__all__"
                ),
            "GraphQL Python model conflicts with package internals {name}"
        );
        ensure!(
            self.paths.insert(python_identifier(name)),
            "GraphQL Python model filename collision {name}"
        );
        let file_name = python_identifier(name);
        ensure!(
            !file_name.starts_with("_exports_")
                && !file_name.starts_with("_aliases_")
                && !matches!(file_name.as_str(), "__init__" | "_parts"),
            "Python GraphQL filename conflicts with package internals {file_name}"
        );
        let mut required = Vec::new();
        let mut optional = Vec::new();
        let mut ordered = fields.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|field| &field.name);
        for field in ordered {
            let ty = self.ty(
                &field.ty,
                &format!("{name}{}", crate::python_type_name(&field.name)),
            )?;
            let entry = format!("{}: {ty}", serde_json::to_string(&field.name)?);
            if field.optional {
                optional.push(entry);
            } else {
                required.push(entry);
            }
        }
        let mut parts = vec![];
        for (kind, entries) in [("Required", required), ("Optional", optional)] {
            for (index, fields) in entries.chunks(32).enumerate() {
                let part = format!("_{name}{kind}{index}");
                let file = format!("_parts/{file_name}_{}{index}", kind.to_ascii_lowercase());
                let code = format!(
                    "from typing import Any, TypedDict, Optional, List, Union, Literal\n{part} = TypedDict({part:?}, {{{}}}, total={})\n",
                    fields.join(", "),
                    if kind == "Required" { "True" } else { "False" }
                );
                let refs = fields
                    .iter()
                    .flat_map(|entry| entry.split(|c: char| !c.is_ascii_alphanumeric() && c != '_'))
                    .filter(|name| self.names.contains(*name) || self.input_names.contains(*name))
                    .map(str::to_string)
                    .collect::<BTreeSet<_>>();
                let code = format!(
                    "{code}__poolster_refs__ = {:?}\n",
                    refs.into_iter().collect::<Vec<_>>()
                );
                self.files.insert(format!("models/{file}.py"), code);
                parts.push((file, part));
            }
        }
        let mut depth = 0;
        while parts.len() > 2 {
            let mut next = vec![];
            for (index, children) in parts.chunks(2).enumerate() {
                if children.len() == 1 {
                    next.push(children[0].clone());
                    continue;
                }
                let part = format!("_{name}Branch{depth}Node{index}");
                let file = format!("_parts/{file_name}_branch{depth}_{index}");
                let code = format!(
                    "from .{} import {} as _Left\nfrom .{} import {} as _Right\nimport sys as _sys\nclass {part}(_Left, _Right):\n    pass\n__poolster_refs__ = tuple(dict.fromkeys(ref for parent in [_Left, _Right] for ref in getattr(_sys.modules[parent.__module__], '__poolster_refs__', ())))\n",
                    children[0].0.trim_start_matches("_parts/"),
                    children[0].1,
                    children[1].0.trim_start_matches("_parts/"),
                    children[1].1
                );
                self.files.insert(format!("models/{file}.py"), code);
                next.push((file, part));
            }
            parts = next;
            depth += 1;
        }
        let mut code = String::new();
        for (file, part) in &parts {
            writeln!(code, "from .{} import {part}", file.replace('/', "."))?;
        }
        if parts.is_empty() {
            code.push_str("from typing import TypedDict\n");
            writeln!(code, "class {name}(TypedDict):\n    pass\n")?;
        } else {
            writeln!(
                code,
                "class {name}({}):\n    pass\n",
                parts
                    .iter()
                    .map(|(_, p)| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        if !parts.is_empty() {
            writeln!(
                code,
                "import sys as _sys\n__poolster_refs__ = tuple(dict.fromkeys(ref for part in [{}] for ref in getattr(_sys.modules[part.__module__], '__poolster_refs__', ())))",
                parts
                    .iter()
                    .map(|(_, part)| part.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        let file = file_name;
        self.files.insert(format!("models/{file}.py"), code);
        self.exports.push((file, vec![name.into()]));
        Ok(name.into())
    }
    pub(super) fn ty(&mut self, ty: &ModelType, name: &str) -> Result<String> {
        let mut value = match &ty.kind {
            ModelKind::Scalar(s) => match s.as_str() {
                "String" | "ID" => "str",
                "Int" => "int",
                "Float" => "float",
                "Boolean" => "bool",
                _ => "Any",
            }
            .into(),
            ModelKind::Named(s) => {
                ensure!(
                    self.input_names.contains(s),
                    "unknown GraphQL Python input reference {s}"
                );
                format!("{s:?}")
            }
            ModelKind::Enum(values) => format!(
                "Literal[{}]",
                values
                    .iter()
                    .map(|v| format!("{v:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ModelKind::Literal(v) => format!("Literal[{v:?}]"),
            ModelKind::List(inner) => format!("List[{}]", self.ty(inner, &format!("{name}Item"))?),
            ModelKind::Object(fields) => format!("{:?}", self.fields(name, fields)?),
            ModelKind::Union(members) => format!(
                "Union[{}]",
                members
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.ty(t, &format!("{name}Variant{i}")))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ")
            ),
        };
        if ty.nullable {
            value = format!("Optional[{value}]");
        }
        Ok(value)
    }
}
