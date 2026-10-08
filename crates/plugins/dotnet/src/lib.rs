//! Deprecated compatibility facade for [`poolster_plugin_csharp`].
//!
//! New embedded profiles should depend on `poolster-plugin-csharp` and use its
//! `csharp` target. This crate deliberately re-exports the exact same types,
//! preventing generator drift while keeping the original package available.

use poolster_core::engine::Package;
use poolster_plugin_csharp::dotnet_package;
pub use poolster_plugin_csharp::{DotNet, PackageExt, Sdk, Settings, sdk};

/// Creates a package using the legacy `dotnet` language identity.
pub fn package(dir: impl Into<String>) -> Package<DotNet> {
    dotnet_package(dir)
}
