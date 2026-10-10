use super::*;

pub(super) fn append(
    mut profiles: ProfileSet,
    request: &Request,
    package: SdkPackage,
    common: Common,
    input: InputProvider<GraphqlOperations>,
    subscriptions: bool,
    style: &str,
) -> AnyResult<ProfileSet> {
    let mut mappings = request.scalars.clone();
    for (name, mapping) in &package.scalars {
        if mappings.get(name).is_some_and(|old| old != mapping) {
            bail!("conflicting TypeScript scalar mapping {name}");
        }
        mappings.insert(name.clone(), mapping.clone());
    }
    let mut generator = ts::graphql(Some(input.handle()))
        .scalars(mappings)
        .groups(package.groups.clone());
    generator = if package.raw.unwrap_or(false) || style == "raw" {
        generator.raw()
    } else if style == "flat" {
        generator.flat()
    } else {
        generator.idiomatic()
    };
    if subscriptions {
        generator = generator.subscriptions();
    }
    let client = generator.handle();
    let mut target = ts::package(package.path)
        .common(common)
        .with(input)
        .with(generator);
    if let Some(name) = package.name {
        target = target.name(name);
    }
    let target = graphql_addons::attach(target, package.plugins, client)?;
    profiles = profiles.package(target);
    Ok(profiles)
}
