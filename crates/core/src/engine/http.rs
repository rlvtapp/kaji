//! Explicit typed HTTP selection, with a compatibility bridge for legacy calls.
use super::*;
use crate::{
    AdaptedApi, Operation, Schema,
    blocks::{Block, Blocks},
};

#[derive(Clone, Default)]
pub struct HttpInput {
    whole: Option<Handle<AdaptedApi>>,
    models: Option<Handle<Blocks<Schema>>>,
    endpoints: Option<Handle<Blocks<Operation>>>,
}
/// Owned renderer view; selected contracts never borrow the output emitter.
pub struct HttpView {
    pub api: Api,
    pub semantics: SdkSemantics,
    pub security_schemes: Option<SecuritySchemeCatalog>,
}
impl HttpInput {
    pub fn input(mut self, whole: Handle<AdaptedApi>) -> Self {
        self.whole = Some(whole);
        self
    }
    pub fn input_models(mut self, models: Handle<Blocks<Schema>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn input_endpoints(mut self, endpoints: Handle<Blocks<Operation>>) -> Self {
        self.endpoints = Some(endpoints);
        self
    }
    /// HTTP output may opt into native generation only when explicitly selected.
    pub fn is_explicit(&self) -> bool {
        self.whole.is_some() || self.models.is_some() || self.endpoints.is_some()
    }
    pub fn requirements(&self) -> Vec<Requirement> {
        let whole = Requirement::on(self.whole);
        let mut requirements = vec![if self.is_explicit() {
            whole
        } else {
            whole.optional()
        }];
        if let Some(models) = self.models {
            requirements.push(Requirement::on(Some(models)));
        }
        if let Some(endpoints) = self.endpoints {
            requirements.push(Requirement::on(Some(endpoints)));
        }
        requirements
    }
    pub fn resolve<L: Language>(&self, cx: &PluginContext<'_, L>) -> Result<HttpView> {
        let selected = cx.inputs.optional::<AdaptedApi>()?;
        let Some(selected) = selected else {
            anyhow::ensure!(
                !self.is_explicit(),
                "explicit HTTP input requires a selected whole AdaptedApi contract"
            );
            return Ok(HttpView {
                api: cx.api.clone(),
                semantics: cx.semantics.clone(),
                security_schemes: cx.security_schemes.cloned(),
            });
        };
        let mut api = selected.api.clone();
        if self.models.is_some() {
            let blocks = cx.inputs.get::<Blocks<Schema>>()?;
            check_blocks(blocks, cx.inputs.reference::<AdaptedApi>()?)?;
            api.schemas = blocks.items.iter().map(|item| item.value.clone()).collect();
        }
        if self.endpoints.is_some() {
            let blocks = cx.inputs.get::<Blocks<Operation>>()?;
            check_blocks(blocks, cx.inputs.reference::<AdaptedApi>()?)?;
            api.operations = blocks.items.iter().map(|item| item.value.clone()).collect();
        }
        if let Some(version) = &cx.common.package_version {
            api.version.clone_from(version);
        }
        let api = crate::idempotency::prepare_api(&api, cx.idempotency)?;
        let catalog = selected.security_schemes.clone();
        let semantics = analyze_sdk_semantics(&api, Some(&catalog));
        Ok(HttpView {
            api,
            semantics,
            security_schemes: Some(catalog),
        })
    }
    /// Render with the selected HTTP API, semantics and security context.
    pub fn run<L: Language, T>(
        &self,
        cx: &mut PluginContext<'_, L>,
        render: impl FnOnce(&mut PluginContext<'_, L>) -> Result<T>,
    ) -> Result<T> {
        self.with_context(cx, render)
    }

    /// Rebind an existing renderer without changing its legacy public context API.
    pub fn with_context<L: Language, T>(
        &self,
        cx: &mut PluginContext<'_, L>,
        render: impl FnOnce(&mut PluginContext<'_, L>) -> Result<T>,
    ) -> Result<T> {
        let view = self.resolve(cx)?;
        let mut selected = PluginContext {
            api: &view.api,
            semantics: &view.semantics,
            security_schemes: view.security_schemes.as_ref(),
            common: cx.common,
            settings: cx.settings,
            inputs: Inputs {
                contracts: cx.inputs.contracts,
                references: cx.inputs.references,
                bindings: cx.inputs.bindings,
            },
            workspace: &mut *cx.workspace,
            files: Emitter {
                tree: &mut *cx.files.tree,
                owners: &mut *cx.files.owners,
                owner: cx.files.owner.clone(),
            },
            publications: &mut *cx.publications,
            publication_references: &mut *cx.publication_references,
            declared: cx.declared,
            idempotency: cx.idempotency,
        };
        render(&mut selected)
    }
}
fn check_blocks<T: Block>(
    blocks: &Blocks<T>,
    parent: Option<&crate::blocks::ContractReference>,
) -> Result<()> {
    let parent = parent.context(
        "selected HTTP blocks require authoritative provenance on the selected whole contract",
    )?;
    blocks.validate()?;
    blocks.require_complete()?;
    blocks.require_parent(parent)
}

// Keep legacy consumers' existing/default rules, while validating rules for new
// transformed operations against the selected contract when the renderer runs.
pub(super) fn prepare_legacy(
    api: &Api,
    config: &crate::idempotency::IdempotencyConfig,
    bindings: &[Bindings],
) -> Result<Api> {
    let typed_http = bindings.iter().any(|bound| {
        bound
            .get(&TypeId::of::<AdaptedApi>())
            .is_some_and(Option::is_some)
    });
    if !typed_http {
        return crate::idempotency::prepare_api(api, config);
    }
    let mut legacy = config.clone();
    legacy
        .operations
        .retain(|id, _| api.operations.iter().any(|operation| &operation.id == id));
    crate::idempotency::prepare_api(api, &legacy)
}
