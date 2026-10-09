//! Existing addon names bind to the selected GraphQL output contract.
use super::*;
use poolster_core::engine::{Handle, Package};
pub(super) fn attach(
    mut target: Package<ts::TypeScript>,
    plugins: &[PluginConfig],
    client: Handle<ts::GraphqlClient>,
) -> Result<Package<ts::TypeScript>> {
    for plugin in plugins
        .iter()
        .filter(|p| !matches!(p.name.as_str(), "graphql" | "sdk"))
    {
        target = match plugin.name.as_str() {
            "zod" | "faker" | "msw" | "cypress" => {
                let mut auxiliary = match plugin.name.as_str() {
                    "zod" => ts::composition::zod(),
                    "faker" => ts::composition::faker(),
                    "msw" => ts::composition::msw(),
                    _ => ts::composition::cypress(),
                }
                .using_graphql(Some(client));
                if let Some(options) = &plugin.fixture_options {
                    auxiliary = auxiliary.fixture_options(options.clone());
                }
                if let Some(options) = &plugin.cypress_options {
                    auxiliary = auxiliary.cypress_options(options.clone());
                }
                if let Some(output) = &plugin.output {
                    auxiliary = auxiliary.output(output);
                }
                target.with(auxiliary)
            }
            "react-query" | "vue-query" | "swr" => {
                let mut query = match plugin.name.as_str() {
                    "react-query" => ts::composition::react_query(),
                    "vue-query" => ts::composition::vue_query(),
                    _ => ts::composition::swr(),
                }
                .using_graphql(Some(client));
                if let Some(output) = &plugin.output {
                    query = query.output(output);
                }
                target.with(query)
            }
            _ => bail!("native GraphQL addon {:?} is not registered", plugin.name),
        };
    }
    Ok(target)
}
