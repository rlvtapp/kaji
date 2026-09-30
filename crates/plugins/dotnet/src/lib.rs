//! Deprecated compatibility facade for [`kaji_plugin_csharp`].
//!
//! New embedded profiles should depend on `kaji-plugin-csharp` and use its
//! `csharp` target. This crate deliberately re-exports the exact same types,
//! preventing generator drift while keeping the original package available.

use kaji_core::engine::Package;
use kaji_plugin_csharp::dotnet_package;
pub use kaji_plugin_csharp::{DotNet, PackageExt, Sdk, Settings, sdk};

/// Creates a package using the legacy `dotnet` language identity.
pub fn package(dir: impl Into<String>) -> Package<DotNet> {
    dotnet_package(dir)
}
