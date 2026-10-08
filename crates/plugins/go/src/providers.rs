//! Independently selectable native Go providers and replaceable HTTP execution.
use crate::Go;
use anyhow::Result;
use kaji_core::{
    GeneratedFile, SdkClientStyle,
    engine::{
        Contract, FinalizeContext, Handle, Meta, Plugin, PluginContext, Provision, Requirement,
    },
};
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Models {
    pub symbols: BTreeMap<String, String>,
}
impl Contract for Models {
    const NAME: &'static str = "go.models";
}
/// Custom providers emit their implementation in the package and publish a
/// constructor expression returning the KajiHTTPClient interface.
#[derive(Clone)]
pub struct Transport {
    pub constructor: String,
}
impl Contract for Transport {
    const NAME: &'static str = "go.transport";
}
#[derive(Clone)]
pub struct Operations {
    pub methods: BTreeMap<String, String>,
}
impl Contract for Operations {
    const NAME: &'static str = "go.operations";
}
#[derive(Clone)]
pub struct Client {
    pub symbol: String,
}
impl Contract for Client {
    const NAME: &'static str = "go.client";
}
#[derive(Default)]
pub struct Workspace {
    pub(crate) models: bool,
    pub(crate) operations: bool,
    pub(crate) resources: bool,
    pub(crate) transport: Option<Transport>,
}
impl Workspace {
    pub fn finalize(cx: &mut FinalizeContext<'_, Go>) -> Result<()> {
        if !cx.workspace.models && !cx.workspace.operations {
            return Ok(());
        }
        let package =
            crate::go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        let module =
            crate::go_module_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        cx.files
            .emit(GeneratedFile::new("go.mod", crate::render_go_mod(&module))?)?;
        if cx.workspace.operations {
            let style = if cx.workspace.resources {
                SdkClientStyle::Namespaced
            } else {
                SdkClientStyle::Flat
            };
            let prepared = crate::symbols::prepare(cx.api);
            let runtime = crate::render_runtime(&prepared, &package, style).replace(
                "httpClient = http.DefaultClient",
                &format!(
                    "httpClient = {}",
                    cx.workspace.transport.as_ref().unwrap().constructor
                ),
            );
            let runtime = runtime.replace(&crate::response_validation::render(&prepared), "");
            let (_, runtime) = runtime.split_once("\n)\n\n").expect("runtime import block");
            let mut validation = kaji_core::GeneratedTree::default();
            crate::layout::emit(&mut validation, ".", "client.go", &package, runtime)?;
            for (path, source) in crate::response_validation::split_files(&prepared) {
                crate::layout::emit(&mut validation, ".", &path, &package, &source)?;
            }
            cx.files.append(validation)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Part {
    Models,
    Transport,
    Operations,
    Client,
}
pub struct Provider {
    meta: Meta,
    part: Part,
    models: Option<Handle<Models>>,
    transport: Option<Handle<Transport>>,
    operations: Option<Handle<Operations>>,
    namespaced: bool,
    jobs: usize,
}
fn provider(part: Part) -> Provider {
    Provider {
        meta: Meta::new(),
        part,
        models: None,
        transport: None,
        operations: None,
        namespaced: true,
        jobs: 0,
    }
}
pub fn models() -> Provider {
    provider(Part::Models)
}
pub fn transport() -> Provider {
    provider(Part::Transport)
}
pub fn operations() -> Provider {
    provider(Part::Operations)
}
pub fn client() -> Provider {
    provider(Part::Client)
}
impl Provider {
    pub fn using_models(mut self, value: Handle<Models>) -> Self {
        self.models = Some(value);
        self
    }
    pub fn using_transport(mut self, value: Handle<Transport>) -> Self {
        self.transport = Some(value);
        self
    }
    pub fn using_operations(mut self, value: Handle<Operations>) -> Self {
        self.operations = Some(value);
        self
    }
    pub fn models_handle(&self) -> Handle<Models> {
        self.meta.handle()
    }
    pub fn transport_handle(&self) -> Handle<Transport> {
        self.meta.handle()
    }
    pub fn operations_handle(&self) -> Handle<Operations> {
        self.meta.handle()
    }
    pub fn client_handle(&self) -> Handle<Client> {
        self.meta.handle()
    }
    pub fn flat(mut self) -> Self {
        self.namespaced = false;
        self
    }
    pub fn jobs(mut self, value: usize) -> Self {
        self.jobs = value;
        self
    }
}
impl Plugin<Go> for Provider {
    fn kind(&self) -> &'static str {
        match self.part {
            Part::Models => "go-models",
            Part::Transport => "go-transport",
            Part::Operations => "go-operations",
            Part::Client => "go-client",
        }
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn provides(&self) -> Vec<Provision> {
        vec![match self.part {
            Part::Models => Provision::of::<Models>(),
            Part::Transport => Provision::of::<Transport>(),
            Part::Operations => Provision::of::<Operations>(),
            Part::Client => Provision::of::<Client>(),
        }]
    }
    fn requires(&self) -> Vec<Requirement> {
        match self.part {
            Part::Operations => vec![
                Requirement::on(self.models),
                Requirement::on(self.transport),
            ],
            Part::Client => vec![Requirement::on(self.operations)],
            _ => vec![],
        }
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        match self.part {
            Part::Transport => {
                cx.publish(default_transport())?;
                return Ok(());
            }
            Part::Operations => {
                let _ = cx.inputs.get::<Models>()?;
                cx.workspace.transport = Some(cx.inputs.get::<Transport>()?.clone());
            }
            Part::Client => {
                let _ = cx.inputs.get::<Operations>()?;
            }
            _ => {}
        }
        let style = if matches!(self.part, Part::Client) && self.namespaced {
            SdkClientStyle::Namespaced
        } else {
            SdkClientStyle::Flat
        };
        let tree = crate::render_sdk(
            cx.api,
            ".",
            cx.settings.package_name.as_deref(),
            style,
            self.jobs,
        )?;
        for (file, _) in tree.into_files() {
            let name = file.path.file_name().unwrap().to_string_lossy();
            let emit = match self.part {
                Part::Models => name.starts_with("model_"),
                Part::Operations => name.starts_with("operation_"),
                Part::Client => name.starts_with("service_"),
                Part::Transport => false,
            };
            if emit {
                cx.files.emit(file)?;
            }
        }
        match self.part {
            Part::Models => {
                cx.workspace.models = true;
                cx.publish(model_contract(cx.api))?;
            }
            Part::Operations => {
                cx.workspace.operations = true;
                cx.publish(operation_contract(cx.api))?;
            }
            Part::Client => {
                cx.workspace.resources = self.namespaced;
                cx.publish(Client {
                    symbol: "Client".into(),
                })?;
            }
            _ => {}
        }
        Ok(())
    }
}
pub(crate) fn model_contract(api: &kaji_core::Api) -> Models {
    Models {
        symbols: crate::symbols::model_symbols(api),
    }
}
pub(crate) fn operation_contract(api: &kaji_core::Api) -> Operations {
    Operations {
        methods: crate::symbols::operation_symbols(api),
    }
}
pub(crate) fn default_transport() -> Transport {
    Transport {
        constructor: "http.DefaultClient".into(),
    }
}

pub struct RoundtripTests {
    meta: Meta,
    models: Option<Handle<Models>>,
    options: kaji_core::samples::SampleOptions,
}
pub fn roundtrip_tests() -> RoundtripTests {
    RoundtripTests {
        meta: Meta::new(),
        models: None,
        options: Default::default(),
    }
}
impl RoundtripTests {
    pub fn using_models(mut self, value: Handle<Models>) -> Self {
        self.models = Some(value);
        self
    }
    pub fn options(mut self, value: kaji_core::samples::SampleOptions) -> Self {
        self.options = value;
        self
    }
}
impl Plugin<Go> for RoundtripTests {
    fn kind(&self) -> &'static str {
        "go-roundtrip-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.models)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        let models = cx.inputs.get::<Models>()?;
        let package =
            crate::go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        let mut source = format!(
            "// Generated by Kaji. Do not edit.\npackage {package}\nimport (\"bytes\";\"encoding/json\";\"reflect\";\"testing\")\nvar _ = bytes.NewReader\nvar _ = json.Unmarshal\nvar _ = reflect.DeepEqual\nvar _ *testing.T\n"
        );
        let mut diagnostics = BTreeMap::new();
        for (index, schema) in cx.api.schemas.iter().enumerate() {
            let symbol = models
                .symbols
                .get(&schema.name)
                .ok_or_else(|| anyhow::anyhow!("missing model symbol {}", schema.name))?;
            let report = kaji_core::samples::schema_samples(cx.api, &schema.value, self.options);
            diagnostics.insert(schema.name.clone(), report.diagnostics);
            for (sample_index, sample) in report.samples.iter().enumerate() {
                let input = serde_json::to_string(&sample.value)?;
                source.push_str(&format!("func TestSchema{index}Sample{sample_index}(t *testing.T) {{\n input:=[]byte({input:?}); var model {symbol}; if err:=json.Unmarshal(input,&model);err!=nil {{t.Fatal(err)}}\n output,err:=json.Marshal(model);if err!=nil {{t.Fatal(err)}}\n var before,after any; decoder:=json.NewDecoder(bytes.NewReader(input));decoder.UseNumber();if err:=decoder.Decode(&before);err!=nil {{t.Fatal(err)}}\n decoder=json.NewDecoder(bytes.NewReader(output));decoder.UseNumber();if err:=decoder.Decode(&after);err!=nil {{t.Fatal(err)}}\n if !reflect.DeepEqual(before,after) {{t.Fatalf(\"roundtrip failed: %s -> %s\",input,output)}}\n}}\n"));
            }
        }
        cx.files
            .emit(GeneratedFile::new("roundtrip_generated_test.go", source)?)?;
        cx.files.emit(GeneratedFile::new(
            ".kaji/roundtrip-diagnostics.json",
            serde_json::to_string_pretty(&diagnostics)? + "\n",
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{
        Api, HttpMethod, Operation, OperationResponse, Schema, SchemaKind, SchemaValue,
        engine::Packages,
    };
    struct CustomTransport {
        meta: Meta,
    }
    impl Plugin<Go> for CustomTransport {
        fn kind(&self) -> &'static str {
            "custom-http"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Transport>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
            cx.files.emit(GeneratedFile::new("custom_http.go",r#"package demo
import("io";"net/http";"strings")
type customHTTP struct{}
func(customHTTP) Do(request *http.Request)(*http.Response,error){return &http.Response{StatusCode:200,Header:http.Header{"Content-Type":[]string{"application/json"}},Body:io.NopCloser(strings.NewReader(`"custom"`)),Request:request},nil}
"#)?)?;
            cx.publish(Transport {
                constructor: "customHTTP{}".into(),
            })
        }
    }
    #[test]
    fn custom_provider_and_generated_roundtrips_execute_without_network() {
        let mut integer = SchemaValue::new(SchemaKind::Integer);
        integer.format = Some("int64".into());
        let api = Api {
            name: "demo".into(),
            version: "1.0.0".into(),
            schemas: vec![
                Schema::new(
                    "Details",
                    SchemaValue::new(SchemaKind::Object {
                        fields: vec![kaji_core::Field {
                            name: "note".into(),
                            value: {
                                let mut value = SchemaValue::new(SchemaKind::String);
                                value.nullable = true;
                                value
                            },
                            required: false,
                            annotations: Default::default(),
                        }],
                        additional_properties: kaji_core::AdditionalProperties::Any,
                    }),
                ),
                Schema::new("Counter", integer),
                Schema::new("Label", SchemaValue::new(SchemaKind::String)),
            ],
            operations: vec![Operation {
                id: "getLabel".into(),
                method: HttpMethod::Get,
                path: "/label".into(),
                responses: vec![OperationResponse::json(
                    "200",
                    SchemaValue::new(SchemaKind::String),
                )],
                ..Default::default()
            }],
            ..Default::default()
        };
        let model = models();
        let model_handle = model.models_handle();
        let custom = CustomTransport { meta: Meta::new() };
        let transport_handle = custom.meta.handle::<Transport>();
        let operation = operations()
            .using_models(model_handle)
            .using_transport(transport_handle);
        let operation_handle = operation.operations_handle();
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .with(client().using_operations(operation_handle))
                    .with(operation)
                    .with(custom)
                    .with(model)
                    .with(roundtrip_tests().using_models(model_handle)),
            )
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        std::fs::write(root.path().join("sdk/custom_test.go"),r#"package demo
import("context";"testing")
func TestCustomTransport(t *testing.T){client,err:=NewClient(ClientConfig{BaseURL:"https://unused.example"});if err!=nil{t.Fatal(err)};result,err:=client.GetLabel(context.Background());if err!=nil{t.Fatal(err)};if result==nil || *result!="custom"{t.Fatal(result)}}
"#).unwrap();
        let output = std::process::Command::new("go")
            .args(["test", "./..."])
            .env("GOCACHE", root.path().join("go-cache"))
            .current_dir(root.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
