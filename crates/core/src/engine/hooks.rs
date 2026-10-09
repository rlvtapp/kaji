//! Typed lifecycle handlers; contracts do not need to expose building blocks.
use super::*;
use crate::blocks::{Block, Blocks, BuildingBlock};

/// Mutable output surface for a handler. Its input is passed separately.
pub struct HookContext<'a, 'b, L: Language> {
    pub input_reference: Option<&'a crate::blocks::ContractReference>,
    pub common: &'a Common,
    pub settings: &'a L::Settings,
    pub workspace: &'a mut L::Workspace,
    pub files: &'a mut Emitter<'b>,
    publications: &'a mut HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    publication_references: &'a mut HashMap<TypeId, crate::blocks::ContractReference>,
    declared: &'a [Provision],
}
impl<L: Language> HookContext<'_, '_, L> {
    pub fn publish_with_reference<C: Contract>(
        &mut self,
        value: C,
        reference: crate::blocks::ContractReference,
    ) -> Result<()> {
        anyhow::ensure!(
            reference.contract == C::NAME,
            "publication reference contract does not match {}",
            C::NAME
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
type Callback<L> = Box<dyn Fn(&mut PluginContext<'_, L>) -> Result<()> + Send + Sync>;

/// One plugin can register handlers for several contract and block types.
pub struct Hooks<L: Language> {
    meta: Meta,
    phase: PluginPhase,
    requirements: Vec<Requirement>,
    provisions: Vec<Provision>,
    callbacks: Vec<Callback<L>>,
}
pub fn hooks<L: Language>() -> Hooks<L> {
    Hooks {
        meta: Meta::new(),
        phase: PluginPhase::Generate,
        requirements: vec![],
        provisions: vec![],
        callbacks: vec![],
    }
}
impl<L: Language> Hooks<L> {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.meta = self.meta.label(label);
        self
    }
    pub fn phase(mut self, phase: PluginPhase) -> Self {
        self.phase = phase;
        self
    }
    pub fn provides<C: Contract>(mut self) -> Self {
        self.provisions.push(Provision::of::<C>());
        self
    }
    pub fn handle<C: Contract>(&self) -> Handle<C> {
        self.meta.handle()
    }
    /// Required handlers use normal graph ordering and provider ambiguity checks.
    pub fn on_contract<C: Contract>(
        mut self,
        provider: Option<Handle<C>>,
        handler: impl Fn(&C, &mut HookContext<'_, '_, L>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        self.requirements.push(Requirement::on(provider));
        self.callbacks.push(Box::new(move |cx| {
            let input = cx.inputs.get::<C>()?;
            let mut hook = HookContext {
                input_reference: cx.inputs.reference::<C>()?,
                common: cx.common,
                settings: cx.settings,
                workspace: cx.workspace,
                files: &mut cx.files,
                publications: cx.publications,
                publication_references: cx.publication_references,
                declared: cx.declared,
            };
            handler(input, &mut hook)
        }));
        self
    }
    /// Optional automatic absence skips this handler; explicit missing providers
    /// and ambiguous bindings still fail through the existing graph rules.
    pub fn on_optional_contract<C: Contract>(
        mut self,
        provider: Option<Handle<C>>,
        handler: impl Fn(&C, &mut HookContext<'_, '_, L>) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        self.requirements.push(Requirement::on(provider).optional());
        self.callbacks.push(Box::new(move |cx| {
            if let Some(input) = cx.inputs.optional::<C>()? {
                let mut hook = HookContext {
                    input_reference: cx.inputs.reference::<C>()?,
                    common: cx.common,
                    settings: cx.settings,
                    workspace: cx.workspace,
                    files: &mut cx.files,
                    publications: cx.publications,
                    publication_references: cx.publication_references,
                    declared: cx.declared,
                };
                handler(input, &mut hook)?;
            }
            Ok(())
        }));
        self
    }
    /// Opt-in extraction binds precisely to the selected whole-contract provider.
    pub fn extract_blocks<C: Contract, T: Block>(
        self,
        provider: Option<Handle<C>>,
        extract: impl Fn(&C) -> Result<Blocks<T>> + Send + Sync + 'static,
    ) -> Self {
        self.provides::<Blocks<T>>()
            .on_contract(provider, move |whole, cx| {
                let parent = cx
                    .input_reference
                    .cloned()
                    .context("block extraction requires parent contract instance and revision")?;
                let blocks = extract(whole)?.with_parent(parent);
                blocks.validate()?;
                cx.publish(blocks)
            })
    }
    /// A consumer of both views validates the chosen authoritative whole revision.
    pub fn on_derived_block<C: Contract, T: Block>(
        mut self,
        whole: Option<Handle<C>>,
        blocks: Option<Handle<Blocks<T>>>,
        handler: impl Fn(&BuildingBlock<T>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.requirements.push(Requirement::on(whole));
        self.requirements.push(Requirement::on(blocks));
        self.callbacks.push(Box::new(move |cx| {
            let parent = cx
                .inputs
                .reference::<C>()?
                .context("selected whole contract has no revision metadata")?;
            let blocks = cx.inputs.get::<Blocks<T>>()?;
            blocks.validate()?;
            blocks.require_complete()?;
            blocks.require_parent(parent)?;
            let mut hook = HookContext {
                input_reference: Some(parent),
                common: cx.common,
                settings: cx.settings,
                workspace: cx.workspace,
                files: &mut cx.files,
                publications: cx.publications,
                publication_references: cx.publication_references,
                declared: cx.declared,
            };
            for block in &blocks.items {
                handler(block, &mut hook)?;
            }
            Ok(())
        }));
        self
    }
    /// Explicit opt-in for diagnostic/partial-data consumers.
    pub fn on_partial_block<T: Block>(
        self,
        provider: Option<Handle<Blocks<T>>>,
        handler: impl Fn(&BuildingBlock<T>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_contract(provider, move |blocks: &Blocks<T>, cx| {
            blocks.validate()?;
            anyhow::ensure!(
                !matches!(
                    blocks.state,
                    crate::blocks::CollectionState::Unavailable { .. }
                ),
                "block collection unavailable: {:?}",
                blocks.state
            );
            for block in &blocks.items {
                handler(block, cx)?;
            }
            Ok(())
        })
    }
    pub fn on_nonempty_block<T: Block>(
        self,
        provider: Option<Handle<Blocks<T>>>,
        handler: impl Fn(&BuildingBlock<T>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_contract(provider, move |blocks: &Blocks<T>, cx| {
            blocks.validate()?;
            blocks.require_complete()?;
            blocks.require_nonempty()?;
            for block in &blocks.items {
                handler(block, cx)?;
            }
            Ok(())
        })
    }
    pub fn on_block<T: Block>(
        self,
        provider: Option<Handle<Blocks<T>>>,
        handler: impl Fn(&BuildingBlock<T>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_contract(provider, move |blocks: &Blocks<T>, cx| {
            blocks.validate()?;
            blocks.require_complete()?;
            for block in &blocks.items {
                handler(block, cx)?;
            }
            Ok(())
        })
    }
}
impl<L: Language> Hooks<L> {
    pub fn on_model(
        self,
        provider: Option<Handle<Blocks<crate::Schema>>>,
        handler: impl Fn(&BuildingBlock<crate::Schema>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
    pub fn on_endpoint(
        self,
        provider: Option<Handle<Blocks<crate::Operation>>>,
        handler: impl Fn(&BuildingBlock<crate::Operation>, &mut HookContext<'_, '_, L>) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
    pub fn on_model_shape(
        self,
        provider: Option<Handle<Blocks<crate::blocks::ModelBlock>>>,
        handler: impl Fn(
            &BuildingBlock<crate::blocks::ModelBlock>,
            &mut HookContext<'_, '_, L>,
        ) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
    pub fn on_message(
        self,
        provider: Option<Handle<Blocks<crate::native::events::EventMessage>>>,
        handler: impl Fn(
            &BuildingBlock<crate::native::events::EventMessage>,
            &mut HookContext<'_, '_, L>,
        ) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
    pub fn on_rpc_method(
        self,
        provider: Option<Handle<Blocks<crate::native::rpc::RpcMethodBlock>>>,
        handler: impl Fn(
            &BuildingBlock<crate::native::rpc::RpcMethodBlock>,
            &mut HookContext<'_, '_, L>,
        ) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
    pub fn on_workflow_step(
        self,
        provider: Option<Handle<Blocks<crate::native::workflows::WorkflowStepBlock>>>,
        handler: impl Fn(
            &BuildingBlock<crate::native::workflows::WorkflowStepBlock>,
            &mut HookContext<'_, '_, L>,
        ) -> Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_block(provider, handler)
    }
}

impl<L: Language> Plugin<L> for Hooks<L> {
    fn kind(&self) -> &'static str {
        "typed-hooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn phase(&self) -> PluginPhase {
        self.phase
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements: Vec<Requirement> = Vec::new();
        for r in &self.requirements {
            if let Some(existing) = requirements.iter_mut().find(|existing| {
                existing.contract.type_id == r.contract.type_id && existing.provider == r.provider
            }) {
                existing.optional &= r.optional;
            } else {
                requirements.push(Requirement {
                    contract: r.contract,
                    provider: r.provider,
                    optional: r.optional,
                });
            }
        }
        requirements
    }
    fn provides(&self) -> Vec<Provision> {
        self.provisions.clone()
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        for callback in &self.callbacks {
            callback(cx)?;
        }
        Ok(())
    }
}
