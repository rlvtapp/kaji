use poolster_core::{
    AdaptedApi, Api, GeneratedFile, Operation, Schema, SchemaKind, SchemaValue, blocks::*,
    engine::*,
};
struct Lang;
impl Language for Lang {
    const NAME: &'static str = "http-test";
    type Settings = ();
    type Workspace = ();
}
fn whole(name: &str) -> AdaptedApi {
    AdaptedApi::new(
        Api {
            name: name.into(),
            version: "source".into(),
            schemas: vec![Schema::new("Model", SchemaValue::new(SchemaKind::String))],
            operations: vec![Operation {
                id: name.into(),
                path: "/selected".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        Default::default(),
    )
}
fn provenance(api: &AdaptedApi) -> ContractReference {
    ContractReference::from_bytes(
        AdaptedApi::NAME,
        "selected-source",
        &serde_json::to_vec(api).unwrap(),
    )
}
struct Producer {
    meta: Meta,
    api: AdaptedApi,
    block_mode: u8,
    from: Option<Handle<AdaptedApi>>,
}
impl Plugin<Lang> for Producer {
    fn kind(&self) -> &'static str {
        "producer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        self.from
            .map(|handle| vec![Requirement::on(Some(handle))])
            .unwrap_or_default()
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<AdaptedApi>(),
            Provision::of::<Blocks<Schema>>(),
            Provision::of::<Blocks<Operation>>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> anyhow::Result<()> {
        if self.from.is_some() {
            assert_eq!(cx.inputs.get::<AdaptedApi>()?.api.name, "original");
        }
        let parent = provenance(&self.api);
        let mut models = self
            .api
            .schema_blocks("selected-source")
            .with_parent(parent.clone());
        let endpoints = self
            .api
            .endpoint_blocks("selected-source")
            .with_parent(parent.clone());
        models.items[0].value.name = "TransformedModel".into();
        if self.block_mode == 1 {
            models = models.with_parent(ContractReference::from_bytes(
                AdaptedApi::NAME,
                "other-source",
                b"stale",
            ));
        }
        if self.block_mode == 2 {
            models.state = CollectionState::Partial {
                diagnostics: vec!["unsupported shape".into()],
            };
        }
        cx.publish_with_reference(self.api.clone(), parent)?;
        cx.publish(models)?;
        cx.publish(endpoints)
    }
}
struct Output {
    meta: Meta,
    input: HttpInput,
    expected: &'static str,
    blocks: bool,
    policy: bool,
}
impl Plugin<Lang> for Output {
    fn kind(&self) -> &'static str {
        "output"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        self.input.is_explicit()
    }
    fn requires(&self) -> Vec<Requirement> {
        self.input.requirements()
    }
    fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> anyhow::Result<()> {
        self.input.run(cx, |cx| {
            assert_eq!(cx.api.name, self.expected);
            assert_eq!(cx.semantics.operations[0].operation_id, self.expected);
            assert_eq!(cx.api.version, "release");
            if self.policy {
                assert!(poolster_core::idempotency::resolved(&cx.api.operations[0]).is_some());
            }
            if self.blocks {
                assert_eq!(cx.api.schemas[0].name, "TransformedModel");
            }
            cx.files
                .emit(GeneratedFile::new("selected.txt", &cx.api.name)?)
        })
    }
}
#[test]
fn explicit_selected_whole_and_blocks_rebind_api_and_derived_semantics() {
    let original = Producer {
        meta: Meta::new(),
        api: whole("original"),
        block_mode: 0,
        from: None,
    };
    let selected = Producer {
        meta: Meta::new(),
        api: whole("transformed"),
        block_mode: 0,
        from: Some(original.meta.handle()),
    };
    let input = HttpInput::default()
        .input(selected.meta.handle())
        .input_models(selected.meta.handle())
        .input_endpoints(selected.meta.handle());
    let output = Output {
        meta: Meta::new(),
        input,
        expected: "transformed",
        blocks: true,
        policy: false,
    };
    let tree = Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .common(Common::default().package_version("release"))
                .with(output)
                .with(original)
                .with(selected),
        )
        .generate_native()
        .unwrap();
    assert_eq!(tree.get("sdk/selected.txt"), Some("transformed"));
}
#[test]
fn selected_blocks_reject_stale_parents_and_partial_collections() {
    for mode in [1, 2] {
        let provider = Producer {
            meta: Meta::new(),
            api: whole("selected"),
            block_mode: mode,
            from: None,
        };
        let input = HttpInput::default()
            .input(provider.meta.handle())
            .input_models(provider.meta.handle());
        let output = Output {
            meta: Meta::new(),
            input,
            expected: "selected",
            blocks: true,
            policy: false,
        };
        let error = Packages::new()
            .package(Package::<Lang>::new("sdk").with(output).with(provider))
            .generate_native()
            .unwrap_err();
        assert!(format!("{error:#}").contains(if mode == 1 { "parent" } else { "complete" }));
    }
}
#[test]
fn default_legacy_and_opaque_whole_need_no_block_collections() {
    struct Opaque(Meta);
    impl Plugin<Lang> for Opaque {
        fn kind(&self) -> &'static str {
            "opaque-http"
        }
        fn meta(&self) -> &Meta {
            &self.0
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<AdaptedApi>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> anyhow::Result<()> {
            cx.publish(whole("opaque"))
        }
    }
    let provider = Opaque(Meta::new());
    let output = Output {
        meta: Meta::new(),
        input: HttpInput::default().input(provider.0.handle()),
        expected: "opaque",
        blocks: false,
        policy: false,
    };
    Packages::new()
        .common(Common::default().package_version("release"))
        .package(Package::<Lang>::new("sdk").with(output).with(provider))
        .generate_native()
        .unwrap();

    let output = Output {
        meta: Meta::new(),
        input: HttpInput::default(),
        expected: "legacy",
        blocks: false,
        policy: false,
    };
    let api = whole("legacy").api;
    Packages::new()
        .common(Common::default().package_version("release"))
        .package(Package::<Lang>::new("sdk").with(output))
        .generate(&api, None)
        .unwrap();
}
#[test]
fn default_whole_ambiguity_and_explicit_missing_handle_fail_preflight() {
    let output = Output {
        meta: Meta::new(),
        input: HttpInput::default(),
        expected: "unused",
        blocks: false,
        policy: false,
    };
    let error = Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .with(output)
                .with(Producer {
                    meta: Meta::new(),
                    api: whole("one"),
                    block_mode: 0,
                    from: None,
                })
                .with(Producer {
                    meta: Meta::new(),
                    api: whole("two"),
                    block_mode: 0,
                    from: None,
                }),
        )
        .generate(&Api::default(), None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("Select a provider handle explicitly"));
    let input = HttpInput::default().input(Meta::new().handle());
    let output = Output {
        meta: Meta::new(),
        input,
        expected: "unused",
        blocks: false,
        policy: false,
    };
    let error = Packages::new()
        .package(Package::<Lang>::new("sdk").with(output))
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("not registered"));
}

