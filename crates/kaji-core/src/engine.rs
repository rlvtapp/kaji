//! Typed package composition for native and community generators.
//!
//! Contracts are package-local, immutable once published, and looked up by
//! Rust type. Language implementations own package settings and shared state.

use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, BTreeSet, HashMap},
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail};

use crate::{
    Api, GeneratedFile, GeneratedTree, SdkClientStyle, SdkSemantics, SecuritySchemeCatalog,
    analyze_sdk_semantics,
};

/// Optional shared settings. Unset values remain unset until a plugin applies
/// its defaults; false/flat/empty explicit values are never treated as absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Common {
    pub client_name: Option<String>,
    pub client_style: Option<SdkClientStyle>,
    pub package_version: Option<String>,
}

impl Common {
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = Some(name.into());
        self
    }
    pub fn client_style(mut self, style: SdkClientStyle) -> Self {
        self.client_style = Some(style);
        self
    }
    pub fn package_version(mut self, version: impl Into<String>) -> Self {
        self.package_version = Some(version.into());
        self
    }
    pub fn overlay(&self, local: &Self) -> Self {
        Self {
            client_name: local
                .client_name
                .clone()
                .or_else(|| self.client_name.clone()),
            client_style: local.client_style.or(self.client_style),
            package_version: local
                .package_version
                .clone()
                .or_else(|| self.package_version.clone()),
        }
    }
}

/// Process-local identity. Never used in paths, output bytes, or ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(u64);

impl InstanceId {
    fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Metadata belongs to an instance, not to a generator kind. Not Clone:
/// construct another plugin to obtain a new identity.
#[derive(Debug)]
pub struct Meta {
    id: InstanceId,
    label: Option<String>,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            id: InstanceId::fresh(),
            label: None,
        }
    }
}

impl Meta {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn handle<C: Contract>(&self) -> Handle<C> {
        Handle {
            id: self.id,
            marker: PhantomData,
        }
    }
}

pub trait Contract: Any + Send + Sync {
    const NAME: &'static str;
}

pub struct Handle<C: Contract> {
    id: InstanceId,
    marker: PhantomData<fn() -> C>,
}
impl<C: Contract> Copy for Handle<C> {}
impl<C: Contract> Clone for Handle<C> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Clone, Copy)]
pub struct Provision {
    type_id: TypeId,
    name: &'static str,
}
impl Provision {
    pub fn of<C: Contract>() -> Self {
        Self {
            type_id: TypeId::of::<C>(),
            name: C::NAME,
        }
    }
}

pub struct Requirement {
    contract: Provision,
    provider: Option<InstanceId>,
    optional: bool,
}
impl Requirement {
    pub fn on<C: Contract>(handle: Option<Handle<C>>) -> Self {
        Self {
            contract: Provision::of::<C>(),
            provider: handle.map(|h| h.id),
            optional: false,
        }
    }
    /// An absent automatic provider is allowed. Explicit missing handles and
    /// ambiguous optional providers still fail. Optional edges also form cycles.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

pub trait Language: Send + Sync + Sized + 'static {
    const NAME: &'static str;
    type Settings: Default + Send + Sync;
    type Workspace: Default + Send;
    fn finalize(_cx: &mut FinalizeContext<'_, Self>) -> Result<()> {
        Ok(())
    }
}

pub trait Plugin<L: Language>: Send + Sync + 'static {
    fn kind(&self) -> &'static str;
    fn meta(&self) -> &Meta;
    fn requires(&self) -> Vec<Requirement> {
        Vec::new()
    }
    fn provides(&self) -> Vec<Provision> {
        Vec::new()
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()>;
}

type Contracts = HashMap<(InstanceId, TypeId), Box<dyn Any + Send + Sync>>;
type Bindings = HashMap<TypeId, Option<InstanceId>>;

