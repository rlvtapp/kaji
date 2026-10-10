//! Native collections and CLI packages share GraphQL provider validation.
use super::*;
pub(super) fn append(
    profiles: ProfileSet,
    package: SdkPackage,
    common: Common,
    input: InputProvider<GraphqlOperations>,
) -> AnyResult<ProfileSet> {
    match package.language.as_str() {
        "rust-cli" => {
            let mut generator = rust_cli::graphql().input(input.handle());
            if let Some(value) = package.command_name {
                generator = generator.command_name(value);
            }
            if let Some(value) = package.endpoint {
                generator = generator.endpoint(value);
            }
            let mut target = rust_cli::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            Ok(profiles.package(target))
        }
        "typescript-cli" => {
            let mut generator = ts_cli::graphql().input(input.handle());
            if let Some(value) = package.command_name {
                generator = generator.command_name(value);
            }
            if let Some(value) = package.endpoint {
                generator = generator.endpoint(value);
            }
            let mut target = ts_cli::package(package.path)
                .common(common)
                .with(input)
                .with(generator);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            Ok(profiles.package(target))
        }
        "postman" => {
            if package.command_name.is_some() {
                bail!("Postman does not accept commandName");
            }
            let generator = postman::graphql().input(input.handle()).strict(true);
            let environment = postman::graphql_environment().using_collection(generator.handle());
            let mut target = postman::package(package.path)
                .common(common)
                .with(input)
                .with(generator)
                .with(environment);
            if let Some(name) = package.name {
                target = target.name(name);
            }
            if let Some(endpoint) = package.endpoint {
                target = target.base_url(endpoint);
            }
            Ok(profiles.package(target))
        }
        _ => bail!("unsupported GraphQL tool output"),
    }
}
