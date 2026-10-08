//! Constraint predicates shared by schema-derived validators.
use poolster_core::{SchemaKind, SchemaValue};
use serde_json::Value;

fn integer_ratio(value: &Value) -> Option<(String, String)> {
    let text = value.to_string();
    let (mantissa, exponent) = text
        .split_once(['e', 'E'])
        .map_or((text.as_str(), 0), |(m, e)| {
            (m, e.parse::<i32>().ok().unwrap_or(1000))
        });
    if exponent.abs() > 512 {
        return None;
    }
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut numerator = format!("{whole}{fraction}");
    let scale = fraction.len() as i32 - exponent;
    let denominator = if scale > 0 {
        format!("1{}", "0".repeat(scale as usize))
    } else {
        numerator.push_str(&"0".repeat((-scale) as usize));
        "1".into()
    };
    Some((numerator, denominator))
}
fn numeric(schema: &SchemaValue, bound: &Value, operator: &str) -> Option<String> {
    if matches!(schema.kind, SchemaKind::Integer)
        && matches!(
            schema
                .extensions
                .get("x-kaji-integer")
                .and_then(Value::as_str),
            Some("bigint" | "string")
        )
    {
        let (n, d) = integer_ratio(bound)?;
        let variable = if schema
            .extensions
            .get("x-kaji-integer")
            .and_then(Value::as_str)
            == Some("string")
        {
            "BigInt(value)"
        } else {
            "value"
        };
        Some(format!(
            "({variable} * BigInt({})) {operator} BigInt({})",
            serde_json::to_string(&d).ok()?,
            serde_json::to_string(&n).ok()?
        ))
    } else {
        Some(format!("value {operator} {bound}"))
    }
}
pub(crate) fn constrain(mut source: String, schema: &SchemaValue) -> String {
    for (name, value) in &schema.constraints {
        let test = match name.as_str() {
            "minimum" => numeric(schema, value, ">="),
            "maximum" => numeric(schema, value, "<="),
            "exclusiveMinimum" => numeric(schema, value, ">"),
            "exclusiveMaximum" => numeric(schema, value, "<"),
            "multipleOf" => {
                if value.as_f64().is_some_and(|value| value <= 0.0) {
                    Some("false".into())
                } else if matches!(schema.kind, SchemaKind::Integer)
                    && matches!(
                        schema
                            .extensions
                            .get("x-kaji-integer")
                            .and_then(Value::as_str),
                        Some("bigint" | "string")
                    )
                {
                    integer_ratio(value).map(|(n, d)| {
                        let variable = if schema
                            .extensions
                            .get("x-kaji-integer")
                            .and_then(Value::as_str)
                            == Some("string")
                        {
                            "BigInt(value)"
                        } else {
                            "value"
                        };
                        format!("({variable} * BigInt({d:?})) % BigInt({n:?}) === 0n")
                    })
                } else {
                    Some(format!(
                        "Math.abs(value / {value} - Math.round(value / {value})) <= 1e-9"
                    ))
                }
            }
            "minLength" => Some(format!("Array.from(value).length >= {value}")),
            "maxLength" => Some(format!("Array.from(value).length <= {value}")),
            "pattern" => value.as_str().map(|pattern| {
                format!(
                    "new RegExp({}).test(value)",
                    serde_json::to_string(pattern).unwrap()
                )
            }),
            "minItems" => Some(format!("value.length >= {value}")),
            "maxItems" => Some(format!("value.length <= {value}")),
            "uniqueItems" if value == &Value::Bool(true) => Some(
                "new Set(value.map((item: unknown) => canonical(item))).size === value.length"
                    .into(),
            ),
            "minProperties" => Some(format!("Object.keys(value).length >= {value}")),
            "maxProperties" => Some(format!("Object.keys(value).length <= {value}")),
            _ => None,
        };
        if let Some(mut test) = test {
            // JSON Schema constraints apply only to values of the relevant kind.
            if matches!(
                name.as_str(),
                "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf"
            ) {
                let guard = if matches!(
                    schema
                        .extensions
                        .get("x-kaji-integer")
                        .and_then(Value::as_str),
                    Some("string")
                ) {
                    "typeof value !== 'string' || !/^-?\\d+$/.test(value)"
                } else if matches!(
                    schema
                        .extensions
                        .get("x-kaji-integer")
                        .and_then(Value::as_str),
                    Some("bigint")
                ) {
                    "typeof value !== 'bigint'"
                } else {
                    "typeof value !== 'number'"
                };
                test = format!("({guard}) || ({test})");
            } else if matches!(name.as_str(), "minLength" | "maxLength" | "pattern") {
                test = format!("typeof value !== 'string' || ({test})");
            } else if matches!(name.as_str(), "minItems" | "maxItems" | "uniqueItems") {
                test = format!("!Array.isArray(value) || ({test})");
            } else {
                test = format!(
                    "value === null || typeof value !== 'object' || Array.isArray(value) || ({test})"
                );
            }
            let prefix = if name == "uniqueItems" {
                "const canonical = (value: unknown): string => typeof value === 'bigint' ? `integer:${value}` : Array.isArray(value) ? '[' + value.map(canonical).join(',') + ']' : value && typeof value === 'object' ? '{' + Object.keys(value).sort().map(key => JSON.stringify(key) + ':' + canonical((value as Record<string, unknown>)[key])).join(',') + '}' : JSON.stringify(value) ?? 'undefined'; "
            } else {
                ""
            };
            source = format!(
                "{source}.refine((value) => {{ {prefix}return {test}; }}, {{ message: {} }})",
                serde_json::to_string(&format!("Schema constraint {name}")).unwrap()
            );
        }
    }
    if matches!(schema.kind, SchemaKind::String) {
        let check = match schema.format.as_deref() {
            Some("uuid") => Some(
                "/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)",
            ),
            Some("email") => Some("/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(value)"),
            Some("date") => {
                Some("/^\\d{4}-\\d{2}-\\d{2}$/.test(value) && !Number.isNaN(Date.parse(value))")
            }
            Some("date-time") => Some(
                "/^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}(?:\\.\\d+)?(?:Z|[+-]\\d{2}:\\d{2})$/.test(value) && !Number.isNaN(Date.parse(value))",
            ),
            Some("uri" | "url") => {
                Some("(() => { try { new URL(value); return true; } catch { return false; } })()")
            }
            _ => None,
        };
        if let Some(check) = check {
            source = format!("{source}.refine((value) => {check}, {{ message: 'Schema format' }})");
        }
    }
    source
}