pub struct Inputs<'a> {
    contracts: &'a Contracts,
    bindings: &'a Bindings,
}
impl Inputs<'_> {
    pub fn get<C: Contract>(&self) -> Result<&C> {
        self.optional::<C>()?
            .with_context(|| format!("no provider bound for {}", C::NAME))
    }
    /// Fails for undeclared reads, instead of silently returning None.
    pub fn optional<C: Contract>(&self) -> Result<Option<&C>> {
        let provider = self
            .bindings
            .get(&TypeId::of::<C>())
            .with_context(|| format!("undeclared contract read: {}", C::NAME))?;
        provider
            .map(|id| {
                self.contracts
                    .get(&(id, TypeId::of::<C>()))
                    .and_then(|value| value.downcast_ref::<C>())
                    .with_context(|| format!("provider did not publish {}", C::NAME))
            })
            .transpose()
    }
}

/// Package-relative emitter. Ownership is retained for collision diagnostics.
pub struct Emitter<'a> {
    tree: &'a mut GeneratedTree,
    owners: &'a mut BTreeMap<PathBuf, String>,
    owner: String,
}
impl Emitter<'_> {
    pub fn emit(&mut self, file: GeneratedFile) -> Result<()> {
        self.insert(file, false)
    }
    pub fn emit_custom(&mut self, file: GeneratedFile) -> Result<()> {
        self.insert(file, true)
    }
    fn insert(&mut self, file: GeneratedFile, custom: bool) -> Result<()> {
        // Normalize lexical aliases before collision detection.
        let path = checked_path(&file.path)?;
        if let Some(previous) = self.owners.get(&path) {
            bail!(
                "{} and {} both emit {}",
                previous,
                self.owner,
                path.display()
            );
        }
        let file = GeneratedFile::new(&path, file.contents)?;
        if custom {
            self.tree.insert_custom(file)?;
        } else {
            self.tree.insert(file)?;
        }
        self.owners.insert(path, self.owner.clone());
        Ok(())
    }
    pub fn append(&mut self, tree: GeneratedTree) -> Result<()> {
        for (path, contents) in tree.iter() {
            self.insert(
                GeneratedFile::new(path, contents)?,
                tree.preserves_existing(path),
            )?;
        }
        Ok(())
    }

    /// Adapts a renderer with its own output prefix while retaining create-once files.
    pub fn append_from(&mut self, tree: GeneratedTree, prefix: &Path) -> Result<()> {
        for (path, contents) in tree.iter() {
            let relative = path.strip_prefix(prefix).with_context(|| {
                format!(
                    "renderer output {} is outside {}",
                    path.display(),
                    prefix.display()
                )
            })?;
            self.insert(
                GeneratedFile::new(relative, contents)?,
                tree.preserves_existing(path),
            )?;
        }
        Ok(())
    }
}

pub struct PluginContext<'a, L: Language> {
    pub api: &'a Api,
    pub semantics: &'a SdkSemantics,
    pub security_schemes: Option<&'a SecuritySchemeCatalog>,
    pub common: &'a Common,
    pub settings: &'a L::Settings,
    pub inputs: Inputs<'a>,
    pub workspace: &'a mut L::Workspace,
    pub files: Emitter<'a>,
    publications: &'a mut HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    declared: &'a [Provision],
}
impl<L: Language> PluginContext<'_, L> {
    pub fn publish<C: Contract>(&mut self, value: C) -> Result<()> {
        let key = TypeId::of::<C>();
        if !self.declared.iter().any(|p| p.type_id == key) {
            bail!("undeclared contract publication: {}", C::NAME);
        }
        if self.publications.contains_key(&key) {
            bail!("{} published twice", C::NAME);
        }
        self.publications.insert(key, Box::new(value));
        Ok(())
    }
}

pub struct FinalizeContext<'a, L: Language> {
    pub api: &'a Api,
    pub common: &'a Common,
    pub settings: &'a L::Settings,
    pub workspace: &'a mut L::Workspace,
    pub files: Emitter<'a>,
}

pub struct Package<L: Language> {
    dir: String,
    settings: L::Settings,
    common: Common,
    plugins: Vec<Box<dyn Plugin<L>>>,
}

