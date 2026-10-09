//! Optional native operation wire smoke tests driven by bounded core samples.
use super::*;
use poolster_core::engine::{Handle, Meta, Plugin, PluginContext, Requirement};
use providers::{Client, Operations};
use serde_json::{Value, json};
pub struct OperationTests {
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    client: Option<Handle<Client>>,
    operations: Option<Handle<Operations>>,
    options: poolster_core::samples::SampleOptions,
    max_operations: usize,
}
pub fn operation_tests() -> OperationTests {
    OperationTests {
        http_input: Default::default(),
        meta: Meta::new(),
        client: None,
        operations: None,
        options: Default::default(),
        max_operations: 128,
    }
}
impl OperationTests {
    pub fn using_client(mut self, handle: Handle<Client>) -> Self {
        self.client = Some(handle);
        self
    }
    pub fn using_operations(mut self, handle: Handle<Operations>) -> Self {
        self.operations = Some(handle);
        self
    }
    pub fn sample_options(mut self, options: poolster_core::samples::SampleOptions) -> Self {
        self.options = options;
        self
    }
    pub fn max_operations(mut self, limit: usize) -> Self {
        self.max_operations = limit;
        self
    }
}
fn sample(
    api: &Api,
    schema: &SchemaValue,
    options: poolster_core::samples::SampleOptions,
) -> std::result::Result<Value, String> {
    let report = poolster_core::samples::schema_samples(api, schema, options);
    report
        .samples
        .into_iter()
        .next()
        .map(|sample| sample.value)
        .ok_or_else(|| {
            if report.diagnostics.is_empty() {
                "no bounded sample available".into()
            } else {
                report.diagnostics.join("; ")
            }
        })
}
fn scalar_parameter(api: &Api, value: &SchemaValue) -> bool {
    let mut value = value;
    let mut seen = BTreeSet::new();
    while let Some(name) = value.kind.reference_name() {
        if !seen.insert(name) {
            return false;
        }
        let Some(schema) = api.schemas.iter().find(|schema| schema.name == name) else {
            return false;
        };
        value = &schema.value;
    }
    !matches!(value.format.as_deref(), Some("byte" | "binary"))
        && matches!(
            value.kind,
            SchemaKind::String | SchemaKind::Boolean | SchemaKind::Integer | SchemaKind::Number
        )
}
fn string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn operation_test(
    api: &Api,
    operation: &Operation,
    method: &str,
    index: usize,
    options: poolster_core::samples::SampleOptions,
) -> std::result::Result<String, String> {
    let name = go_type_name(&operation.id);
    if method != name {
        return Err("custom operation symbol requires a test adapter".into());
    }
    let mut input = serde_json::Map::new();
    let mut path = quote(&operation.path);
    let mut checks = String::new();
    for parameter in &operation.parameters {
        if !parameter
            .schema
            .as_ref()
            .is_some_and(|schema| scalar_parameter(api, schema))
        {
            return Err("parameter kind needs a native smoke adapter".into());
        }
        let value = sample(
            api,
            parameter
                .schema
                .as_ref()
                .ok_or("parameter schema is missing")?,
            options,
        )?;
        let encoded =
            string(&value).ok_or("parameter smoke tests currently require scalar samples")?;
        let key = quote(&parameter.name);
        let expected = quote(&encoded);
        match parameter.location.as_str() {
            "path" => {
                if value.is_f64() {
                    return Err("floating path parameter needs a native smoke adapter".into());
                }
                path = format!(
                    "strings.ReplaceAll({path},{},url.PathEscape({expected}))",
                    quote(&format!("{{{}}}", parameter.name))
                )
            }
            "query" => {
                let _ = writeln!(
                    checks,
                    "if !contractScalarEqual(request.URL.Query().Get({key}),{expected},{}) {{t.Fatal(\"query parameter mismatch\")}}",
                    value.is_number()
                );
            }
            "header" => {
                let _ = writeln!(
                    checks,
                    "if !contractScalarEqual(request.Header.Get({key}),{expected},{}) {{t.Fatal(\"header parameter mismatch\")}}",
                    value.is_number()
                );
            }
            _ => return Err("parameter location needs a native smoke adapter".into()),
        }
        input.insert(parameter.name.clone(), value);
    }
    if let Some(body) = &operation.request_body {
        if input.contains_key("body") {
            return Err("body input name collision needs a smoke adapter".into());
        }
        let media = body.media_types.first().ok_or("body media type missing")?;
        if !media.content_type.contains("json") {
            return Err("non-JSON request body needs a native smoke adapter".into());
        }
        let body_sample = sample(
            api,
            media.schema.as_ref().ok_or("body schema missing")?,
            options,
        )?;
        if body_sample.is_null() {
            return Err("null request body needs a native smoke adapter".into());
        }
        let expected = quote(&serde_json::to_string(&body_sample).unwrap());
        let _ = writeln!(
            checks,
            "if request.Body==nil{{t.Fatal(\"missing JSON body\")}};defer request.Body.Close();body,err:=io.ReadAll(request.Body);if err!=nil{{t.Fatal(err)}};contractJSONEqual(t,body,[]byte({expected}))"
        );
        input.insert("body".into(), body_sample);
    }
    let response = operation
        .responses
        .iter()
        .find(|response| response.status.starts_with('2'))
        .ok_or("no explicit success response")?;
    let status = response
        .status
        .parse::<u16>()
        .map_err(|_| "success wildcard needs a smoke adapter")?;
    let (response_body, response_check, call) = match operation_response_kind(operation) {
        GoResponseKind::Json(_) => {
            if status == 204 {
                return Err("JSON 204 response needs a smoke adapter".into());
            }
            let media = response
                .media_types
                .first()
                .ok_or("response media missing")?;
            let value = sample(
                api,
                media
                    .schema
                    .as_ref()
                    .ok_or("JSON response schema missing")?,
                options,
            )?;
            if value.is_null() {
                return Err("root null response needs a smoke adapter".into());
            }
            let encoded = quote(&serde_json::to_string(&value).unwrap());
            (
                encoded.clone(),
                format!(
                    "if result==nil{{t.Fatal(\"missing result\")}};output,err:=json.Marshal(result);if err!=nil{{t.Fatal(err)}};contractJSONEqual(t,output,[]byte({encoded}))"
                ),
                "result,err:=".to_string(),
            )
        }
        GoResponseKind::None => (quote(""), String::new(), "err=".to_string()),
        _ => return Err("text/binary/SSE response needs a native smoke adapter".into()),
    };
    let has_input = !operation.parameters.is_empty() || operation.request_body.is_some();
    let request = super::operation_request_name(api, operation);
    let input_setup = if has_input {
        format!(
            "var input {request};if err:=json.Unmarshal([]byte({}),&input);err!=nil{{t.Fatal(err)}}",
            quote(&serde_json::to_string(&Value::Object(input)).unwrap())
        )
    } else {
        String::new()
    };
    let input_argument = if has_input { ",&input" } else { "" };
    let verb = quote(operation.method.as_str());
    Ok(format!(
        r#"
func TestOperationWire{index}(t *testing.T) {{
    {input_setup}
    calls:=0
    native:=PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){{
        calls++
        if request.Method!={verb}||request.URL.EscapedPath()!={path}{{t.Fatal("operation method/path mismatch")}}
        {checks}
        return &http.Response{{StatusCode:{status},Header:http.Header{{"Content-Type":[]string{{"application/json"}}}},Body:io.NopCloser(strings.NewReader({response_body}))}},nil
    }})
    client,err:=NewClient(ClientConfig{{BaseURL:"https://unused.example",HTTPClient:native,Retry:&RetryConfig{{MaxAttempts:1}}}});if err!=nil{{t.Fatal(err)}}
    {call}client.{method}(context.Background(){input_argument});if err!=nil{{t.Fatal(err)}}
    if calls!=1{{t.Fatal("expected exactly one native request")}}
    {response_check}
}}
"#
    ))
}
impl Plugin<Go> for OperationTests {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "go-operation-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = self.http_input.requirements();
        requirements.extend(vec![
            Requirement::on(self.client),
            Requirement::on(self.operations),
        ]);
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
        let client = cx.inputs.get::<Client>()?;
        let operations = cx.inputs.get::<Operations>()?;
        let package = go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        let mut source = format!(
            "// Generated by Poolster. Do not edit.\npackage {package}\nimport(\"bytes\";\"context\";\"encoding/json\";\"io\";\"net/http\";\"net/url\";\"math/big\";\"reflect\";\"strings\";\"testing\")\nvar _=context.Background\nvar _=io.ReadAll\nvar _=http.MethodGet\nvar _=url.PathEscape\nvar _=strings.NewReader\n"
        );
        source.push_str("func contractJSONEqual(t *testing.T,left,right []byte){t.Helper();var first,second any;a:=json.NewDecoder(bytes.NewReader(left));a.UseNumber();if err:=a.Decode(&first);err!=nil{t.Fatal(err)};b:=json.NewDecoder(bytes.NewReader(right));b.UseNumber();if err:=b.Decode(&second);err!=nil{t.Fatal(err)};if !contractJSONValueEqual(first,second){t.Fatal(\"wire JSON differs from bounded sample\")}}\n");
        source.push_str(r#"
func contractScalarEqual(actual,expected string,numeric bool)bool{if numeric{return contractJSONValueEqual(json.Number(actual),json.Number(expected))};return actual==expected}
func contractJSONValueEqual(left,right any)bool{
    switch value:=left.(type){
    case json.Number:other,ok:=right.(json.Number);if !ok{return false};a,ok:=new(big.Rat).SetString(string(value));if !ok{return false};b,ok:=new(big.Rat).SetString(string(other));return ok&&a.Cmp(b)==0
    case map[string]any:other,ok:=right.(map[string]any);if !ok||len(value)!=len(other){return false};for key,item:=range value{next,ok:=other[key];if !ok||!contractJSONValueEqual(item,next){return false}};return true
    case []any:other,ok:=right.([]any);if !ok||len(value)!=len(other){return false};for index,item:=range value{if !contractJSONValueEqual(item,other[index]){return false}};return true
    default:return reflect.DeepEqual(left,right)
    }
}
"#);
        let prepared = super::symbols::prepare(cx.api);
        let mut skipped = BTreeMap::new();
        let mut generated = 0;
        for (index, operation) in cx.api.operations.iter().enumerate() {
            let rendered = if index >= self.max_operations {
                Err("operation count bound reached".into())
            } else if client.symbol != "Client" {
                Err("custom client requires a native smoke adapter".into())
            } else {
                operations
                    .methods
                    .get(&operation.id)
                    .ok_or_else(|| "operation symbol unavailable".into())
                    .and_then(|method| {
                        operation_test(
                            &prepared,
                            &prepared.operations[index],
                            method,
                            index,
                            self.options,
                        )
                    })
            };
            match rendered {
                Ok(test) => {
                    source.push_str(&test);
                    generated += 1
                }
                Err(reason) => {
                    source.push_str(&format!(
                        "func TestOperationWire{index}(t *testing.T){{t.Skip({})}}\n",
                        quote(&reason)
                    ));
                    skipped.insert(operation.id.clone(), reason);
                }
            }
        }
        cx.files
            .emit(GeneratedFile::new("operation_generated_test.go", source)?)?;
        cx.files.emit(GeneratedFile::new("OPERATION_TESTS.md", format!("# Native operation wire tests\n\nRun `go test ./...` to execute `operation_generated_test.go`. This package contains {generated} bounded sample operation tests and {} explicitly skipped operations. Inspect [.poolster/operation-test-diagnostics.json](.poolster/operation-test-diagnostics.json) for exclusions; use `go test -v ./...` to see native skip reasons.\n\nThese tests construct the SDK's public Client and call actual operation methods through a fake PoolsterHTTPClient; no HTTP server or remote API is contacted. They check method/path escaping, supported scalar query/header inputs, JSON body encoding, explicit successful HTTP statuses, and decoded JSON responses. JSON comparisons preserve exact numeric values while accepting equivalent number spellings. Each smoke operation makes one transport attempt; retry, middleware, auth, streaming and cancellation are covered separately.\n\nThe consumer supports native Client/method ABIs, string/bool/numeric scalar parameters, JSON bodies and JSON/void success responses. Body/media kinds needing other adapters, nullable roots, unsupported samples, custom operation/client symbols, floating path parameters, and operations exceeding configured bounds are recorded as skips. This is a serialization/wire smoke suite, not full schema validation or service acceptance coverage. Tests use the first bounded structural sample, which may omit optional fields.\n", skipped.len()))?)?;
        cx.files.emit(GeneratedFile::new(
            ".poolster/operation-test-diagnostics.json",
            serde_json::to_string_pretty(&json!({"generated":generated,"skipped":skipped}))? + "\n",
        )?)
     })
    }
}

#[path = "operation_tests_input.rs"]
mod http_input;

#[cfg(test)]
#[path = "operation_tests_tests.rs"]
mod tests;
