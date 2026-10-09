//! Independently selectable native Go providers and replaceable HTTP execution.
use crate::Go;
use anyhow::Result;
use poolster_core::{
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
/// constructor expression returning the PoolsterHTTPClient interface.
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
    pub(crate) http_api: Option<poolster_core::Api>,
    pub(crate) models: bool,
    pub(crate) operations: bool,
    pub(crate) resources: bool,
    pub(crate) transport: Option<Transport>,
}
impl Workspace {
    pub fn finalize(cx: &mut FinalizeContext<'_, Go>) -> Result<()> {
        let selected_api = cx.workspace.http_api.clone();
        let api = selected_api.as_ref().unwrap_or(cx.api);
        if !cx.workspace.models && !cx.workspace.operations {
            return Ok(());
        }
        let package =
            crate::go_package_name(cx.settings.package_name.as_deref().unwrap_or(&api.name));
        let module =
            crate::go_module_name(cx.settings.package_name.as_deref().unwrap_or(&api.name));
        cx.files
            .emit(GeneratedFile::new("go.mod", crate::render_go_mod(&module))?)?;
        if cx.workspace.operations {
            let style = if cx.workspace.resources {
                SdkClientStyle::Namespaced
            } else {
                SdkClientStyle::Flat
            };
            let prepared = crate::symbols::prepare(api);
            let runtime = crate::render_runtime(&prepared, &package, style).replace(
                "httpClient = http.DefaultClient",
                &format!(
                    "httpClient = {}",
                    cx.workspace.transport.as_ref().unwrap().constructor
                ),
            );
            let runtime = runtime.replace(&crate::response_validation::render(&prepared), "");
            let (_, runtime) = runtime.split_once("\n)\n\n").expect("runtime import block");
            let mut validation = poolster_core::GeneratedTree::default();
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
    http_input: poolster_core::engine::HttpInput,
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
        http_input: Default::default(),
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
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
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
        let mut requirements = self.http_input.requirements();
        requirements.extend({
            match self.part {
                Part::Operations => vec![
                    Requirement::on(self.models),
                    Requirement::on(self.transport),
                ],
                Part::Client => vec![Requirement::on(self.operations)],
                _ => vec![],
            }
        });
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
            if matches!(self.part, Part::Operations) || cx.workspace.http_api.is_none() {
                cx.workspace.http_api = Some(cx.api.clone());
            }
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
        })
    }
}
pub(crate) fn model_contract(api: &poolster_core::Api) -> Models {
    Models {
        symbols: crate::symbols::model_symbols(api),
    }
}
pub(crate) fn operation_contract(api: &poolster_core::Api) -> Operations {
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
    http_input: poolster_core::engine::HttpInput,
    meta: Meta,
    models: Option<Handle<Models>>,
    options: poolster_core::samples::SampleOptions,
}
pub fn roundtrip_tests() -> RoundtripTests {
    RoundtripTests {
        http_input: Default::default(),
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
    pub fn options(mut self, value: poolster_core::samples::SampleOptions) -> Self {
        self.options = value;
        self
    }
}
impl Plugin<Go> for RoundtripTests {
    fn supports_native_input(&self) -> bool {
        self.http_input.is_explicit()
    }
    fn kind(&self) -> &'static str {
        "go-roundtrip-tests"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = self.http_input.requirements();
        requirements.extend(vec![Requirement::on(self.models)]);
        requirements
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        self.http_input.with_context(cx, |cx| {
        let models = cx.inputs.get::<Models>()?;
        let package =
            crate::go_package_name(cx.settings.package_name.as_deref().unwrap_or(&cx.api.name));
        let mut source = format!(
            "// Generated by Poolster. Do not edit.\npackage {package}\nimport (\"bytes\";\"encoding/json\";\"reflect\";\"testing\")\nvar _ = bytes.NewReader\nvar _ = json.Unmarshal\nvar _ = reflect.DeepEqual\nvar _ *testing.T\n"
        );
        let mut diagnostics = BTreeMap::new();
        for (index, schema) in cx.api.schemas.iter().enumerate() {
            let symbol = models
                .symbols
                .get(&schema.name)
                .ok_or_else(|| anyhow::anyhow!("missing model symbol {}", schema.name))?;
            let report =
                poolster_core::samples::schema_samples(cx.api, &schema.value, self.options);
            diagnostics.insert(schema.name.clone(), report.diagnostics);
            for (sample_index, sample) in report.samples.iter().enumerate() {
                let input = serde_json::to_string(&sample.value)?;
                source.push_str(&format!("func TestSchema{index}Sample{sample_index}(t *testing.T) {{\n input:=[]byte({input:?}); var model {symbol}; if err:=json.Unmarshal(input,&model);err!=nil {{t.Fatal(err)}}\n output,err:=json.Marshal(model);if err!=nil {{t.Fatal(err)}}\n var before,after any; decoder:=json.NewDecoder(bytes.NewReader(input));decoder.UseNumber();if err:=decoder.Decode(&before);err!=nil {{t.Fatal(err)}}\n decoder=json.NewDecoder(bytes.NewReader(output));decoder.UseNumber();if err:=decoder.Decode(&after);err!=nil {{t.Fatal(err)}}\n if !reflect.DeepEqual(before,after) {{t.Fatalf(\"roundtrip failed: %s -> %s\",input,output)}}\n}}\n"));
            }
        }
        cx.files
            .emit(GeneratedFile::new("roundtrip_generated_test.go", source)?)?;
        cx.files.emit(GeneratedFile::new(
            ".poolster/roundtrip-diagnostics.json",
            serde_json::to_string_pretty(&diagnostics)? + "\n",
        )?)
     })
    }
}

#[path = "providers_input.rs"]
mod http_input;

#[cfg(test)]
#[path = "providers_tests.rs"]
mod tests;