impl<L: Language> Package<L> {
    pub fn new(dir: impl Into<String>) -> Self {
        Self {
            dir: dir.into(),
            settings: Default::default(),
            common: Default::default(),
            plugins: vec![],
        }
    }
    pub fn with(mut self, plugin: impl Plugin<L>) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }
    pub fn common(mut self, common: Common) -> Self {
        self.common = common;
        self
    }
    pub fn settings(mut self, settings: L::Settings) -> Self {
        self.settings = settings;
        self
    }
    pub fn settings_mut(&mut self) -> &mut L::Settings {
        &mut self.settings
    }
}

struct Plan {
    order: Vec<usize>,
    bindings: Vec<Bindings>,
    provisions: Vec<Vec<Provision>>,
}

impl<L: Language> Package<L> {
    fn label(&self, index: usize) -> String {
        let plugin = &self.plugins[index];
        format!(
            "{} ({})",
            plugin.kind(),
            plugin.meta().label.as_deref().unwrap_or(&self.dir)
        )
    }

    fn resolve(&self) -> Result<Plan> {
        checked_path(Path::new(&self.dir))?;
        let mut instances = BTreeMap::new();
        let mut providers: HashMap<TypeId, Vec<usize>> = HashMap::new();
        let mut provisions = Vec::new();
        for (index, plugin) in self.plugins.iter().enumerate() {
            if instances.insert(plugin.meta().id, index).is_some() {
                bail!("duplicate plugin instance: {}", self.label(index));
            }
            let supplied = plugin.provides();
            let mut seen = BTreeSet::new();
            for contract in &supplied {
                if !seen.insert(contract.type_id) {
                    bail!("{} declares {} twice", self.label(index), contract.name);
                }
                providers.entry(contract.type_id).or_default().push(index);
            }
            provisions.push(supplied);
        }
        let mut dependencies = vec![BTreeSet::new(); self.plugins.len()];
        let mut bindings = Vec::new();
        for (index, plugin) in self.plugins.iter().enumerate() {
            let mut bound = HashMap::new();
            for req in plugin.requires() {
                let candidates = providers
                    .get(&req.contract.type_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let selected = if let Some(id) = req.provider {
                    let selected = *instances.get(&id).with_context(|| {
                        format!(
                            "{}: handle for {} is not registered in this package",
                            self.label(index),
                            req.contract.name
                        )
                    })?;
                    if !candidates.contains(&selected) {
                        bail!(
                            "{}: selected provider does not provide {}",
                            self.label(index),
                            req.contract.name
                        );
                    }
                    Some(selected)
                } else {
                    match candidates {
                        [] if req.optional => None,
                        [] => bail!(
                            "{} requires {}, but no provider is registered",
                            self.label(index),
                            req.contract.name
                        ),
                        [one] => Some(*one),
                        many => bail!(
                            "{} needs one {} provider; found {}: {}. Select a provider handle explicitly",
                            self.label(index),
                            req.contract.name,
                            many.len(),
                            many.iter()
                                .map(|i| self.label(*i))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    }
                };
                if bound
                    .insert(
                        req.contract.type_id,
                        selected.map(|i| self.plugins[i].meta().id),
                    )
                    .is_some()
                {
                    bail!("{} requires {} twice", self.label(index), req.contract.name);
                }
                if let Some(selected) = selected {
                    dependencies[index].insert(selected);
                }
            }
            bindings.push(bound);
        }
        let mut order = Vec::new();
        let mut completed = BTreeSet::new();
        while order.len() < self.plugins.len() {
            let next = (0..self.plugins.len())
                .find(|i| !completed.contains(i) && dependencies[*i].is_subset(&completed));
            let Some(next) = next else {
                bail!(
                    "plugin dependency cycle in {}: {}",
                    self.dir,
                    (0..self.plugins.len())
                        .filter(|i| !completed.contains(i))
                        .map(|i| self.label(i))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            };
            completed.insert(next);
            order.push(next);
        }
        Ok(Plan {
            order,
            bindings,
            provisions,
        })
    }

    fn run(
        &self,
        api: &Api,
        common: &Common,
        catalog: Option<&SecuritySchemeCatalog>,
    ) -> Result<GeneratedTree> {
        let plan = self.resolve()?;
        let common = common.overlay(&self.common);
        let mut api = api.clone();
        if let Some(version) = &common.package_version {
            api.version.clone_from(version);
        }
        let semantics = analyze_sdk_semantics(&api, catalog);
        let mut workspace = L::Workspace::default();
        let mut tree = GeneratedTree::default();
        let mut owners = BTreeMap::new();
        let mut contracts = Contracts::new();
        for index in plan.order {
            let plugin = &self.plugins[index];
            let mut publications = HashMap::new();
            let mut cx = PluginContext {
                api: &api,
                semantics: &semantics,
                security_schemes: catalog,
                common: &common,
                settings: &self.settings,
                inputs: Inputs {
                    contracts: &contracts,
                    bindings: &plan.bindings[index],
                },
                workspace: &mut workspace,
                files: Emitter {
                    tree: &mut tree,
                    owners: &mut owners,
                    owner: self.label(index),
                },
                publications: &mut publications,
                declared: &plan.provisions[index],
            };
            plugin
                .generate(&mut cx)
                .with_context(|| format!("generate {}", self.label(index)))?;
            for provision in &plan.provisions[index] {
                if !publications.contains_key(&provision.type_id) {
                    bail!(
                        "{} did not publish declared contract {}",
                        self.label(index),
                        provision.name
                    );
                }
            }
            for (key, value) in publications {
                contracts.insert((plugin.meta().id, key), value);
            }
        }
        L::finalize(&mut FinalizeContext {
            api: &api,
            common: &common,
            settings: &self.settings,
            workspace: &mut workspace,
            files: Emitter {
                tree: &mut tree,
                owners: &mut owners,
                owner: format!("{} finalizer", L::NAME),
            },
        })?;
        let mut output = GeneratedTree::default();
        let dir = checked_path(Path::new(&self.dir))?;
        for (path, contents) in tree.iter() {
            let file = GeneratedFile::new(dir.join(path), contents)?;
            if tree.preserves_existing(path) {
                output.insert_custom(file)?;
            } else {
                output.insert(file)?;
            }
        }
        Ok(output)
    }
}

/// Type erasure only at the release boundary; package builders remain typed.
trait ErasedPackage: Send + Sync {
    fn directory(&self) -> &str;
    fn validate(&self) -> Result<()>;
    fn generate(
        &self,
        api: &Api,
        common: &Common,
        catalog: Option<&SecuritySchemeCatalog>,
    ) -> Result<GeneratedTree>;
}
impl<L: Language> ErasedPackage for Package<L> {
    fn directory(&self) -> &str {
        &self.dir
    }
    fn validate(&self) -> Result<()> {
        self.resolve().map(|_| ())
    }
    fn generate(
        &self,
        api: &Api,
        common: &Common,
        catalog: Option<&SecuritySchemeCatalog>,
    ) -> Result<GeneratedTree> {
        self.run(api, common, catalog)
    }
}

#[derive(Default)]
pub struct Packages {
    packages: Vec<Box<dyn ErasedPackage>>,
    common: Common,
}
impl Packages {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn common(mut self, common: Common) -> Self {
        self.common = common;
        self
    }
    pub fn package<L: Language>(mut self, package: Package<L>) -> Self {
        self.packages.push(Box::new(package));
        self
    }
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
    pub fn generate(
        &self,
        api: &Api,
        catalog: Option<&SecuritySchemeCatalog>,
    ) -> Result<GeneratedTree> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        // Validate every package before executing any generator.
        for package in &self.packages {
            let dir = checked_path(Path::new(package.directory()))?;
            if let Some(other) = dirs
                .iter()
                .find(|other| dir.starts_with(other) || other.starts_with(&dir))
            {
                bail!(
                    "package directories overlap: {} and {}",
                    other.display(),
                    dir.display()
                );
            }
            dirs.push(dir);
            package.validate()?;
        }
        let mut output = GeneratedTree::default();
        for package in &self.packages {
            output.append(package.generate(api, &self.common, catalog)?)?;
        }
        Ok(output)
    }
}

fn checked_path(path: &Path) -> Result<PathBuf> {
    GeneratedFile::new(path, "")?;
    let normalized: PathBuf = path
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect();
    if normalized.as_os_str().is_empty() || normalized == Path::new(".") {
        bail!("output path cannot be empty or '.'");
    }
    Ok(normalized)
}
