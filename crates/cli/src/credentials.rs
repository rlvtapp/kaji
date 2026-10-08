use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
struct CredentialStore {
    profiles: BTreeMap<String, CredentialProfile>,
}

#[derive(Deserialize, Serialize)]
struct CredentialProfile {
    token_env: String,
}

pub fn login(name: &str, token_env: &str) -> Result<()> {
    validate_name(name)?;
    if token_env.trim().is_empty() {
        bail!("--token-env cannot be empty")
    }
    env::var(token_env)
        .with_context(|| format!("read token environment variable {token_env:?}"))?;
    let path = store_path()?;
    let mut store = load(&path)?;
    store.profiles.insert(
        name.into(),
        CredentialProfile {
            token_env: token_env.into(),
        },
    );
    save(&path, &store)
}

pub fn logout(name: &str) -> Result<bool> {
    validate_name(name)?;
    let path = store_path()?;
    let mut store = load(&path)?;
    let removed = store.profiles.remove(name).is_some();
    if removed {
        save(&path, &store)?;
    }
    Ok(removed)
}

pub fn profiles() -> Result<Vec<(String, String)>> {
    let path = store_path()?;
    Ok(load(&path)?
        .profiles
        .into_iter()
        .map(|(name, profile)| (name, profile.token_env))
        .collect())
}

pub fn resolve(name: &str) -> Result<String> {
    validate_name(name)?;
    let path = store_path()?;
    let store = load(&path)?;
    let profile = store.profiles.get(name).with_context(|| {
        format!("no Poolster auth profile named {name:?}; run poolster auth login")
    })?;
    env::var(&profile.token_env).with_context(|| {
        format!(
            "read token environment variable {:?} for Poolster auth profile {name:?}",
            profile.token_env
        )
    })
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        bail!("auth profile names use letters, numbers, hyphens, and underscores")
    }
    Ok(())
}

fn store_path() -> Result<PathBuf> {
    let base = env::var_os("POOLSTER_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("locate a Poolster config directory; set POOLSTER_CONFIG_HOME")?;
    Ok(base.join("poolster").join("auth.json"))
}

fn load(path: &PathBuf) -> Result<CredentialStore> {
    if !path.exists() {
        return Ok(CredentialStore::default());
    }
    let document = std::fs::read_to_string(path)
        .with_context(|| format!("read Poolster auth profiles {}", path.display()))?;
    serde_json::from_str(&document)
        .with_context(|| format!("parse Poolster auth profiles {}", path.display()))
}

fn save(path: &PathBuf, store: &CredentialStore) -> Result<()> {
    let parent = path.parent().expect("auth path has a parent");
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "create Poolster auth profile directory {}",
            parent.display()
        )
    })?;
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(store)?))
        .with_context(|| format!("write Poolster auth profiles {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restrict Poolster auth profiles {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn auth_profile_names_are_deliberately_small() {
        assert!(validate_name("github-work").is_ok());
        assert!(validate_name("team_a").is_ok());
        assert!(validate_name("../escape").is_err());
    }
}
