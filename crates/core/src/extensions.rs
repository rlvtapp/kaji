//! Canonical Poolster OpenAPI extensions with Kaji compatibility.

use serde_json::Value;
use std::collections::BTreeMap;

/// Read an `x-poolster-*` extension, falling back to its legacy `x-kaji-*` name.
/// When both are present, the Poolster value wins.
pub fn poolster_extension<'a>(
    annotations: &'a BTreeMap<String, Value>,
    suffix: &str,
) -> Option<&'a Value> {
    annotations
        .get(&format!("x-poolster-{suffix}"))
        .or_else(|| annotations.get(&format!("x-kaji-{suffix}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_name_wins_and_legacy_name_remains_readable() {
        let mut extensions = BTreeMap::from([("x-kaji-pagination".into(), json!("legacy"))]);
        assert_eq!(
            poolster_extension(&extensions, "pagination"),
            Some(&json!("legacy"))
        );
        extensions.insert("x-poolster-pagination".into(), json!("canonical"));
        assert_eq!(
            poolster_extension(&extensions, "pagination"),
            Some(&json!("canonical"))
        );
    }
}
