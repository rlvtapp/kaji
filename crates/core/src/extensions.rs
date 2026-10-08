//! Canonical Poolster OpenAPI extensions.

use serde_json::Value;
use std::collections::BTreeMap;

/// Read an `x-poolster-*` extension.
pub fn poolster_extension<'a>(
    annotations: &'a BTreeMap<String, Value>,
    suffix: &str,
) -> Option<&'a Value> {
    annotations.get(&format!("x-poolster-{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_canonical_name() {
        let extensions = BTreeMap::from([("x-poolster-pagination".into(), json!("canonical"))]);
        assert_eq!(
            poolster_extension(&extensions, "pagination"),
            Some(&json!("canonical"))
        );
        assert_eq!(poolster_extension(&extensions, "missing"), None);
    }
}
