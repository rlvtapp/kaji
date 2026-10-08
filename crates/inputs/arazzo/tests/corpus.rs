use kaji_input_arazzo::parse;
#[test]
fn official_arazzo_workflow_corpus() {
    for (name, source, workflows, expected_steps) in [
        (
            "Buy now pay later",
            include_str!("corpus/bnpl-1.0.yaml"),
            1,
            7,
        ),
        ("FAPI PAR", include_str!("corpus/fapi-par-1.0.yaml"), 1, 3),
        ("OAuth", include_str!("corpus/oauth-1.0.yaml"), 3, 5),
        (
            "Pet coupons",
            include_str!("corpus/pet-coupons-1.0.yaml"),
            3,
            6,
        ),
    ] {
        let doc = parse(source).unwrap_or_else(|error| panic!("{name}: {error:#}"));
        let steps: usize = doc.source["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|workflow| workflow["steps"].as_array().unwrap().len())
            .sum();
        println!(
            "{name}: {} bytes, {} workflows, {steps} steps, {} unloaded sources",
            source.len(),
            doc.summary().operations.len(),
            doc.unresolved_sources.len()
        );
        assert_eq!(doc.summary().operations.len(), workflows, "{name}");
        assert_eq!(steps, expected_steps, "{name}");
        assert_eq!(doc.unresolved_sources.len(), 1, "{name}");
        assert!(source.len() > 5_000);
    }
}
