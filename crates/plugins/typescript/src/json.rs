//! Schema-directed lossless JSON plans shared by generated transports.
use crate::{Int64Type, ModelOptions};
use kaji_core::{AdditionalProperties, Api, SchemaKind, SchemaValue};
use serde_json::{Value, json};

pub(crate) fn representation(value: &SchemaValue, options: &ModelOptions) -> Int64Type {
    if !matches!(value.kind, SchemaKind::Integer) {
        return Int64Type::Number;
    }
    if options.integer_as_string {
        Int64Type::String
    } else if value.format.as_deref() == Some("int64") {
        options.int64_type
    } else {
        Int64Type::Number
    }
}
fn plan(value: &SchemaValue, options: &ModelOptions) -> Value {
    let mut descriptor = match &value.kind {
        SchemaKind::Integer => match representation(value, options) {
            Int64Type::Number => json!({"kind":"integer"}),
            Int64Type::String => json!({"kind":"integer","integer":"string"}),
            Int64Type::BigInt => json!({"kind":"integer","integer":"bigint"}),
        },
        SchemaKind::Reference { reference } => {
            json!({"ref":reference.rsplit('/').next().unwrap_or(reference)})
        }
        SchemaKind::Array { items } => json!({"kind":"array","items":plan(items, options)}),
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let fields = fields
                .iter()
                .map(|f| (f.name.clone(), plan(&f.value, options)))
                .collect::<serde_json::Map<_, _>>();
            let additional = match additional_properties {
                AdditionalProperties::Schema { value } => plan(value, options),
                _ => Value::Null,
            };
            json!({"kind":"object","fields":fields,"required":value_object_required(value),"additional":additional})
        }
        SchemaKind::AllOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::OneOf { variants } => {
            json!({"kind":if matches!(value.kind,SchemaKind::AllOf {..}) {"allOf"} else {"union"},"variants":variants.iter().map(|v| plan(v,options)).collect::<Vec<_>>()})
        }
        SchemaKind::String => json!({"kind":"string"}),
        SchemaKind::Number => json!({"kind":"number"}),
        SchemaKind::Boolean => json!({"kind":"boolean"}),
        SchemaKind::Null => json!({"kind":"null"}),
        _ => Value::Null,
    };
    if !descriptor.is_null() {
        descriptor["nullable"] = Value::Bool(value.nullable || value.nullish);
        if value.write_only {
            descriptor["writeOnly"] = Value::Bool(true);
        }
        if let Some(literal) = &value.const_value {
            descriptor["literals"] = json!([literal]);
        } else if !value.enum_values.is_empty() {
            descriptor["literals"] = json!(value.enum_values);
        }
    }
    descriptor
}
fn value_object_required(value: &SchemaValue) -> Vec<&str> {
    if let SchemaKind::Object { fields, .. } = &value.kind {
        fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name.as_str())
            .collect()
    } else {
        vec![]
    }
}
pub(crate) fn operation_plan(
    api: &Api,
    operation: &kaji_core::Operation,
    options: &ModelOptions,
) -> Option<Value> {
    let schemas = api
        .schemas
        .iter()
        .map(|schema| (schema.name.as_str(), &schema.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    let requests = operation
        .request_body
        .iter()
        .flat_map(|b| &b.media_types)
        .filter_map(|m| {
            m.schema
                .as_ref()
                .map(|s| (m.content_type.clone(), plan(s, options)))
        })
        .collect::<serde_json::Map<_, _>>();
    let responses = operation
        .responses
        .iter()
        .map(|r| {
            (
                r.status.clone(),
                Value::Object(
                    r.media_types
                        .iter()
                        .filter_map(|m| {
                            m.schema
                                .as_ref()
                                .map(|s| (m.content_type.clone(), plan(s, options)))
                        })
                        .collect(),
                ),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    fn collect(value: &Value, names: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(values) => {
                if let Some(Value::String(name)) = values.get("ref") {
                    names.insert(name.clone());
                }
                for value in values.values() {
                    collect(value, names);
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, names);
                }
            }
            _ => {}
        }
    }
    let mut pending = std::collections::BTreeSet::new();
    collect(&Value::Object(requests.clone()), &mut pending);
    collect(&Value::Object(responses.clone()), &mut pending);
    let mut refs = serde_json::Map::new();
    while let Some(name) = pending.pop_first() {
        if refs.contains_key(&name) {
            continue;
        }
        if let Some(schema) = schemas.get(name.as_str()) {
            let descriptor = plan(schema, options);
            collect(&descriptor, &mut pending);
            refs.insert(name, descriptor);
        }
    }
    Some(
        json!({"lossless":options.integer_as_string || options.int64_type != Int64Type::Number,"refs":refs,"requests":requests,"responses":responses}),
    )
}

pub(crate) const RUNTIME: &str = r#"
export type JsonShape = { kind?: string; nullable?: boolean; writeOnly?: boolean; literals?: unknown[]; required?: string[]; integer?: 'string' | 'bigint'; ref?: string; fields?: Record<string, JsonShape | null>; additional?: JsonShape | null; items?: JsonShape | null; variants?: (JsonShape | null)[] }
export type JsonPlan = { lossless?: boolean; refs: Record<string, JsonShape | null>; requests: Record<string, JsonShape | null>; responses: Record<string, Record<string, JsonShape | null>> }
class JsonNumber { constructor(readonly text: string) {} }
// Parse tokens before converting numbers. JSON.parse validates the complete
// document first; the second pass preserves integer digits exactly.
const rawJson = (text: string): unknown => {
  JSON.parse(text)
  let i = 0
  const whitespace = () => { while (/\s/.test(text[i] ?? '') && i < text.length) i++ }
  const string = (): string => {
    const start = i++
    while (i < text.length) { const c = text[i++]; if (c === '\\') i++; else if (c === '"') break }
    return JSON.parse(text.slice(start, i)) as string
  }
  const value = (): unknown => {
    whitespace()
    if (text[i] === '"') return string()
    if (text[i] === '[') { i++; const result: unknown[] = []; whitespace(); if (text[i] === ']') { i++; return result }; while (true) { result.push(value()); whitespace(); if (text[i++] === ']') return result } }
    if (text[i] === '{') { i++; const result: Record<string, unknown> = {}; whitespace(); if (text[i] === '}') { i++; return result }; while (true) { whitespace(); const key = string(); whitespace(); i++; const item = value(); Object.defineProperty(result, key, { value: item, enumerable: true, configurable: true, writable: true }); whitespace(); if (text[i++] === '}') return result } }
    for (const [literal, result] of [['true', true], ['false', false], ['null', null]] as const) { if (text.startsWith(literal, i)) { i += literal.length; return result } }
    const number = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(text.slice(i))!
    i += number[0].length
    return new JsonNumber(number[0])
  }
  return value()
}
const resolvedShape = (shape: JsonShape | null | undefined, refs: JsonPlan['refs'], seen = new Set<string>()): JsonShape | null => {
  if (!shape) return null
  if (shape.ref) { if (seen.has(shape.ref)) return null; seen.add(shape.ref); const target = resolvedShape(refs[shape.ref], refs, seen); return target ? { ...target, nullable: shape.nullable || target.nullable, writeOnly: shape.writeOnly || target.writeOnly } : null }
  return shape
}
const integerDigits = (token: string): string => {
  const match = /^(-?)(\d+)(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/.exec(token)
  if (!match) throw new TypeError('Invalid integer JSON token')
  const fraction = match[3] ?? ''
  const exponent = Number(match[4] ?? 0)
  if (!Number.isSafeInteger(exponent) || Math.abs(exponent) > 10000) throw new TypeError('Integer JSON exponent exceeds the supported bound')
  let digits = match[2] + fraction
  if (/^0+$/.test(digits)) return '0'
  const shift = exponent - fraction.length
  if (shift >= 0) digits += '0'.repeat(shift)
  else {
    const cut = digits.length + shift
    if (cut < 0 || !/^0*$/.test(digits.slice(Math.max(0,cut)))) throw new TypeError('Fractional value for an integer JSON schema')
    digits = digits.slice(0,cut) || '0'
  }
  digits = digits.replace(/^0+(?=\d)/,'')
  return (match[1] && digits !== '0' ? '-' : '') + digits
}
const matchesJsonShape = (value: unknown, shape: JsonShape | null | undefined, refs: JsonPlan['refs'], depth = 0): boolean => {
  if (depth > 64) return true
  const resolved = resolvedShape(shape, refs)
  if (!resolved) return true
  if (value === null) return !!resolved.nullable || resolved.kind === 'null'
  if (resolved.literals) { const scalar = value instanceof JsonNumber ? value.text : String(value); if (!resolved.literals.some((literal) => String(literal) === scalar)) return false }
  if (resolved.variants) return resolved.variants.some((variant) => matchesJsonShape(value,variant,refs,depth+1))
  if (resolved.kind === 'integer') { if (value instanceof JsonNumber) { try { integerDigits(value.text); return true } catch { return false } }; return typeof value === 'bigint' || (typeof value === 'number' && Number.isInteger(value)) || (typeof value === 'string' && resolved.integer === 'string' && /^-?(?:0|[1-9]\d*)$/.test(value)) }
  if (resolved.kind === 'number') return value instanceof JsonNumber || typeof value === 'number'
  if (resolved.kind === 'string' || resolved.kind === 'boolean') return typeof value === resolved.kind
  if (resolved.kind === 'array') return Array.isArray(value)
  if (resolved.kind === 'object') {
    if (!value || typeof value !== 'object' || Array.isArray(value) || value instanceof JsonNumber) return false
    const object = value as Record<string, unknown>
    return (resolved.required ?? []).every((key) => Object.hasOwn(object,key)) && Object.entries(resolved.fields ?? {}).every(([key,child]) => !Object.hasOwn(object,key) || matchesJsonShape(object[key],child,refs,depth+1))
  }
  return true
}
const selectedShape = (value: unknown, shape: JsonShape | null | undefined, refs: JsonPlan['refs']): JsonShape | null => {
  const resolved = resolvedShape(shape,refs)
  if (!resolved?.variants) return resolved
  // A union selects a matching branch; an intersection combines its fields.
  if (resolved.kind !== 'allOf') return selectedShape(value,resolved.variants.find((variant) => matchesJsonShape(value,variant,refs)),refs)
  const members = resolved.variants.map((variant) => selectedShape(value,variant,refs)).filter((variant): variant is JsonShape => !!variant)
  return { integer: members.find((member) => member.integer)?.integer, fields: Object.assign({},...members.map((member) => member.fields ?? {})), items: members.find((member) => member.items)?.items, additional: members.find((member) => member.additional)?.additional }
}
const fieldJsonShape = (shape: JsonShape | null, key: string): JsonShape | null | undefined => shape?.fields && Object.hasOwn(shape.fields,key) ? shape.fields[key] : shape?.additional
const decodeJsonValue = (value: unknown, shape: JsonShape | null | undefined, refs: JsonPlan['refs']): unknown => {
  const resolved = selectedShape(value, shape, refs)
  if (value instanceof JsonNumber) {
    if (resolved?.integer) { const digits = integerDigits(value.text); return resolved.integer === 'bigint' ? BigInt(digits) : digits }
    return Number(value.text)
  }
  if (typeof value === 'string' && resolved?.integer && /^-?(?:0|[1-9]\d*)$/.test(value)) return resolved.integer === 'bigint' ? BigInt(value) : value
  if (Array.isArray(value)) return value.map((item) => decodeJsonValue(item, resolved?.items, refs))
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, decodeJsonValue(item, fieldJsonShape(resolved,key), refs)]))
  return value
}
/** Structural response checks; enum values and unknown object fields remain forward compatible. */
export class ResponseDecodeError extends TypeError {
  constructor(public readonly path: string, public readonly expected: string) {
    super(`Kaji response decoding failed at ${path}: expected ${expected}`)
    this.name = 'ResponseDecodeError'
  }
}
export const assertResponseShape = (value: unknown, shape: JsonShape | null | undefined, refs: JsonPlan['refs'] = {}, path = '$', depth = 0): void => {
  const schema = resolvedShape(shape, refs)
  if (!schema) return
  if (depth > 128) throw new ResponseDecodeError(path, 'a bounded response structure')
  if (value === null && (schema.nullable || schema.kind === 'null')) return
  if (schema.variants) {
    if (schema.kind === 'allOf') {
      for (const variant of schema.variants) assertResponseShape(value, variant, refs, path, depth + 1)
      return
    }
    for (const variant of schema.variants) {
      try { assertResponseShape(value, variant, refs, path, depth + 1); return }
      catch (error) { if (!(error instanceof ResponseDecodeError)) throw error }
    }
    throw new ResponseDecodeError(path, 'a declared union shape')
  }
  if (value === null) throw new ResponseDecodeError(path, schema.kind ?? 'a non-null value')
  const fail = (expected: string): never => { throw new ResponseDecodeError(path, expected) }
  switch (schema.kind) {
    case 'string': if (typeof value !== 'string') fail('string'); break
    case 'boolean': if (typeof value !== 'boolean') fail('boolean'); break
    case 'number': if (typeof value !== 'number' || !Number.isFinite(value)) fail('finite number'); break
    case 'integer':
      if (schema.integer === 'bigint') { if (typeof value !== 'bigint') fail('bigint') }
      else if (schema.integer === 'string') { if (typeof value !== 'string' || !/^-?(?:0|[1-9]\d*)$/.test(value)) fail('integer string') }
      else if (typeof value !== 'number' || !Number.isInteger(value)) fail('integer')
      break
    case 'null': fail('null'); break
    case 'array':
      if (!Array.isArray(value)) fail('array')
      for (const [index, item] of (value as unknown[]).entries()) assertResponseShape(item, schema.items, refs, `${path}[${index}]`, depth + 1)
      break
    case 'object': {
      if (!value || typeof value !== 'object' || Array.isArray(value)) fail('object')
      const object = value as Record<string, unknown>
      for (const key of schema.required ?? []) {
        if (!resolvedShape(schema.fields?.[key], refs)?.writeOnly && !Object.hasOwn(object, key)) throw new ResponseDecodeError(`${path}[${JSON.stringify(key)}]`, 'required property')
      }
      for (const [key, child] of Object.entries(schema.fields ?? {})) {
        if (Object.hasOwn(object, key)) assertResponseShape(object[key], child, refs, `${path}[${JSON.stringify(key)}]`, depth + 1)
      }
      if (schema.additional) for (const [key, item] of Object.entries(object)) {
        if (!Object.hasOwn(schema.fields ?? {}, key)) assertResponseShape(item, schema.additional, refs, `${path}[${JSON.stringify(key)}]`, depth + 1)
      }
      break
    }
  }
}
const checkResponseEnvelope = (response: unknown, plan: JsonPlan | undefined): unknown => {
  if (!plan) return response
  if (!response || typeof response !== 'object' || !('status' in response)) throw new ResponseDecodeError('$', 'response envelope')
  const envelope = response as { status: number; data?: unknown; contentType?: string }
  if (!Number.isInteger(envelope.status) || envelope.status < 100 || envelope.status > 599) throw new ResponseDecodeError('$.status', 'HTTP status')
  if (envelope.status >= 200 && envelope.status < 300 && envelope.status !== 204) {
    assertResponseShape(envelope.data, responseJsonShape(plan, envelope.status, envelope.contentType ?? ''), plan.refs)
  }
  return response
}
export const parseJson = (text: string, shape?: JsonShape | null, refs: JsonPlan['refs'] = {}): unknown => decodeJsonValue(rawJson(text), shape, refs)
export const stringifyJson = (value: unknown, shape?: JsonShape | null, refs: JsonPlan['refs'] = {}): string => {
  const encode = (item: unknown, current?: JsonShape | null): string | undefined => {
    const resolved = selectedShape(item, current, refs)
    if (resolved?.integer && item !== null && item !== undefined) {
      if (typeof item === 'number' && !Number.isSafeInteger(item)) throw new TypeError('Integer JSON values must be supplied as an exact string or bigint')
      const text = String(item)
      if (!/^-?(?:0|[1-9]\d*)$/.test(text)) throw new TypeError('Invalid integer JSON value')
      return text
    }
    if (typeof item === 'bigint') throw new TypeError('BigInt requires a declared integer schema')
    if (Array.isArray(item)) return '[' + item.map((v) => encode(v, resolved?.items) ?? 'null').join(',') + ']'
    if (item && typeof item === 'object') {
      if (typeof (item as { toJSON?: unknown }).toJSON === 'function') return JSON.stringify(item)
      return '{' + Object.entries(item).flatMap(([key, v]) => { const encoded = encode(v, fieldJsonShape(resolved,key)); return encoded === undefined ? [] : [JSON.stringify(key) + ':' + encoded] }).join(',') + '}'
    }
    return JSON.stringify(item)
  }
  return encode(value, shape) ?? 'null'
}
const eventStreamStatus = (value: unknown): number => value instanceof Response ? value.status : value && typeof value === 'object' && 'status' in value ? Number(value.status) : 200
const requestJsonShape = (plan: JsonPlan | undefined, contentType: string | undefined) => plan?.requests[(contentType ?? 'application/json').split(';')[0].trim()]
const responseJsonShape = (plan: JsonPlan | undefined, status: number, contentType: string) => {
  const statusSchemas = plan?.responses[String(status)] ?? plan?.responses[`${Math.floor(status / 100)}XX`] ?? plan?.responses.default
  const media = contentType.split(';')[0].trim().toLowerCase()
  return Object.entries(statusSchemas ?? {}).find(([key]) => key.toLowerCase() === media)?.[1] ?? statusSchemas?.[`${media.split('/')[0]}/*`] ?? statusSchemas?.['*/*']
}
"#;

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
            .insert("x-kaji-integer".into(), Value::String(name.into()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{
        Field, HttpMethod, Operation, OperationMediaType, OperationRequestBody, OperationResponse,
        Schema, engine::Packages,
    };
    fn api() -> Api {
        let mut id = SchemaValue::new(SchemaKind::Integer);
        id.format = Some("int64".into());
        let fields = [
            ("id", id.clone()),
            ("count", SchemaValue::new(SchemaKind::Integer)),
            ("ratio", SchemaValue::new(SchemaKind::Number)),
            ("label", SchemaValue::new(SchemaKind::String)),
            (
                "ids",
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(id),
                }),
            ),
        ]
        .into_iter()
        .map(|(name, value)| Field {
            name: name.into(),
            required: true,
            value,
            annotations: Default::default(),
        })
        .collect();
        let record = SchemaValue::new(SchemaKind::Object {
            fields,
            additional_properties: AdditionalProperties::Forbidden,
        });
        let reference = SchemaValue::reference("#/components/schemas/Record");
        Api {
            name: "Record API".into(),
            version: "1.0.0".into(),
            schemas: vec![Schema::new("Record", record)],
            operations: vec![
                Operation {
                    id: "echoRecord".into(),
                    method: HttpMethod::Post,
                    path: "/records".into(),
                    request_body: Some(OperationRequestBody {
                        required: true,
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "application/json".into(),
                            schema: Some(reference.clone()),
                        }],
                    }),
                    responses: vec![OperationResponse::json("200", reference.clone())],
                    ..Default::default()
                },
                Operation {
                    id: "streamRecords".into(),
                    method: HttpMethod::Get,
                    path: "/stream".into(),
                    responses: vec![OperationResponse {
                        status: "200".into(),
                        description: None,
                        media_types: vec![OperationMediaType {
                            content_type: "text/event-stream".into(),
                            schema: Some(reference),
                        }],
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
    #[test]
    fn default_number_operations_also_publish_response_shapes() {
        let api = api();
        let plan = operation_plan(&api, &api.operations[0], &ModelOptions::default()).unwrap();
        assert!(plan["responses"]["200"]["application/json"].is_object());
        assert!(plan["refs"].as_object().unwrap().contains_key("Record"));
    }

    #[test]
    fn integer_models_and_wire_plans_agree() {
        for (representation, expected) in [
            (Int64Type::String, "id: string"),
            (Int64Type::BigInt, "id: bigint"),
        ] {
            let tree = Packages::new()
                .package(
                    crate::package("ts").with(
                        crate::sdk()
                            .raw()
                            .group_by_tag(false)
                            .model_options(ModelOptions {
                                int64_type: representation,
                                ..Default::default()
                            }),
                    ),
                )
                .generate(&api(), None)
                .unwrap();
            let source = tree.get("ts/models/Record.ts").unwrap();
            assert!(source.contains(expected), "{source}");
            assert!(source.contains("count: number"));
            assert!(
                tree.get("ts/clients/echoRecord.ts")
                    .unwrap()
                    .contains("jsonPlan:")
            );
            assert!(
                tree.get("ts/clients/streamRecords.ts")
                    .unwrap()
                    .contains("\"text/event-stream\"")
            );
        }
    }
    #[test]
    #[ignore = "requires Node, KAJI_TSC_JS, and KAJI_AXIOS_NODE_MODULES"]
    fn generated_lossless_fetch_axios_and_sse_round_trip() {
        let compiler = std::env::var("KAJI_TSC_JS").unwrap();
        let modules = std::env::var("KAJI_AXIOS_NODE_MODULES").unwrap();
        let directory = std::env::temp_dir().join(format!("kaji-json-{}", std::process::id()));
        let mut packages = Packages::new();
        for (name, representation, axios) in [
            ("big", Int64Type::BigInt, false),
            ("string", Int64Type::String, false),
            ("axios", Int64Type::BigInt, true),
        ] {
            let mut sdk = crate::sdk()
                .raw()
                .group_by_tag(false)
                .model_options(ModelOptions {
                    int64_type: representation,
                    ..Default::default()
                });
            if axios {
                sdk = sdk.axios();
            }
            packages = packages.package(crate::package(name).with(sdk));
        }
        let tree = packages.generate(&api(), None).unwrap();
        tree.write_to(&directory).unwrap();
        for package in ["big", "string", "axios"] {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&modules, directory.join(package).join("node_modules"))
                .unwrap();
            let result = std::process::Command::new("node")
                .arg(&compiler)
                .args(["--project"])
                .arg(directory.join(package).join("tsconfig.json"))
                .args([
                    "--module",
                    "commonjs",
                    "--moduleResolution",
                    "node",
                    "--outDir",
                    "compiled",
                ])
                .current_dir(directory.join(package))
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            std::fs::write(
                directory.join(package).join("compiled/package.json"),
                "{\"type\":\"commonjs\"}",
            )
            .unwrap();
        }
        std::fs::write(directory.join("test.cjs"),r#"
const assert = require('node:assert/strict');
const wire = '{"id":9223372036854775807,"count":7,"ratio":2,"label":"quoted \\" digits 123","ids":[-9223372036854775808,9223372036854775807]}';
const inputWire = JSON.parse(wire);
async function run(name, integer) {
  const { echoRecord } = require(`./${name}/compiled/clients/echoRecord.js`);
  const { createClient, parseJson, stringifyJson } = require(`./${name}/compiled/.kaji/client.js`);
  const body = { id: integer('9223372036854775807'), count: 7, ratio: 2, label: inputWire.label, ids: [integer('-9223372036854775808'), integer('9223372036854775807')] };
  let requestBody;
  const transport = name === 'axios'
    ? createClient({ validateResponses:true, client: { request: async config => { requestBody = config.data; assert.equal(config.responseType,'text'); assert.ok(config.transformResponse); return { status:200, headers:{'content-type':'application/json'}, data:wire }; } } })
    : createClient({ validateResponses:true, fetch: async (_,config) => { requestBody = config.body; return new Response(wire,{headers:{'content-type':'application/json'}}); } });
  const result = await echoRecord({body,client:transport});
  assert.equal(result.id, integer('9223372036854775807'));
  assert.equal(result.ids[0], integer('-9223372036854775808'));
  assert.equal(result.count,7); assert.equal(result.ratio,2); assert.equal(result.label,inputWire.label);
  const malformed = createClient({validateResponses:true,middleware:[async()=>({status:200,contentType:'application/json',data:{...result,count:'wrong'}})]});
  await assert.rejects(()=>echoRecord({body,client:malformed}),error=>error.name==='ResponseDecodeError' && error.path.includes('count'));
  assert.match(requestBody,/"id":9223372036854775807/); assert.match(requestBody,/-9223372036854775808/);
  assert.equal(JSON.parse(requestBody).label,inputWire.label);
  assert.equal(parseJson('{"__proto__":{"polluted":true},"n":12}').n,12);
  assert.equal({}.polluted,undefined);
  assert.equal(parseJson('9.223372036854775807e18',{integer:'bigint'}),9223372036854775807n);
  assert.equal(parseJson('-9223372036854775808.0',{integer:'string'}),'-9223372036854775808');
  assert.throws(()=>parseJson('1.5',{integer:'bigint'}),/Fractional/);
  assert.equal(parseJson('1e3',{kind:'union',variants:[{kind:'string'},{kind:'integer',integer:'bigint'}]}),1000n);
  const additional = parseJson('{"any":1,"other":9223372036854775807}',{kind:'object',fields:{any:null},additional:{kind:'integer',integer:'bigint'}}); assert.equal(additional.any,1); assert.equal(additional.other,9223372036854775807n);
  assert.throws(()=>stringifyJson(9007199254740992,{integer:'bigint'}),/exact string or bigint/);
  {
    const { streamRecords } = require(`./${name}/compiled/clients/streamRecords.js`);
    const streamClient = name === 'axios' ? createClient({client:{request:async()=>({status:200,headers:{'content-type':'text/event-stream'},data:new Response(`data: ${wire}\n\n`).body})}}) : createClient({fetch:async()=>new Response(`data: ${wire}\n\n`,{headers:{'content-type':'text/event-stream'}})});
    const stream = await streamRecords({client:streamClient});
    for await (const record of stream) assert.equal(record.id,integer('9223372036854775807'));
  }
}
(async()=> { await run('big',BigInt); await run('string',String); await run('axios',BigInt); })().catch(error=> {console.error(error);process.exitCode=1;});
"#).unwrap();
        let result = std::process::Command::new("node")
            .arg(directory.join("test.cjs"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
