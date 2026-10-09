use super::*;
use anyhow::Result;
use std::collections::BTreeMap;

/// Immutable reservation plan exchanged through the existing typed plugin graph.
/// Custom collectors may populate this from whole contracts or optional blocks.
#[derive(Clone, Debug, Default)]
pub struct SymbolPlan {
    pub requests: Vec<SymbolRequest>,
    pub forbidden_names: Vec<SymbolNameReservation>,
    pub target_rules: BTreeMap<String, SymbolRules>,
}
#[derive(Clone, Debug)]
pub struct SymbolNameReservation {
    pub target: String,
    pub module: String,
    pub name: String,
}
impl crate::engine::Contract for SymbolPlan {
    const NAME: &'static str = "poolster.symbol-plan.v1";
}
impl SymbolPlan {
    pub fn validate(&self) -> Result<()> {
        self.allocator().map(|_| ())
    }
    fn allocator(&self) -> Result<SymbolRequests> {
        let mut names = SymbolRequests::new();
        for (target, rules) in &self.target_rules {
            names.target_rules(target, *rules)?;
        }
        for reserved in &self.forbidden_names {
            names.reserve_name(&reserved.target, &reserved.module, &reserved.name)?;
        }
        for request in &self.requests {
            names.reserve(request.clone())?;
        }
        Ok(names)
    }
}
/// Opt-in reservation provider. Providers with dynamic inputs can publish SymbolPlan directly.
pub struct ReserveSymbolRequests {
    meta: crate::engine::Meta,
    plan: SymbolPlan,
}
pub fn reserve_symbol_requests(
    requests: impl IntoIterator<Item = SymbolRequest>,
) -> ReserveSymbolRequests {
    ReserveSymbolRequests {
        meta: crate::engine::Meta::new(),
        plan: SymbolPlan {
            requests: requests.into_iter().collect(),
            ..Default::default()
        },
    }
}
impl ReserveSymbolRequests {
    pub fn handle(&self) -> crate::engine::Handle<SymbolPlan> {
        self.meta.handle()
    }
    pub fn reserve_name(
        mut self,
        target: impl Into<String>,
        module: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        self.plan.forbidden_names.push(SymbolNameReservation {
            target: target.into(),
            module: module.into(),
            name: name.into(),
        });
        self
    }
    pub fn target_rules(mut self, target: impl Into<String>, rules: SymbolRules) -> Self {
        self.plan.target_rules.insert(target.into(), rules);
        self
    }
}
impl<L: crate::engine::Language> crate::engine::Plugin<L> for ReserveSymbolRequests {
    fn kind(&self) -> &'static str {
        "symbol-reservations"
    }
    fn meta(&self) -> &crate::engine::Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<crate::engine::Provision> {
        vec![crate::engine::Provision::of::<SymbolPlan>()]
    }
    fn generate(&self, cx: &mut crate::engine::PluginContext<'_, L>) -> Result<()> {
        self.plan.validate()?;
        cx.publish(self.plan.clone())
    }
}
/// Resolves a complete plan before any dependent emitter runs.
pub struct ResolveSymbols {
    meta: crate::engine::Meta,
    provider: Option<crate::engine::Handle<SymbolPlan>>,
}
pub fn resolve_symbol_requests(
    provider: Option<crate::engine::Handle<SymbolPlan>>,
) -> ResolveSymbols {
    ResolveSymbols {
        meta: crate::engine::Meta::new(),
        provider,
    }
}
impl ResolveSymbols {
    pub fn handle(&self) -> crate::engine::Handle<ResolvedSymbols> {
        self.meta.handle()
    }
}
impl<L: crate::engine::Language> crate::engine::Plugin<L> for ResolveSymbols {
    fn kind(&self) -> &'static str {
        "symbol-resolution"
    }
    fn meta(&self) -> &crate::engine::Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<crate::engine::Requirement> {
        vec![crate::engine::Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<crate::engine::Provision> {
        vec![crate::engine::Provision::of::<ResolvedSymbols>()]
    }
    fn generate(&self, cx: &mut crate::engine::PluginContext<'_, L>) -> Result<()> {
        let mut names = cx.inputs.get::<SymbolPlan>()?.allocator()?;
        cx.publish(names.resolve()?.clone())
    }
}
