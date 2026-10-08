use kaji_inputs::{InputPluginInfo, default_registry};

#[test]
fn bundle_registers_exactly_the_enabled_providers() {
    let mut expected: Vec<InputPluginInfo> = vec![
        #[cfg(feature = "graphql")]
        InputPluginInfo {
            provider: "graphql.apollo".into(),
            format: "graphql".into(),
        },
        #[cfg(feature = "asyncapi")]
        InputPluginInfo {
            provider: "asyncapi.roas".into(),
            format: "asyncapi".into(),
        },
        #[cfg(feature = "arazzo")]
        InputPluginInfo {
            provider: "arazzo.roas".into(),
            format: "arazzo".into(),
        },
        #[cfg(feature = "protobuf")]
        InputPluginInfo {
            provider: "protobuf.protox".into(),
            format: "protobuf".into(),
        },
        #[cfg(feature = "capnproto")]
        InputPluginInfo {
            provider: "capnproto.capnp".into(),
            format: "capnproto".into(),
        },
    ];
    expected.sort_by(|a, b| a.provider.cmp(&b.provider));
    assert_eq!(default_registry().unwrap().plugins(), expected);
}

#[test]
fn registries_are_independent_instances() {
    use kaji_inputs::{InputContract, InputPlugin};
    struct Extra;
    impl InputPlugin for Extra {
        fn id(&self) -> &str {
            "community.extra"
        }
        fn format(&self) -> &str {
            "community"
        }
        fn load(&self, _: &std::path::Path) -> anyhow::Result<InputContract> {
            anyhow::bail!("must not load during registration")
        }
    }
    let mut first = default_registry().unwrap();
    let second = default_registry().unwrap();
    let initial = second.plugins();
    first.register(Extra).unwrap();
    assert_eq!(second.plugins(), initial);
    assert_eq!(first.plugins().len(), initial.len() + 1);
}
