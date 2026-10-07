use crate::plan::{ScalarType, ShapePlan};
pub(crate) fn example(shape: &ShapePlan) -> String {
    match shape {
        ShapePlan::Scalar {
            ty: ScalarType::String,
        } => "\"example\"".into(),
        ShapePlan::Scalar {
            ty: ScalarType::Bool,
        } => "true".into(),
        ShapePlan::Scalar { .. } => "1".into(),
        ShapePlan::Object { fields } => format!(
            "{{ {} }}",
            fields
                .iter()
                .filter(|field| field.required)
                .map(|field| format!("{} = {}", field.name, example(&field.shape)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ShapePlan::List { element } => format!("[{}]", example(element)),
        ShapePlan::Map { element } => format!("{{ example = {} }}", example(element)),
    }
}
pub(crate) fn native(shape: &ShapePlan) -> &'static str {
    match shape {
        ShapePlan::Scalar { ty } => match ty {
            ScalarType::String => "String",
            ScalarType::Bool => "Bool",
            ScalarType::Int64 => "Int64",
            ScalarType::Float64 => "Float64",
        },
        ShapePlan::Object { .. } => "Object",
        ShapePlan::List { .. } => "List",
        ShapePlan::Map { .. } => "Map",
    }
}
pub(crate) fn type_expression(shape: &ShapePlan) -> String {
    match shape {
        ShapePlan::Scalar { .. } => format!("types.{}Type", native(shape)),
        ShapePlan::Object { fields } => format!(
            "types.ObjectType{{AttrTypes:map[string]attr.Type{{{}}}}}",
            fields
                .iter()
                .map(|field| format!("{:?}:{},", field.name, type_expression(&field.shape)))
                .collect::<String>()
        ),
        ShapePlan::List { element } => {
            format!("types.ListType{{ElemType:{}}}", type_expression(element))
        }
        ShapePlan::Map { element } => {
            format!("types.MapType{{ElemType:{}}}", type_expression(element))
        }
    }
}
fn fields(shape: &ShapePlan, computed_only: bool) -> String {
    let ShapePlan::Object { fields } = shape else {
        unreachable!()
    };
    format!(
        "map[string]schema.Attribute{{{}}}",
        fields
            .iter()
            .map(|field| format!(
                "{:?}:{},",
                field.name,
                schema(
                    &field.shape,
                    !computed_only && field.required,
                    !computed_only && !field.required,
                    computed_only || !field.required,
                    false,
                    false
                )
            ))
            .collect::<String>()
    )
}
pub(crate) fn schema(
    shape: &ShapePlan,
    required: bool,
    optional: bool,
    computed: bool,
    sensitive: bool,
    replace: bool,
) -> String {
    let computed_only = computed && !optional && !required;
    let base = format!(
        "Required:{required},Optional:{optional},Computed:{computed},Sensitive:{sensitive}"
    );
    let native = native(shape);
    let modifier = if replace {
        format!(
            ",PlanModifiers:[]planmodifier.{native}{{{}planmodifier.RequiresReplace()}}",
            native.to_ascii_lowercase()
        )
    } else {
        String::new()
    };
    match shape {
        ShapePlan::Object { .. } => format!(
            "schema.SingleNestedAttribute{{{base}{modifier},Attributes:{}}}",
            fields(shape, computed_only)
        ),
        ShapePlan::List { element } if matches!(element.as_ref(), ShapePlan::Object { .. }) => {
            format!(
                "schema.ListNestedAttribute{{{base}{modifier},NestedObject:schema.NestedAttributeObject{{Attributes:{}}}}}",
                fields(element, computed_only)
            )
        }
        ShapePlan::Map { element } if matches!(element.as_ref(), ShapePlan::Object { .. }) => {
            format!(
                "schema.MapNestedAttribute{{{base}{modifier},NestedObject:schema.NestedAttributeObject{{Attributes:{}}}}}",
                fields(element, computed_only)
            )
        }
        ShapePlan::List { element } | ShapePlan::Map { element } => format!(
            "schema.{native}Attribute{{{base}{modifier},ElementType:{}}}",
            type_expression(element)
        ),
        ShapePlan::Scalar { .. } => format!("schema.{native}Attribute{{{base}{modifier}}}"),
    }
}
