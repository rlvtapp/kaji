#![cfg(feature = "all-plugins")]
//! Ensure legacy and explicitly selected contract paths emit the same packages.
mod support;
use poolster::{elixir, mock, php, postman, prelude::*, python, ruby, symfony, terraform};
use poolster_core::{
    AdaptedApi, Api, GeneratedTree, Operation, Schema, SecuritySchemeCatalog,
    blocks::{Blocks, ContractReference},
};

struct HttpSource {
    meta: Meta,
    contract: AdaptedApi,
    reference: ContractReference,
}
impl HttpSource {
    fn new(api: Api) -> Self {
        let contract = AdaptedApi::new(api, SecuritySchemeCatalog::default());
        let reference = ContractReference::from_bytes(
            AdaptedApi::NAME,
            "test:http",
            &serde_json::to_vec(&contract).unwrap(),
        );
        Self {
            meta: Meta::new(),
            contract,
            reference,
        }
    }
    fn whole(&self) -> Handle<AdaptedApi> {
        self.meta.handle()
    }
    fn models(&self) -> Handle<Blocks<Schema>> {
        self.meta.handle()
    }
    fn endpoints(&self) -> Handle<Blocks<Operation>> {
        self.meta.handle()
    }
}
impl<L: Language> Plugin<L> for HttpSource {
    fn kind(&self) -> &'static str {
        "test-selected-http"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<AdaptedApi>(),
            Provision::of::<Blocks<Schema>>(),
            Provision::of::<Blocks<Operation>>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> anyhow::Result<()> {
        cx.publish_with_reference(self.contract.clone(), self.reference.clone())?;
        cx.publish(
            self.contract
                .schema_blocks("test:http")
                .with_parent(self.reference.clone()),
        )?;
        cx.publish(
            self.contract
                .endpoint_blocks("test:http")
                .with_parent(self.reference.clone()),
        )
    }
}
fn packages(api: &Api, selected: bool, blocks: bool, reverse: bool) -> ProfileSet {
    let mut profiles = ProfileSet::new("sdk");
    macro_rules! add {
        ($module:ident) => {
            add!($module, $module::sdk())
        };
        ($module:ident, $sdk:expr) => {{
            let current = HttpSource::new(api.clone());
            let original = HttpSource::new(Api {
                name: "Wrong original input".into(),
                ..Api::default()
            });
            let mut sdk = $sdk;
            if selected {
                sdk = sdk.input(current.whole());
            }
            if blocks {
                sdk = sdk
                    .input_models(current.models())
                    .input_endpoints(current.endpoints());
            }
            let package = $module::package(stringify!($module)).with(sdk);
            profiles = profiles.package(if !selected {
                package
            } else if reverse {
                package.with(current).with(original)
            } else {
                package.with(original).with(current)
            });
        }};
    }
    add!(python);
    add!(php);
    add!(ruby);
    add!(elixir);
    add!(symfony);
    add!(postman);
    add!(mock, mock::server());
    add!(
        terraform,
        terraform::sdk().resource(terraform::TerraformResource::new(
            "contact",
            "createContact",
            "getContact",
            "updateContact",
            "deleteContact"
        ))
    );
    profiles
}
fn files(tree: GeneratedTree) -> Vec<(poolster_core::GeneratedFile, bool)> {
    tree.into_files().collect()
}
#[test]
fn selected_and_block_based_outputs_preserve_legacy_files_and_ignore_unselected_input() {
    let mut api = support::sdk_contract_api();
    api.operations[0].path = "/v1/contacts/{id}".into();
    api.operations[0]
        .parameters
        .push(poolster_core::OperationParameter {
            name: "id".into(),
            location: "path".into(),
            required: true,
            schema: Some(poolster_core::SchemaValue::new(
                poolster_core::SchemaKind::String,
            )),
            description: None,
            annotations: Default::default(),
        });
    for (id, method) in [
        ("createContact", poolster_core::HttpMethod::Post),
        ("updateContact", poolster_core::HttpMethod::Patch),
        ("deleteContact", poolster_core::HttpMethod::Delete),
    ] {
        let mut operation = api.operations[0].clone();
        operation.id = id.into();
        operation.method = method;
        if id == "createContact" {
            operation.path = "/v1/contacts".into();
            operation.parameters.clear();
        }
        api.operations.push(operation);
    }
    let baseline = files(poolster::generate(&api, packages(&api, false, false, false)).unwrap());
    for blocks in [false, true] {
        for reverse in [false, true] {
            // The context has no models/endpoints: only the selected typed contract can produce these bytes.
            let actual = files(
                poolster::generate(&Api::default(), packages(&api, true, blocks, reverse)).unwrap(),
            );
            assert_eq!(actual, baseline, "blocks={blocks}, reverse={reverse}");
        }
    }
}
#[test]
fn model_consumers_inherit_their_sdk_input_selection() {
    let api = support::sdk_contract_api();
    let source = HttpSource::new(api.clone());
    let sdk = python::sdk()
        .input(source.whole())
        .input_models(source.models())
        .input_endpoints(source.endpoints());
    let profiles = ProfileSet::new("sdk").package(
        python::package("python")
            .with(python::roundtrips().models_from(&sdk))
            .with(python::operation_tests().models_from(&sdk))
            .with(sdk)
            .with(source),
    );
    let selected = files(poolster::generate(&Api::default(), profiles).unwrap());
    let sdk = python::sdk();
    let profiles = ProfileSet::new("sdk").package(
        python::package("python")
            .with(python::roundtrips().models_from(&sdk))
            .with(python::operation_tests().models_from(&sdk))
            .with(sdk),
    );
    assert_eq!(selected, files(poolster::generate(&api, profiles).unwrap()));
}
