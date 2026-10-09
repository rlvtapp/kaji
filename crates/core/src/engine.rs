//! Typed package composition for native and community generators.
//!
//! Contracts are package-local, immutable once published, and looked up by
//! Rust type. Language implementations own package settings and shared state.

mod context;
use context::{Bindings, ContractReferences, Contracts, checked_path};
pub use context::{Emitter, FinalizeContext, Inputs, PluginContext};

mod http;
pub use http::{HttpInput, HttpView};

pub mod hooks;
pub use hooks::{HookContext, Hooks, hooks};

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
    pub layout: Option<crate::SourceLayout>,
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
    pub fn layout(mut self, layout: crate::SourceLayout) -> Self {
        self.layout = Some(layout);
        self
    }
    pub fn overlay(&self, local: &Self) -> Self {
        Self {
            client_name: local
                .client_name
                .clone()
                .or_else(|| self.client_name.clone()),
            client_style: local.client_style.or(self.client_style),
            layout: local.layout.clone().or_else(|| self.layout.clone()),
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
    /// Adapt assembled generated files for the target runtime after middleware
    /// bundling and before explicit author source overlays. Default is unchanged.
    fn finalize_files(_tree: &mut GeneratedTree) -> Result<()> {
        Ok(())
    }
    /// Bundle SDK-author runtime middleware and register it in generated clients.
    /// Called with package-relative output after finalization and post plugins.
    fn bundle_middleware(
        _tree: &mut GeneratedTree,
        _middleware: &[crate::customization::BundledMiddleware],
    ) -> Result<()> {
        bail!("{} does not support bundled runtime middleware", Self::NAME)
    }
}

/// A plugin's point in the package generation lifecycle.
///
/// Generation plugins run first. Post plugins run after every generation
/// plugin and the language finalizer, so they can add derived artifacts after
/// a package's normal output has been assembled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PluginPhase {
    #[default]
    Generate,
    Post,
}

/// Shorthand for selecting a plugin execution phase.
///
/// [`Enforce::Post`] is intended for plugins that consume or augment the
/// completed generated package. Override [`Plugin::phase`] when selecting a
/// phase dynamically is more appropriate than this shorthand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Enforce {
    #[default]
    Default,
    Post,
}

impl Enforce {
    pub const fn phase(self) -> PluginPhase {
        match self {
            Self::Default => PluginPhase::Generate,
            Self::Post => PluginPhase::Post,
        }
    }
}

pub trait Plugin<L: Language>: Send + Sync + 'static {
    fn kind(&self) -> &'static str;
    fn meta(&self) -> &Meta;
    /// Selects the plugin lifecycle phase. The default honors [`Self::enforce`]
    /// so a post plugin only needs to override that shorthand.
    fn phase(&self) -> PluginPhase {
        self.enforce().phase()
    }
    /// Phase shorthand. Existing plugins remain generation plugins.
    fn enforce(&self) -> Enforce {
        Enforce::Default
    }
    /// Opt in when this plugin reads protocol contracts instead of the legacy HTTP context.
    /// Native generation skips packages containing HTTP-only plugins before loading sources.
    fn supports_native_input(&self) -> bool {
        false
    }
    fn requires(&self) -> Vec<Requirement> {
        Vec::new()
    }
    fn provides(&self) -> Vec<Provision> {
        Vec::new()
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()>;
}

pub struct Package<L: Language> {
    dir: String,
    settings: L::Settings,
    common: Common,
    plugins: Vec<Box<dyn Plugin<L>>>,
    customizations: Vec<crate::customization::CodeCustomization>,
    middleware: Vec<crate::customization::BundledMiddleware>,
    idempotency: crate::idempotency::IdempotencyConfig,
}