#[test]
fn transformed_operation_policy_is_prepared_after_typed_selection() {
    use poolster_core::idempotency::{IdempotencyConfig, IdempotencyRule};
    let original = Producer {
        meta: Meta::new(),
        api: whole("original"),
        block_mode: 0,
        from: None,
    };
    let selected = Producer {
        meta: Meta::new(),
        api: whole("transformed"),
        block_mode: 0,
        from: Some(original.meta.handle()),
    };
    let output = Output {
        meta: Meta::new(),
        input: HttpInput::default().input(selected.meta.handle()),
        expected: "transformed",
        blocks: false,
        policy: true,
    };
    let policy = IdempotencyConfig {
        operations: [("transformed".into(), IdempotencyRule::default())].into(),
        ..Default::default()
    };
    Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .common(Common::default().package_version("release"))
                .idempotency(policy.clone())
                .with(output)
                .with(original)
                .with(selected),
        )
        .generate(&whole("original").api, None)
        .unwrap();
    // With no typed provider, existing strict unknown-operation validation stays.
    let error = Packages::new()
        .package(Package::<Lang>::new("sdk").idempotency(policy))
        .generate(&whole("original").api, None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("must identify exactly one operation"));
}

#[test]
fn reference_and_release_metadata_can_select_the_transformed_whole() {
    let mut api = whole("transformed");
    api.api.version = "selected-version".into();
    let producer = Producer {
        meta: Meta::new(),
        api,
        block_mode: 0,
        from: None,
    };
    let docs = poolster_core::api_reference::api_reference::<Lang>()
        .input(producer.meta.handle())
        .input_models(producer.meta.handle());
    let metadata = poolster_core::release::metadata::<Lang>(
        poolster_core::release::PackageMetadata::new("test"),
    )
    .input(producer.meta.handle());
    let tree = Packages::new()
        .package(
            Package::<Lang>::new("sdk")
                .with(docs)
                .with(metadata)
                .with(producer),
        )
        .generate_native()
        .unwrap();
    assert!(
        tree.get("sdk/API_REFERENCE.md")
            .unwrap()
            .contains("transformed")
    );
    assert!(
        tree.get("sdk/API_REFERENCE.md")
            .unwrap()
            .contains("TransformedModel")
    );
    let value: serde_json::Value =
        serde_json::from_str(tree.get("sdk/.poolster/package.json").unwrap()).unwrap();
    assert_eq!(value["version"], "selected-version");
}
