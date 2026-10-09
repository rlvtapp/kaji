use crate::blocks::BlockId;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolKey {
    pub entity: BlockId,
    pub target: String,
    /// Canonical target-relative module with forward slashes.
    pub module: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRequest {
    pub entity: BlockId,
    pub target: String,
    pub module: String,
    pub preferred: String,
}
impl SymbolRequest {
    pub fn key(&self) -> Result<SymbolKey> {
        ensure!(
            !self.entity.source.trim().is_empty() && !self.entity.local.trim().is_empty(),
            "symbol entity source and local identities cannot be empty"
        );
        validate_target(&self.target)?;
        Ok(SymbolKey {
            entity: self.entity.clone(),
            target: self.target.clone(),
            module: module(&self.module)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSymbol {
    pub key: SymbolKey,
    pub name: String,
}
/// A target chooses identifier comparison; modules always reject case-only aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolRules {
    pub case_sensitive: bool,
}
impl Default for SymbolRules {
    fn default() -> Self {
        Self {
            case_sensitive: true,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedSymbols {
    symbols: BTreeMap<SymbolKey, ResolvedSymbol>,
}
impl Serialize for ResolvedSymbols {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.symbols
            .values()
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}
impl crate::engine::Contract for ResolvedSymbols {
    const NAME: &'static str = "poolster.resolved-symbols.v1";
}
impl ResolvedSymbols {
    pub fn get(&self, key: &SymbolKey) -> Result<&ResolvedSymbol> {
        self.symbols.get(key).ok_or_else(|| {
            anyhow::anyhow!(
                "symbol was not reserved: {}:{} in {}/{}",
                key.entity.source,
                key.entity.local,
                key.target,
                key.module
            )
        })
    }
    pub fn for_request(&self, request: &SymbolRequest) -> Result<&ResolvedSymbol> {
        self.get(&request.key()?)
    }
    pub fn iter(&self) -> impl Iterator<Item = &ResolvedSymbol> {
        self.symbols.values()
    }
}
/// Mutable planning state. Every mutation is forbidden after successful resolution.
#[derive(Default)]
pub struct SymbolRequests {
    requests: BTreeMap<SymbolKey, SymbolRequest>,
    forbidden: BTreeMap<(String, String), BTreeSet<String>>,
    rules: BTreeMap<String, SymbolRules>,
    resolved: Option<ResolvedSymbols>,
}
impl SymbolRequests {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn target_rules(&mut self, target: impl Into<String>, rules: SymbolRules) -> Result<()> {
        self.planning()?;
        let target = target.into();
        validate_target(&target)?;
        ensure!(
            !self.requests.keys().any(|key| key.target == target)
                && !self.forbidden.keys().any(|key| key.0 == target),
            "target rules must be configured before reservations for {target}"
        );
        self.rules.insert(target, rules);
        Ok(())
    }
    /// Reserve external/runtime names in the same target/module namespace.
    pub fn reserve_name(
        &mut self,
        target: impl Into<String>,
        module_name: impl AsRef<str>,
        name: impl Into<String>,
    ) -> Result<()> {
        self.planning()?;
        let target = target.into();
        validate_target(&target)?;
        let name = name.into();
        identifier(&name)?;
        self.forbidden
            .entry((target, module(module_name.as_ref())?))
            .or_default()
            .insert(name);
        Ok(())
    }
    /// Repeated identical requests share a symbol; conflicting preferences fail.
    pub fn reserve(&mut self, mut request: SymbolRequest) -> Result<SymbolKey> {
        self.planning()?;
        identifier(&request.preferred)?;
        let key = request.key()?;
        request.module = key.module.clone();
        if let Some(existing) = self.requests.get(&key) {
            ensure!(
                existing.preferred == request.preferred,
                "conflicting symbol preferences for {}:{}: {} versus {}",
                key.entity.source,
                key.entity.local,
                existing.preferred,
                request.preferred
            );
        } else {
            self.requests.insert(key.clone(), request);
        }
        Ok(key)
    }
    /// Resolve atomically and freeze. Calling again returns the same immutable table.
    pub fn resolve(&mut self) -> Result<&ResolvedSymbols> {
        if self.resolved.is_none() {
            self.resolved = Some(self.allocate()?);
        }
        Ok(self.resolved.as_ref().unwrap())
    }
    /// Emission must use a reserved, resolved symbol rather than guessing its name.
    pub fn symbol(&self, key: &SymbolKey) -> Result<&ResolvedSymbol> {
        self.resolved
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("symbols must be resolved before emission"))?
            .get(key)
    }
    pub fn is_resolved(&self) -> bool {
        self.resolved.is_some()
    }
    fn planning(&self) -> Result<()> {
        ensure!(
            !self.is_resolved(),
            "symbol reservations are frozen after resolution"
        );
        Ok(())
    }
    fn allocate(&self) -> Result<ResolvedSymbols> {
        let mut namespaces: BTreeMap<(String, String), Vec<(&SymbolKey, &SymbolRequest)>> =
            BTreeMap::new();
        let mut portable_modules: BTreeMap<(String, String), String> = BTreeMap::new();
        for (target, module_name) in self.forbidden.keys() {
            portable_module(&mut portable_modules, target, module_name)?;
        }
        for (key, request) in &self.requests {
            portable_module(&mut portable_modules, &key.target, &key.module)?;
            namespaces
                .entry((key.target.clone(), key.module.clone()))
                .or_default()
                .push((key, request));
        }
        let mut result = ResolvedSymbols::default();
        for ((target, module_name), mut requests) in namespaces {
            requests.sort_by(|a, b| a.0.entity.cmp(&b.0.entity));
            let rules = self.rules.get(&target).copied().unwrap_or_default();
            let folded = |name: &str| {
                if rules.case_sensitive {
                    name.to_owned()
                } else {
                    name.to_ascii_lowercase()
                }
            };
            let mut used: BTreeSet<String> = self
                .forbidden
                .get(&(target, module_name))
                .into_iter()
                .flatten()
                .map(|name| folded(name))
                .collect();
            let preferred: BTreeSet<String> = requests
                .iter()
                .map(|(_, request)| folded(&request.preferred))
                .collect();
            let mut pending = Vec::new();
            // First protect every natural preferred name, including names like Foo_2.
            for (key, request) in requests {
                if used.insert(folded(&request.preferred)) {
                    result.symbols.insert(
                        key.clone(),
                        ResolvedSymbol {
                            key: key.clone(),
                            name: request.preferred.clone(),
                        },
                    );
                } else {
                    pending.push((key, request));
                }
            }
            for (key, request) in pending {
                let mut ordinal = 2u64;
                loop {
                    let candidate = format!("{}_{ordinal}", request.preferred);
                    let candidate_key = folded(&candidate);
                    if !preferred.contains(&candidate_key) && used.insert(candidate_key) {
                        result.symbols.insert(
                            key.clone(),
                            ResolvedSymbol {
                                key: key.clone(),
                                name: candidate,
                            },
                        );
                        break;
                    }
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or_else(|| anyhow::anyhow!("symbol suffix space exhausted"))?;
                }
            }
        }
        Ok(result)
    }
}
fn portable_module(
    modules: &mut BTreeMap<(String, String), String>,
    target: &str,
    module_name: &str,
) -> Result<()> {
    if let Some(existing) = modules.insert(
        (target.into(), module_name.to_ascii_lowercase()),
        module_name.into(),
    ) {
        ensure!(
            existing == module_name,
            "case-only module collision in {target}: {existing} versus {module_name}"
        );
    }
    Ok(())
}
fn validate_target(target: &str) -> Result<()> {
    ensure!(
        !target.trim().is_empty() && !target.chars().any(char::is_control),
        "symbol target must have a nonempty stable identity"
    );
    Ok(())
}
fn identifier(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .enumerate()
                .all(|(index, c)| c.is_ascii_alphabetic()
                    || c == '_'
                    || c == '$'
                    || (index > 0 && c.is_ascii_digit())),
        "preferred symbol must be an identifier, got {name:?}; apply target naming rules before reservation"
    );
    Ok(())
}
fn module(value: &str) -> Result<String> {
    let value = value.replace('\\', "/");
    ensure!(
        !value.starts_with('/') && !value.contains(':') && !value.contains('\0'),
        "symbol module must be target-relative"
    );
    let mut parts = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => anyhow::bail!("symbol module cannot traverse parent directories"),
            _ => parts.push(part),
        }
    }
    ensure!(!parts.is_empty(), "symbol module cannot be empty");
    Ok(parts.join("/"))
}
