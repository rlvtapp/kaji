//! Selection-aware runtime codec descriptors shared by native GraphQL outputs.
use super::{ModelField, ModelKind, ModelType};
use serde_json::{Value, json};
/// Codec traversal preserves wire field names and abstract discriminants.
/// Nullability/presence remain in ModelType; codecs never receive missing/null values.
pub fn graphql_scalar_shape(ty: &ModelType) -> Value {
    match &ty.kind {
        ModelKind::Scalar(name) => json!({"scalar":name}),
        ModelKind::Named(name) => json!({"named":name}),
        ModelKind::List(item) => json!({"list":graphql_scalar_shape(item)}),
        ModelKind::Object(fields) => graphql_scalar_fields(fields),
        ModelKind::Union(types) => {
            json!({"union":types.iter().map(graphql_scalar_shape).collect::<Vec<_>>()})
        }
        ModelKind::Literal(value) => json!({"literal":value}),
        ModelKind::Enum(_) => Value::Null,
    }
}
pub fn graphql_scalar_fields(fields: &[ModelField]) -> Value {
    let mut fields = fields.iter().collect::<Vec<_>>();
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    json!({"fields":fields.iter().map(|f|json!([f.name,graphql_scalar_shape(&f.ty)])).collect::<Vec<_>>()})
}
