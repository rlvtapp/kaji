const EXTENSIONS: &[&str] = &["py"];
use anyhow::Result;
use poolster_core::{GeneratedFile, GeneratedTree};

const MAX_FILE_BYTES: usize = 128 * 1024;

pub(super) fn diagnostics(tree: &mut GeneratedTree) -> Result<()> {
    let oversized = tree
        .iter()
        .filter(|(path, source)| {
            path.extension()
                .is_some_and(|extension| EXTENSIONS.contains(&extension.to_str().unwrap_or("")))
                && source.len() > MAX_FILE_BYTES
        })
        .map(|(path, source)| {
            serde_json::json!({
                "path": path,
                "bytes": source.len(),
                "max_file_bytes": MAX_FILE_BYTES,
                "reason": "Atomic declaration exceeds the source budget; retained intact."
            })
        })
        .collect::<Vec<_>>();
    if !oversized.is_empty() {
        tree.insert(GeneratedFile::new(
            ".poolster/source-layout-diagnostics.json",
            serde_json::to_string_pretty(&oversized)?,
        )?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::{GraphqlStyle, render};
    use poolster_core::native::{
        GraphqlOperation, GraphqlOperationKind, GraphqlOperations, ModelField, ModelKind, ModelType,
    };
    use std::collections::BTreeMap;

    #[test]
    fn oversized_operation_is_retained_and_reported() {
        // A native document is one transport payload: preserve it instead of
        // silently truncating it to meet the preferred source-file budget.
        let document = format!("query ReadUser {{ hello }} #{}", "x".repeat(140 * 1024));
        let contract = GraphqlOperations {
            schema_source: "type Query { hello: String! }".into(),
            operation_source: document.clone(),
            input_objects: BTreeMap::new(),
            operations: vec![GraphqlOperation {
                name: "ReadUser".into(),
                kind: GraphqlOperationKind::Query,
                document,
                variables: vec![],
                result: ModelType {
                    nullable: false,
                    kind: ModelKind::Object(vec![ModelField {
                        name: "hello".into(),
                        optional: false,
                        default_value: None,
                        ty: ModelType {
                            nullable: false,
                            kind: ModelKind::Scalar("String".into()),
                        },
                    }]),
                },
            }],
        };
        let (tree, _) = render(&contract, "example", GraphqlStyle::Flat, &BTreeMap::new()).unwrap();
        let report = tree
            .iter()
            .find(|(path, _)| path.to_string_lossy() == ".poolster/source-layout-diagnostics.json")
            .unwrap()
            .1;
        let report: Vec<serde_json::Value> = serde_json::from_str(report).unwrap();
        let oversized = tree
            .iter()
            .filter(|(path, source)| {
                path.extension()
                    .is_some_and(|extension| EXTENSIONS.contains(&extension.to_str().unwrap_or("")))
                    && source.len() > MAX_FILE_BYTES
            })
            .collect::<Vec<_>>();
        assert_eq!(report.len(), oversized.len());
        assert!(!report.is_empty());
        for (path, source) in oversized {
            let entry = report
                .iter()
                .find(|entry| entry["path"] == path.to_string_lossy().as_ref())
                .unwrap();
            assert_eq!(entry["bytes"].as_u64().unwrap() as usize, source.len());
            assert_eq!(entry["max_file_bytes"], MAX_FILE_BYTES);
            assert!(
                entry["reason"]
                    .as_str()
                    .unwrap()
                    .contains("retained intact")
            );
            assert!(source.contains(&"x".repeat(140 * 1024)));
        }
        let mut small = contract;
        small.operations[0].document = "query ReadUser { hello }".into();
        let (tree, _) = render(&small, "example", GraphqlStyle::Flat, &BTreeMap::new()).unwrap();
        assert!(
            !tree
                .iter()
                .any(|(path, _)| path.to_string_lossy()
                    == ".poolster/source-layout-diagnostics.json")
        );
    }
}