impl<L: Language> Package<L> {
    pub fn new(dir: impl Into<String>) -> Self {
        Self {
            dir: dir.into(),
            settings: Default::default(),
            common: Default::default(),
            plugins: vec![],
            customizations: vec![],
            middleware: vec![],
            idempotency: Default::default(),
        }
    }
    pub fn with(mut self, plugin: impl Plugin<L>) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }
    /// Apply an explicit SDK-author source overlay after all plugins/finalizers.
    pub fn customize(mut self, code: crate::customization::CodeCustomization) -> Self {
        self.customizations.push(code);
        self
    }
    /// Ship SDK-author runtime middleware enabled by default in this package.
    pub fn middleware(mut self, middleware: crate::customization::BundledMiddleware) -> Self {
        self.middleware.push(middleware);
        self
    }
    /// Resolve explicit idempotency policy independently for this package.
    pub fn idempotency(mut self, config: crate::idempotency::IdempotencyConfig) -> Self {
        self.idempotency = config;
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
    phases: Vec<PluginPhase>,
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
        let mut middleware_paths = BTreeSet::new();
        for middleware in &self.middleware {
            middleware.validate()?;
            if !middleware_paths.insert(checked_path(&middleware.path)?) {
                bail!(
                    "duplicate bundled middleware path in {}: {}",
                    self.dir,
                    middleware.path.display()
                );
            }
        }
        let mut instances = BTreeMap::new();
        let mut providers: HashMap<TypeId, Vec<usize>> = HashMap::new();
        let mut phases = Vec::new();
        let mut provisions = Vec::new();
        for (index, plugin) in self.plugins.iter().enumerate() {
            if instances.insert(plugin.meta().id, index).is_some() {
                bail!("duplicate plugin instance: {}", self.label(index));
            }
            phases.push(plugin.phase());
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
        // Stable names are protocol identities, so distinct Rust payload types
        // sharing one name are incompatible even for optional consumers.
        let requirements: Vec<_> = self
            .plugins
            .iter()
            .map(|plugin| plugin.requires())
            .collect();
        let mut identities = HashMap::new();
        for (index, contracts) in provisions.iter().enumerate() {
            for contract in contracts
                .iter()
                .chain(requirements[index].iter().map(|req| &req.contract))
            {
                if let Some((type_id, owner)) = identities.get(contract.name) {
                    if *type_id != contract.type_id {
                        bail!(
                            "contract compatibility error for {}: {} and {} use different Rust types for the same stable contract name",
                            contract.name,
                            owner,
                            self.label(index)
                        );
                    }
                } else {
                    identities.insert(contract.name, (contract.type_id, self.label(index)));
                }
            }
        }
        let mut dependencies = vec![BTreeSet::new(); self.plugins.len()];
        let mut bindings = Vec::new();
        for (index, _) in self.plugins.iter().enumerate() {
            let mut bound = HashMap::new();
            for req in &requirements[index] {
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
                    if phases[index] == PluginPhase::Generate
                        && phases[selected] == PluginPhase::Post
                    {
                        bail!(
                            "{} requires {} from {}, but generation plugins cannot depend on post plugins",
                            self.label(index),
                            req.contract.name,
                            self.label(selected),
                        );
                    }
                    dependencies[index].insert(selected);
                }
            }
            bindings.push(bound);
        }
        // A post plugin observes a completed package. Make the phase boundary
        // explicit in the dependency graph, while retaining normal contract
        // ordering between post plugins themselves.
        for (index, phase) in phases.iter().enumerate() {
            if *phase == PluginPhase::Post {
                dependencies[index].extend(phases.iter().enumerate().filter_map(
                    |(other, phase)| (*phase == PluginPhase::Generate).then_some(other),
                ));
            }
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
            phases,
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
        let overridden = common.package_version.as_ref().map(|version| {
            let mut copy = api.clone();
            copy.version.clone_from(version);
            copy
        });
        let api = overridden.as_ref().unwrap_or(api);
        let prepared = http::prepare_legacy(api, &self.idempotency, &plan.bindings)?;
        let api = &prepared;
        let semantics = analyze_sdk_semantics(api, catalog);
        let mut workspace = L::Workspace::default();
        let mut tree = GeneratedTree::default();
        let mut owners = BTreeMap::new();
        let mut contracts = Contracts::new();
        let mut references = ContractReferences::new();
        for phase in [PluginPhase::Generate, PluginPhase::Post] {
            for &index in &plan.order {
                if plan.phases[index] != phase {
                    continue;
                }
                let plugin = &self.plugins[index];
                let mut publications = HashMap::new();
                let mut publication_references = HashMap::new();
                let mut cx = PluginContext {
                    api,
                    semantics: &semantics,
                    security_schemes: catalog,
                    common: &common,
                    settings: &self.settings,
                    inputs: Inputs {
                        contracts: &contracts,
                        references: &references,
                        bindings: &plan.bindings[index],
                    },
                    workspace: &mut workspace,
                    files: Emitter {
                        tree: &mut tree,
                        owners: &mut owners,
                        owner: self.label(index),
                    },
                    publications: &mut publications,
                    publication_references: &mut publication_references,
                    declared: &plan.provisions[index],
                    idempotency: &self.idempotency,
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
                for (key, reference) in publication_references {
                    references.insert((plugin.meta().id, key), reference);
                }
                for (key, value) in publications {
                    contracts.insert((plugin.meta().id, key), value);
                }
            }
            if phase == PluginPhase::Generate {
                L::finalize(&mut FinalizeContext {
                    api,
                    common: &common,
                    settings: &self.settings,
                    workspace: &mut workspace,
                    files: Emitter {
                        tree: &mut tree,
                        owners: &mut owners,
                        owner: format!("{} finalizer", L::NAME),
                    },
                })?;
            }
        }
        if !self.middleware.is_empty() {
            let mut staged = tree.clone();
            let middleware = self
                .middleware
                .iter()
                .map(|item| {
                    let mut item = item.clone();
                    item.path = checked_path(&item.path)?;
                    Ok(item)
                })
                .collect::<Result<Vec<_>>>()?;
            L::bundle_middleware(&mut staged, &middleware).with_context(|| {
                format!("bundle runtime middleware for {} ({})", self.dir, L::NAME)
            })?;
            for item in &middleware {
                staged.set_owner(
                    &item.path,
                    format!("bundled-middleware:{}", item.path.display()),
                )?;
            }
            tree = staged;
        }
        L::finalize_files(&mut tree)?;
        crate::customization::apply_code_customizations(&mut tree, &self.customizations)?;
        let mut output = GeneratedTree::default();
        let dir = checked_path(Path::new(&self.dir))?;
        for (file, custom, owner) in tree.into_owned_files() {
            let file = GeneratedFile::new(dir.join(file.path), file.contents)?;
            let path = file.path.clone();
            if custom {
                output.insert_custom(file)?;
            } else {
                output.insert(file)?;
            }
            if let Some(owner) = owner {
                output.set_owner(path, format!("{}::{owner}", self.dir))?;
            }
        }
        Ok(output)
    }
}

/// Type erasure only at the release boundary; package builders remain typed.
trait ErasedPackage: Send + Sync {
    fn directory(&self) -> &str;
    fn validate(&self) -> Result<()>;
    fn native_incompatibility(&self) -> Option<String>;
    fn validate_native(&self) -> Result<()>;
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
    fn native_incompatibility(&self) -> Option<String> {
        let plugins: Vec<_> = self
            .plugins
            .iter()
            .enumerate()
            .filter(|(_, plugin)| !plugin.supports_native_input())
            .map(|(index, _)| self.label(index))
            .collect();
        if plugins.is_empty() {
            None
        } else {
            Some(format!(
                "{} requires an HTTP API and cannot consume native input contracts",
                plugins.join(", ")
            ))
        }
    }
    fn validate_native(&self) -> Result<()> {
        if !self.middleware.is_empty()
            || (self.idempotency.defaults.is_some() || !self.idempotency.operations.is_empty())
        {
            bail!(
                "native package {} cannot use HTTP middleware or idempotency policy",
                self.dir
            );
        }
        self.validate()
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

/// An incompatible native package is skipped without running its input provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSkip {
    pub package: String,
    pub reason: String,
}
#[derive(Debug)]
pub struct NativeGeneration {
    pub tree: GeneratedTree,
    pub skipped: Vec<NativeSkip>,
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
    /// Generate from explicitly declared native protocol contracts.
    /// The empty legacy HTTP context preserves the plugin ABI; native operations
    /// are carried only by typed requirements and are never mapped into `Api`.
    pub fn generate_native(&self) -> Result<GeneratedTree> {
        let report = self.generate_native_report()?;
        for skipped in &report.skipped {
            eprintln!(
                "warning: skipped native package {}: {}",
                skipped.package, skipped.reason
            );
        }
        Ok(report.tree)
    }
    /// A structured report lets callers surface skips without treating them as errors.
    /// An empty tree must not replace existing generated output when every package skips.
    pub fn generate_native_report(&self) -> Result<NativeGeneration> {
        let mut skipped = Vec::new();
        let mut active = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();
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
            if let Some(reason) = package.native_incompatibility() {
                skipped.push(NativeSkip {
                    package: package.directory().to_owned(),
                    reason,
                });
            } else {
                package.validate_native()?;
                active.push(package);
            }
        }
        let mut tree = GeneratedTree::default();
        for package in active {
            tree.append(package.generate(&Api::default(), &self.common, None)?)?;
        }
        Ok(NativeGeneration { tree, skipped })
    }
    pub fn generate(
        &self,
        api: &Api,
        catalog: Option<&SecuritySchemeCatalog>,
    ) -> Result<GeneratedTree> {
        for operation in &api.operations {
            let parsed =
                crate::HttpMethod::parse(operation.method.as_str()).map_err(anyhow::Error::msg)?;
            if parsed != operation.method {
                bail!("HTTP method variants must use their canonical representation");
            }
        }
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
