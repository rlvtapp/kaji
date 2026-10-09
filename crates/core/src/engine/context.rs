//! Typed contract reads, publication, and package output context.
use super::*;

pub(super) type Contracts = HashMap<(InstanceId, TypeId), Box<dyn Any + Send + Sync>>;
pub(super) type Bindings = HashMap<TypeId, Option<InstanceId>>;
pub(super) type ContractReferences =
    HashMap<(InstanceId, TypeId), crate::blocks::ContractReference>;

pub struct Inputs<'a> {
    pub(super) contracts: &'a Contracts,
    pub(super) references: &'a ContractReferences,
    pub(super) bindings: &'a Bindings,
}
impl Inputs<'_> {
    /// Provenance belongs to the selected provider, independently of payload data.
    /// Undeclared reads and missing publications fail as ordinary typed reads do.
    pub fn reference<C: Contract>(&self) -> Result<Option<&crate::blocks::ContractReference>> {
        self.optional::<C>()?;
        Ok(self
            .bindings
            .get(&TypeId::of::<C>())
            .copied()
            .flatten()
            .and_then(|provider| self.references.get(&(provider, TypeId::of::<C>()))))
    }

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
    pub(super) tree: &'a mut GeneratedTree,
    pub(super) owners: &'a mut BTreeMap<PathBuf, String>,
    pub(super) owner: String,
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
        self.tree.set_owner(&path, self.owner.clone())?;
        self.owners.insert(path, self.owner.clone());
        Ok(())
    }
    pub fn append(&mut self, tree: GeneratedTree) -> Result<()> {
        for (file, custom, owner) in tree.into_owned_files() {
            let path = file.path.clone();
            self.insert(file, custom)?;
            if let Some(owner) = owner {
                self.tree.set_owner(path, owner)?;
            }
        }
        Ok(())
    }

    /// Adapts a renderer with its own output prefix while retaining create-once files.
    pub fn append_from(&mut self, tree: GeneratedTree, prefix: &Path) -> Result<()> {
        for (file, custom, owner) in tree.into_owned_files() {
            let relative = file.path.strip_prefix(prefix).with_context(|| {
                format!(
                    "renderer output {} is outside {}",
                    file.path.display(),
                    prefix.display()
                )
            })?;
            let relative = relative.to_owned();
            self.insert(GeneratedFile::new(&relative, file.contents)?, custom)?;
            if let Some(owner) = owner {
                self.tree.set_owner(relative, owner)?;
            }
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
    pub(super) publications: &'a mut HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    pub(super) publication_references: &'a mut HashMap<TypeId, crate::blocks::ContractReference>,
    pub(super) declared: &'a [Provision],
    pub(super) idempotency: &'a crate::idempotency::IdempotencyConfig,
}
impl<L: Language> PluginContext<'_, L> {
    /// Publish a typed payload with independently stored authoritative provenance.
    pub fn publish_with_reference<C: Contract>(
        &mut self,
        value: C,
        reference: crate::blocks::ContractReference,
    ) -> Result<()> {
        anyhow::ensure!(
            reference.contract == C::NAME,
            "provenance contract mismatch: expected {}, found {}",
            C::NAME,
            reference.contract
        );
        reference.validate()?;
        self.publish(value)?;
        self.publication_references
            .insert(TypeId::of::<C>(), reference);
        Ok(())
    }

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

pub(super) fn checked_path(path: &Path) -> Result<PathBuf> {
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
