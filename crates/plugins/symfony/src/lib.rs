//! Symfony integration packages for generated Poolster PHP SDKs.
//!
//! The package intentionally wraps the portable PSR-18 SDK instead of
//! re-rendering models or operations. Symfony applications receive normal
//! container configuration and `HttpClientInterface`; non-Symfony consumers
//! continue to use the same generated PHP SDK unchanged.

use anyhow::Result;
use poolster_core::{Api, GeneratedFile, GeneratedTree};

mod graphql;
mod package;
pub use graphql::{Graphql, graphql};
pub use package::{PackageExt, Sdk, Settings, Symfony, package, sdk};

mod http;
use http::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_a_bundle_that_wraps_the_php_sdk() {
        let api = Api {
            name: "Acme Email".into(),
            ..Api::default()
        };
        let tree = render_sdk(
            &api,
            "symfony",
            Some("acme/email-symfony"),
            Some("acme/email-sdk"),
        )
        .unwrap();
        assert!(
            tree.get("symfony/composer.json")
                .unwrap()
                .contains("acme/email-sdk")
        );
        assert!(
            tree.get("symfony/src/DependencyInjection/AcmeEmailExtension.php")
                .unwrap()
                .contains("Psr18Client")
        );
    }
    #[test]
    fn composer_manifest_preserves_metadata_and_namespace_escaping() {
        let title = "A \"quoted\" API \\ v2\nUnicode café";
        let source = composer("acme/sdk-symfony", "acme/sdk", "Acme\\Sdk", title);
        let manifest: serde_json::Value = serde_json::from_str(&source).unwrap();
        assert_eq!(
            manifest["description"],
            format!("Symfony integration for {title}")
        );
        assert_eq!(manifest["autoload"]["psr-4"]["Acme\\Sdk\\"], "src/");
        assert_eq!(manifest["require"]["acme/sdk"], "*");
        assert!(source.ends_with('\n'));
    }
}
