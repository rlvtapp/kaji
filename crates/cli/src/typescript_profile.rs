use super::*;

pub(super) fn typescript_profile(
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<Package<ts::TypeScript>> {
    use ts::composition::{self, Models, Operations, Transport};
    has_only_known_plugins(
        package,
        &[
            "sdk",
            "models",
            "transport",
            "operations",
            "client",
            "zod",
            "tanstack-react-query",
            "tanstack-vue-query",
            "swr",
            "faker",
            "msw",
            "cypress",
            "operation-tests",
            "webhooks",
            "oauth",
        ],
    )?;
    if package.plugins.is_empty() {
        bail!("TypeScript package must declare plugins");
    }
    let mut output = ts::package(&package.path).common(configured_common(style, package));
    if let Some(name) = &package.name {
        output = output.name(name);
    }
    if !has_typescript_provider(package) {
        return Ok(output);
    }
    let mut models = BTreeMap::<String, Handle<Models>>::new();
    let mut transports = BTreeMap::<String, Handle<Transport>>::new();
    let mut operations = BTreeMap::<String, Handle<Operations>>::new();
    let mut ids = BTreeSet::new();
    let mut providers = Vec::new();
    for plugin in &package.plugins {
        let id = plugin.id.clone().unwrap_or_else(|| plugin.name.clone());
        if !ids.insert(id.clone()) {
            bail!("duplicate TypeScript plugin instance {id:?}; give each instance a unique id");
        }
        if plugin.name == "sdk" {
            if package.plugins.iter().any(|p| {
                matches!(
                    p.name.as_str(),
                    "models" | "transport" | "operations" | "client"
                )
            }) {
                bail!("select either sdk convenience or explicit SDK providers in one package");
            }
            if !plugin.uses.is_empty() {
                bail!("sdk convenience does not accept provider bindings; use explicit providers");
            }
            let mut sdk = match plugin.transport.as_deref().unwrap_or("fetch") {
                "fetch" => ts::sdk().fetch(),
                "axios" => ts::sdk().axios(),
                other => bail!("unknown TypeScript transport {other:?}"),
            }
            .label(&id)
            .model_options(typescript_models(plugin)?);
            match plugin.surface.as_deref().unwrap_or("client") {
                "raw" => sdk = sdk.raw(),
                "client" => {}
                other => bail!("unknown TypeScript surface {other:?}"),
            }
            if let Some(name) = &plugin.client_name {
                sdk = sdk.client_name(name);
            }
            if let Some(group) = plugin.group_by_tag {
                sdk = sdk.group_by_tag(group);
            }
            if let Some(throws) = plugin.throw_on_error {
                sdk = sdk.throw_on_error(throws);
            }
            models.insert(id.clone(), sdk.meta().handle());
            transports.insert(id.clone(), sdk.meta().handle());
            operations.insert(id, sdk.meta().handle());
            output = output.with(sdk);
        } else if matches!(
            plugin.name.as_str(),
            "models" | "transport" | "operations" | "client"
        ) {
            let mut provider = match plugin.name.as_str() {
                "models" => composition::models().model_options(typescript_models(plugin)?),
                "transport" => match plugin.transport.as_deref().unwrap_or("fetch") {
                    "fetch" => composition::transport(),
                    "axios" => composition::transport().axios(),
                    other => bail!("unknown TypeScript transport {other:?}"),
                },
                "operations" => composition::operations(),
                "client" => composition::client(),
                _ => unreachable!(),
            }
            .label(&id);
            if let Some(module) = &plugin.output {
                provider = provider.output(module.trim_end_matches(".ts"));
            }
            if let Some(name) = &plugin.client_name {
                provider = provider.client_name(name);
            }
            if let Some(throws) = plugin.throw_on_error {
                provider = provider.throw_on_error(throws);
            }
            if plugin.name == "client" && style == SdkClientStyle::Namespaced {
                provider = provider.namespaced();
            }
            match plugin.name.as_str() {
                "models" => {
                    models.insert(id, provider.models_handle());
                }
                "transport" => {
                    transports.insert(id, provider.transport_handle());
                }
                "operations" => {
                    operations.insert(id, provider.operations_handle());
                }
                _ => {}
            }
            providers.push((plugin, provider));
        }
    }
    for (plugin, mut provider) in providers {
        for (role, id) in &plugin.uses {
            let allowed = match plugin.name.as_str() {
                "operations" => matches!(role.as_str(), "models" | "transport"),
                "client" => matches!(role.as_str(), "models" | "transport" | "operations"),
                _ => false,
            };
            ensure!(
                allowed,
                "{} provider does not accept uses.{role}",
                plugin.name
            );
            provider = match role.as_str() {
                "models" => provider.using_models(
                    *models
                        .get(id)
                        .with_context(|| format!("no model provider {id:?}"))?,
                ),
                "transport" => provider.using_transport(
                    *transports
                        .get(id)
                        .with_context(|| format!("no transport provider {id:?}"))?,
                ),
                "operations" => provider.using_operations(
                    *operations
                        .get(id)
                        .with_context(|| format!("no operation provider {id:?}"))?,
                ),
                other => bail!("unknown provider binding {other:?}"),
            };
        }
        output = output.with(provider);
    }
    for plugin in &package.plugins {
        let query_consumer = matches!(
            plugin.name.as_str(),
            "tanstack-react-query" | "tanstack-vue-query" | "swr"
        );
        let auxiliary_consumer =
            matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress");
        ensure!(
            plugin.layout.is_none() || query_consumer || auxiliary_consumer,
            "layout applies to query and auxiliary consumers only"
        );
        ensure!(
            query_consumer
                || (plugin.include_operations.is_none()
                    && plugin.operation_kinds.is_none()
                    && plugin.operation_names.is_none()),
            "operation selection/classification/names apply to query consumers only"
        );
        ensure!(
            plugin.fixture_options.is_none()
                || matches!(plugin.name.as_str(), "faker" | "msw" | "cypress"),
            "fixture_options applies to Faker/MSW/Cypress only"
        );
        ensure!(
            plugin.cypress_options.is_none() || plugin.name == "cypress",
            "cypress_options applies to Cypress only"
        );
        if plugin.max_file_bytes.is_some()
            && !matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress")
        {
            bail!("max_file_bytes applies to auxiliary consumers only");
        }
        if plugin.max_operations_per_file.is_some()
            && !matches!(
                plugin.name.as_str(),
                "tanstack-react-query" | "tanstack-vue-query" | "swr"
            )
        {
            bail!("max_operations_per_file applies to query consumers only");
        }
        let id = plugin.id.as_deref().unwrap_or(&plugin.name);
        let module = plugin.output.as_deref().map(|directory| {
            let stem = match plugin.name.as_str() {
                "tanstack-react-query" => "react-query",
                "tanstack-vue-query" => "vue-query",
                "cypress" => "api.cy",
                other => other,
            };
            Path::new(directory)
                .join(stem)
                .to_string_lossy()
                .into_owned()
        });
        if plugin.name == "oauth" {
            let mut consumer = ts::oauth();
            for (role, id) in &plugin.uses {
                ensure!(
                    role == "transport",
                    "OAuth accepts only a transport provider binding"
                );
                consumer = consumer.using_transport(
                    *transports
                        .get(id)
                        .with_context(|| format!("no transport provider {id:?}"))?,
                );
            }
            output = output.with(consumer);
        } else if plugin.name == "webhooks" {
            ensure!(
                plugin.uses.is_empty(),
                "webhooks does not accept provider bindings"
            );
            output = output.with(ts::webhooks());
        } else if plugin.name == "operation-tests" {
            let mut consumer = ts::operation_tests();
            for (role, id) in &plugin.uses {
                consumer = match role.as_str() {
                    "operations" => consumer.using_operations(
                        *operations
                            .get(id)
                            .with_context(|| format!("no operation provider {id:?}"))?,
                    ),
                    "transport" => consumer.using_transport(
                        *transports
                            .get(id)
                            .with_context(|| format!("no transport provider {id:?}"))?,
                    ),
                    "models" => consumer.using_models(
                        *models
                            .get(id)
                            .with_context(|| format!("no model provider {id:?}"))?,
                    ),
                    _ => bail!(
                        "operation-tests only accepts uses.models, uses.operations and uses.transport"
                    ),
                };
            }
            output = output.with(consumer);
        } else if matches!(
            plugin.name.as_str(),
            "tanstack-react-query" | "tanstack-vue-query" | "swr"
        ) {
            if plugin.clients_import.is_some() {
                bail!(
                    "native query plugins resolve imports through providers; use uses.operations instead of clients_import"
                );
            }
            let mut query = match plugin.name.as_str() {
                "tanstack-react-query" => composition::react_query(),
                "tanstack-vue-query" => composition::vue_query(),
                _ => composition::swr(),
            }
            .label(id);
            if let Some(count) = plugin.max_operations_per_file {
                query = query.max_operations_per_file(count);
            }
            if let Some(layout) = &plugin.layout {
                query = query.layout(layout.clone());
            }
            if let Some(ids) = &plugin.include_operations {
                query = query.include_operations(ids.clone());
            }
            for (operation, kind) in plugin.operation_kinds.iter().flatten() {
                query = query.operation_kind(
                    operation,
                    match kind.as_str() {
                        "query" => composition::QueryKind::Query,
                        "mutation" => composition::QueryKind::Mutation,
                        _ => bail!("operation kind must be query or mutation"),
                    },
                );
            }
            for (operation, name) in plugin.operation_names.iter().flatten() {
                query = query.operation_name(operation, name);
            }
            if let Some(module) = &module {
                query = query.output(module);
            }
            for (role, id) in &plugin.uses {
                if role != "operations" {
                    bail!("query consumer only accepts uses.operations");
                }
                query = query.using_operations(
                    *operations
                        .get(id)
                        .with_context(|| format!("no operation provider {id:?}"))?,
                );
            }
            output = output.with(query);
        } else if matches!(plugin.name.as_str(), "zod" | "faker" | "msw" | "cypress") {
            let mut consumer = match plugin.name.as_str() {
                "zod" => composition::zod(),
                "faker" => composition::faker(),
                "msw" => composition::msw(),
                _ => composition::cypress(),
            }
            .label(id);
            if let Some(bytes) = plugin.max_file_bytes {
                consumer = consumer.max_file_bytes(bytes);
            }
            if let Some(layout) = &plugin.layout {
                consumer = consumer.layout(layout.clone());
            }
            if let Some(fixtures) = &plugin.fixture_options {
                consumer = consumer.fixture_options(fixtures.clone());
            }
            if let Some(options) = &plugin.cypress_options {
                consumer = consumer.cypress_options(options.clone());
            }
            if plugin.name == "cypress" && module.is_none() {
                consumer = consumer.output("api.cy");
            }
            if let Some(module) = &module {
                consumer = consumer.output(module);
            }
            for (role, id) in &plugin.uses {
                ensure!(
                    role != "operations" || matches!(plugin.name.as_str(), "msw" | "cypress"),
                    "{} consumer does not accept uses.operations",
                    plugin.name
                );
                consumer = match role.as_str() {
                    "models" => consumer.using_models(
                        *models
                            .get(id)
                            .with_context(|| format!("no model provider {id:?}"))?,
                    ),
                    "operations" => consumer.using_operations(
                        *operations
                            .get(id)
                            .with_context(|| format!("no operation provider {id:?}"))?,
                    ),
                    other => bail!("unknown auxiliary binding {other:?}"),
                };
            }
            output = output.with(consumer);
        }
    }
    Ok(output)
}
