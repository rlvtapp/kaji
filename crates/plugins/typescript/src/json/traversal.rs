use super::*;

pub(crate) fn visit_api(api: &mut Api, visitor: &mut impl FnMut(&mut SchemaValue)) {
    fn visit(value: &mut SchemaValue, visitor: &mut impl FnMut(&mut SchemaValue)) {
        visitor(value);
        match &mut value.kind {
            SchemaKind::Array { items } => visit(items, visitor),
            SchemaKind::Object {
                fields,
                additional_properties,
            } => {
                for field in fields {
                    visit(&mut field.value, visitor);
                }
                if let AdditionalProperties::Schema { value } = additional_properties {
                    visit(value, visitor);
                }
            }
            SchemaKind::AnyOf { variants }
            | SchemaKind::AllOf { variants }
            | SchemaKind::OneOf { variants } => {
                for variant in variants {
                    visit(variant, visitor);
                }
            }
            SchemaKind::Not { schema } => visit(schema, visitor),
            _ => {}
        }
    }
    for schema in &mut api.schemas {
        visit(&mut schema.value, visitor);
    }
    for operation in &mut api.operations {
        for parameter in &mut operation.parameters {
            if let Some(value) = &mut parameter.schema {
                visit(value, visitor);
            }
        }
        if let Some(body) = &mut operation.request_body {
            for media in &mut body.media_types {
                if let Some(value) = &mut media.schema {
                    visit(value, visitor);
                }
            }
        }
        for response in &mut operation.responses {
            for media in &mut response.media_types {
                if let Some(value) = &mut media.schema {
                    visit(value, visitor);
                }
            }
        }
    }
}
pub(crate) fn artifact_api(api: &Api, options: &ModelOptions) -> Api {
    let mut api = api.clone();
    visit_api(&mut api, &mut |value| {
        let name = match representation(value, options) {
            Int64Type::Number => return,
            Int64Type::String => "string",
            Int64Type::BigInt => "bigint",
        };
        value
            .extensions
            .insert("x-poolster-integer".into(), Value::String(name.into()));
    });
    if options.remove_optional_properties {
        visit_api(&mut api, &mut |value| {
            if let SchemaKind::Object { fields, .. } = &mut value.kind {
                fields.retain(|f| f.required);
            }
        });
    }
    api
}
