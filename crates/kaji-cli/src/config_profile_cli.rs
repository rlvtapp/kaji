use super::*;

pub(super) fn apply_typescript(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok(profiles.package(with_configured_middleware(
        typescript_profile(package, style)?,
        package,
    )))
}

pub(super) fn apply_typescript_cli(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["cli"])?;
        let plugins = package
            .plugins
            .iter()
            .filter(|plugin| plugin.name == "cli")
            .collect::<Vec<_>>();
        let [plugin] = plugins.as_slice() else {
            bail!(
                "typescript-cli package {:?} requires exactly one {{\"name\":\"cli\"}} plugin",
                package.path
            )
        };
        let mut generator = ts_cli::cli();
        if let Some(command_name) = &plugin.command_name {
            generator = generator.command_name(command_name);
        }
        if let Some(base_url) = &plugin.base_url {
            generator = generator.base_url(base_url);
        }
        if let Some(oauth) = &plugin.oauth {
            if let Some(flow) = oauth.preferred_flow.as_deref()
                && !matches!(flow, "device" | "browser")
            {
                bail!("typescript-cli oauth.preferred_flow must be \"device\" or \"browser\"");
            }
            let mut settings = ts_cli::OAuthConfig::default().client_id(&oauth.client_id);
            if let Some(scheme) = &oauth.security_scheme {
                settings = settings.security_scheme(scheme);
            }
            if !oauth.scopes.is_empty() {
                settings = settings.scopes(oauth.scopes.clone());
            }
            if let Some(flow) = &oauth.preferred_flow {
                settings = settings.preferred_flow(flow);
            }
            if let Some(url) = &oauth.authorization_url {
                settings = settings.authorization_url(url);
            }
            if let Some(url) = &oauth.device_authorization_url {
                settings = settings.device_authorization_url(url);
            }
            if let Some(url) = &oauth.token_url {
                settings = settings.token_url(url);
            }
            if let Some(uri) = &oauth.redirect_uri {
                settings = settings.redirect_uri(uri);
            }
            generator = generator.oauth(settings);
        }
        let package_builder =
            ts_cli::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        profiles.package(with_configured_middleware(
            package_builder.with(generator),
            package,
        ))
    })
}

pub(super) fn apply_rust_cli(
    profiles: ProfileSet,
    package: &PackageConfig,
    style: SdkClientStyle,
) -> Result<ProfileSet> {
    Ok({
        has_only_known_plugins(package, &["cli"])?;
        let plugins = package
            .plugins
            .iter()
            .filter(|plugin| plugin.name == "cli")
            .collect::<Vec<_>>();
        let [plugin] = plugins.as_slice() else {
            bail!(
                "rust-cli package {:?} requires exactly one {{\"name\":\"cli\"}} plugin",
                package.path
            )
        };
        let mut generator = rust_cli::cli();
        if let Some(command_name) = &plugin.command_name {
            generator = generator.command_name(command_name);
        }
        if let Some(base_url) = &plugin.base_url {
            generator = generator.base_url(base_url);
        }
        let package_builder =
            rust_cli::package(&package.path).common(configured_common(style, package));
        let package_builder = if let Some(name) = &package.name {
            package_builder.name(name)
        } else {
            package_builder
        };
        profiles.package(with_configured_middleware(
            package_builder.with(generator),
            package,
        ))
    })
}
