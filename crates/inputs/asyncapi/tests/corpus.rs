use poolster_input_asyncapi::parse;
#[test]
fn official_asyncapi_corpus() {
    for (name, source, operations, schemas) in [
        (
            "Slack RTM 3.1",
            include_str!("corpus/slack-rtm-3.1.yaml"),
            2,
            1,
        ),
        (
            "Slack RTM 3.0",
            include_str!("corpus/slack-rtm-3.0.yaml"),
            2,
            1,
        ),
        (
            "Slack RTM 2.6",
            include_str!("corpus/slack-rtm-2.6.yaml"),
            2,
            1,
        ),
        (
            "ADEO Kafka",
            include_str!("corpus/adeo-kafka-3.1.yaml"),
            1,
            10,
        ),
        (
            "Kraken Websocket",
            include_str!("corpus/kraken-websocket-3.1.yaml"),
            5,
            20,
        ),
    ] {
        let doc = parse(source).unwrap_or_else(|error| panic!("{name}: {error:#}"));
        let summary = doc.summary();
        println!(
            "{name}: {} bytes, {} operations, {} schemas",
            source.len(),
            summary.operations.len(),
            summary.types.len()
        );
        assert_eq!(summary.operations.len(), operations, "{name}");
        assert_eq!(summary.types.len(), schemas, "{name}");
        assert!(source.len() > 10_000);
    }
}
#[test]
fn official_external_reference_example_is_explicitly_rejected() {
    let error = parse(include_str!("corpus/social-backend-external-3.1.yaml")).unwrap_err();
    assert!(format!("{error:#}").contains("external"));
}
