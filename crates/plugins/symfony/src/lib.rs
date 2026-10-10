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

fn render_sdk(
    api: &Api,
    output_dir: &str,
    package_name: Option<&str>,
    sdk_package: Option<&str>,
) -> Result<GeneratedTree> {
    let root = output_dir.trim_matches('/');
    let sdk_package = sdk_package
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("poolster/{}-sdk", slug(&api.name)));
    let package_name = package_name
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("poolster/{}-symfony", slug(&api.name)));
    let package_namespace = namespace(&package_name);
    let sdk_namespace = namespace(&sdk_package);
    let bundle_name = format!("{}Bundle", class_name(&api.name));
    let extension_name = format!("{}Extension", class_name(&api.name));
    let alias = slug(&api.name).replace('-', "_");
    let mut tree = GeneratedTree::default();
    insert(
        &mut tree,
        root,
        "composer.json",
        composer(&package_name, &sdk_package, &package_namespace, &api.name),
    )?;
    insert(
        &mut tree,
        root,
        "README.md",
        readme(&api.name, &sdk_package, &alias),
    )?;
    insert(
        &mut tree,
        root,
        &format!("src/{bundle_name}.php"),
        bundle(&package_namespace, &bundle_name),
    )?;
    insert(
        &mut tree,
        root,
        &format!("src/DependencyInjection/{extension_name}.php"),
        extension(&package_namespace, &extension_name, &sdk_namespace, &alias),
    )?;
    insert(
        &mut tree,
        root,
        "src/DependencyInjection/Configuration.php",
        configuration(&package_namespace, &alias),
    )?;
    Ok(tree)
}

fn insert(tree: &mut GeneratedTree, root: &str, path: &str, contents: String) -> Result<()> {
    let path = if root.is_empty() || root == "." {
        path.into()
    } else {
        format!("{root}/{path}")
    };
    tree.insert(GeneratedFile::new(path, contents)?)
}

fn composer(package: &str, sdk: &str, namespace: &str, api: &str) -> String {
    let autoload = format!("{}\\\\", namespace.replace('\\', "\\\\"));
    format!(
        "{{\n  \"name\": \"{package}\",\n  \"description\": \"Symfony integration for {api}\",\n  \"type\": \"symfony-bundle\",\n  \"require\": {{\n    \"php\": \">=8.2\",\n    \"{sdk}\": \"*\",\n    \"symfony/config\": \"^6.4 || ^7.0\",\n    \"symfony/dependency-injection\": \"^6.4 || ^7.0\",\n    \"symfony/http-client\": \"^6.4 || ^7.0\",\n    \"symfony/http-kernel\": \"^6.4 || ^7.0\"\n  }},\n  \"autoload\": {{\n    \"psr-4\": {{\n      \"{autoload}\": \"src/\"\n    }}\n  }}\n}}\n"
    )
}

fn readme(api: &str, sdk: &str, alias: &str) -> String {
    format!(
        "# {api} Symfony integration\n\nThis generated bundle configures the portable [`{sdk}`](https://packagist.org/packages/{sdk}) PHP SDK as a Symfony service. It adapts Symfony HttpClient to PSR-18; models and operations remain owned by the PHP SDK.\n\n```yaml\n# config/packages/{alias}.yaml\n{alias}:\n  base_url: '%env({alias_upper}_BASE_URL)%'\n  api_key: '%env({alias_upper}_API_KEY)%'\n```\n\nAutowire the generated SDK client directly after enabling the bundle. Override `http_client` with a named Symfony HTTP client service when needed.\n",
        alias_upper = alias.to_ascii_uppercase()
    )
}

fn bundle(namespace: &str, bundle: &str) -> String {
    format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace};\n\nuse Symfony\\Component\\HttpKernel\\Bundle\\Bundle;\n\nfinal class {bundle} extends Bundle\n{{\n}}\n"
    )
}

fn extension(namespace: &str, extension: &str, sdk_namespace: &str, alias: &str) -> String {
    format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\DependencyInjection;\n\nuse {sdk_namespace}\\Client;\nuse Symfony\\Component\\Config\\FileLocator;\nuse Symfony\\Component\\DependencyInjection\\ContainerBuilder;\nuse Symfony\\Component\\DependencyInjection\\Definition;\nuse Symfony\\Component\\DependencyInjection\\Extension\\Extension;\nuse Symfony\\Component\\DependencyInjection\\Reference;\nuse Symfony\\Component\\HttpClient\\Psr18Client;\n\nfinal class {extension} extends Extension\n{{\n    public function getAlias(): string\n    {{\n        return '{alias}';\n    }}\n\n    public function load(array $configs, ContainerBuilder $container): void\n    {{\n        $config = $this->processConfiguration(new Configuration(), $configs);\n        $http = new Definition(Psr18Client::class, [new Reference($config['http_client'])]);\n        $client = new Definition(Client::class, [\n            $http,\n            $config['base_url'],\n            $config['api_key'],\n            $config['api_key_header'],\n            $config['api_key_prefix'],\n        ]);\n        $client->setAutowired(true);\n        $client->setPublic(false);\n        $container->setDefinition(Client::class, $client);\n    }}\n}}\n"
    )
}

fn configuration(namespace: &str, alias: &str) -> String {
    format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace {namespace}\\DependencyInjection;\n\nuse Symfony\\Component\\Config\\Definition\\Builder\\TreeBuilder;\nuse Symfony\\Component\\Config\\Definition\\ConfigurationInterface;\n\nfinal class Configuration implements ConfigurationInterface\n{{\n    public function getConfigTreeBuilder(): TreeBuilder\n    {{\n        $tree = new TreeBuilder('{alias}');\n        $tree->getRootNode()\n            ->children()\n                ->scalarNode('base_url')->isRequired()->cannotBeEmpty()->end()\n                ->scalarNode('api_key')->defaultNull()->end()\n                ->scalarNode('api_key_header')->defaultValue('Authorization')->end()\n                ->scalarNode('api_key_prefix')->defaultValue('Bearer')->end()\n                ->scalarNode('http_client')->defaultValue('http_client')->end()\n            ->end();\n        return $tree;\n    }}\n}}\n"
    )
}

fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = true;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            result.extend(character.to_lowercase());
            separator = false;
        } else if !separator && !result.is_empty() {
            result.push('-');
            separator = true;
        }
    }
    result.trim_matches('-').to_owned().if_empty("api")
}
fn class_name(value: &str) -> String {
    namespace(value).replace('\\', "")
}
fn namespace(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("\\")
}
trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

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
}
