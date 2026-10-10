//! Rust SDK generation through typed packages and a Reqwest-backed client.
pub mod contracts;

mod graphql;
pub use graphql::{
    Graphql, GraphqlClient, GraphqlOperationSymbols, GraphqlScalarMapping, GraphqlStyle, graphql,
    graphql_incremental,
};
mod model_compatibility;
mod native_names;
mod oauth;
mod open_union;
pub use oauth::{OAuth, oauth};
mod webhooks;
pub use webhooks::{Webhooks, webhooks};
mod bundled;
pub mod composition;
mod operation_tests;
mod render;
pub use composition::{client, models, operations, roundtrip_tests, transport};
pub use operation_tests::{OperationTests, operation_tests};

use anyhow::Result;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};
use poolster_core::{GeneratedFile, SdkClientStyle};

mod package;
pub use package::{PackageExt, Rust, Settings, package};
mod http;
pub use http::{Sdk, sdk};

#[cfg(test)]
mod tests;

#[cfg(test)]
fn native_cargo() -> std::process::Command {
    let mut command = std::process::Command::new("cargo");
    configure_native_cargo(
        &mut command,
        std::env::var("POOLSTER_RUNTIME_OFFLINE").as_deref() == Ok("1"),
    );
    command
}

#[cfg(test)]
fn configure_native_cargo(command: &mut std::process::Command, offline: bool) {
    if offline {
        command.arg("--offline");
    }
    command.env(
        "CARGO_TARGET_DIR",
        std::env::var_os("POOLSTER_RUNTIME_RUST_TARGET").unwrap_or_else(|| {
            std::env::temp_dir()
                .join("poolster-runtime-contract-rust-target")
                .into_os_string()
        }),
    );
}

#[cfg(test)]
mod native_cargo_tests {
    #[test]
    fn dependency_downloads_are_allowed_unless_offline_is_requested() {
        for offline in [false, true] {
            let mut command = std::process::Command::new("cargo");
            super::configure_native_cargo(&mut command, offline);
            assert_eq!(command.get_args().any(|arg| arg == "--offline"), offline);
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == "CARGO_TARGET_DIR" && value.is_some())
            );
        }
    }
}

#[cfg(test)]
mod multipart_tests;

#[cfg(test)]
mod source_layout_tests;
