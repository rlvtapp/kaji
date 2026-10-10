//! Versioned, execution-free inspection of the resolved package dependency graph.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationPlan {
    pub version: u32,
    pub packages: Vec<PackagePlan>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PackagePlan {
    pub path: String,
    pub language: String,
    pub status: String,
    pub diagnostic: Option<String>,
    pub plugins: Vec<PluginNode>,
    pub edges: Vec<ContractEdge>,
    pub stages: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginNode {
    pub id: usize,
    pub kind: String,
    pub label: String,
    pub phase: String,
    pub order: Option<usize>,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    pub native: bool,
    pub handlers: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ContractEdge {
    pub provider: Option<usize>,
    pub consumer: usize,
    pub contract: String,
    pub optional: bool,
    pub selection: String,
}

pub(super) fn describe<L: Language>(package: &Package<L>, native: bool) -> PackagePlan {
    let skipped = native.then(|| package.native_incompatibility()).flatten();
    let resolved = if native && skipped.is_none() {
        package.validate_native().and_then(|_| package.resolve())
    } else {
        package.resolve()
    };
    let (status, diagnostic) = if let Some(reason) = skipped {
        ("skipped", Some(reason))
    } else if let Err(error) = &resolved {
        ("invalid", Some(format!("{error:#}")))
    } else {
        ("ready", None)
    };
    let plan = resolved.as_ref().ok();
    let mut edges = vec![];
    let plugins = package
        .plugins
        .iter()
        .enumerate()
        .map(|(index, plugin)| {
            let requirements = plugin.requires();
            for req in &requirements {
                let provider = plan
                    .and_then(|plan| {
                        plan.bindings[index]
                            .get(&req.contract.type_id)
                            .copied()
                            .flatten()
                    })
                    .or(req.provider)
                    .and_then(|id| package.plugins.iter().position(|p| p.meta().id == id));
                edges.push(ContractEdge {
                    provider,
                    consumer: index,
                    contract: req.contract.name.into(),
                    optional: req.optional,
                    selection: if req.provider.is_some() {
                        "explicit"
                    } else {
                        "automatic"
                    }
                    .into(),
                });
            }
            PluginNode {
                id: index,
                kind: plugin.kind().into(),
                label: package.label(index),
                phase: match plugin.phase() {
                    PluginPhase::Generate => "generate",
                    PluginPhase::Post => "post",
                }
                .into(),
                order: plan.and_then(|plan| plan.order.iter().position(|&i| i == index)),
                provides: plugin.provides().iter().map(|p| p.name.into()).collect(),
                requires: requirements
                    .iter()
                    .map(|r| r.contract.name.into())
                    .collect(),
                handlers: plugin.plan_handlers(),
                native: plugin.supports_native_input(),
            }
        })
        .collect();
    let mut stages = vec!["language finalization".into(), "Post plugins".into()];
    if !package.middleware.is_empty() {
        stages.push("bundled middleware".into());
    }
    stages.push("language file finalization".into());
    if !package.customizations.is_empty() {
        stages.push("source customization".into());
    }
    if package.common.source_quality.is_some() {
        stages.push("source formatting and final-byte quality checks".into());
    }
    stages.push("ownership checks at check/write".into());
    PackagePlan {
        path: package.dir.clone(),
        language: L::NAME.into(),
        status: status.into(),
        diagnostic,
        plugins,
        edges,
        stages,
    }
}

impl GenerationPlan {
    pub fn to_text(&self) -> String {
        let mut lines = vec!["Poolster plan (declarations; no execution)".to_owned()];
        for package in &self.packages {
            lines.push(format!(
                "\n{} [{}] — {}",
                package.path, package.language, package.status
            ));
            if let Some(message) = &package.diagnostic {
                lines.push(format!("  ! {message}"));
            }
            for phase in ["generate", "post"] {
                lines.push(format!("  [{phase}]"));
                let mut nodes: Vec<_> = package
                    .plugins
                    .iter()
                    .filter(|p| p.phase == phase)
                    .collect();
                nodes.sort_by_key(|p| p.order.unwrap_or(p.id));
                for node in nodes {
                    lines.push(format!("    #{} {}", node.id, node.label));
                    for handler in &node.handlers {
                        lines.push(format!("       hook: {handler}"));
                    }
                    for contract in &node.provides {
                        lines.push(format!("       -> {contract}"));
                    }
                    for edge in package.edges.iter().filter(|e| e.consumer == node.id) {
                        lines.push(format!(
                            "       <- {} from {} ({})",
                            edge.contract,
                            edge.provider
                                .map(|id| format!("#{id}"))
                                .unwrap_or_else(|| "unbound".into()),
                            edge.selection
                        ));
                    }
                }
            }
            lines.push(format!("  lifecycle: {}", package.stages.join(" -> ")));
        }
        lines.join("\n")
    }
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
    /// Standalone local viewer; data is escaped before embedding and rendered as text.
    pub fn to_html(&self) -> Result<String> {
        let data = serde_json::to_string(self)?
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('&', "\\u0026");
        Ok(include_str!("overview.html").replace("__PLAN_DATA__", &data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestLanguage;
    impl Language for TestLanguage {
        const NAME: &'static str = "test";
        type Settings = ();
        type Workspace = ();
    }
    struct Data;
    impl Contract for Data {
        const NAME: &'static str = "test.data";
    }
    struct Producer(Meta);
    impl Plugin<TestLanguage> for Producer {
        fn kind(&self) -> &'static str {
            "producer</script>"
        }
        fn meta(&self) -> &Meta {
            &self.0
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Data>()]
        }
        fn generate(&self, _: &mut PluginContext<'_, TestLanguage>) -> Result<()> {
            panic!("planning must not execute")
        }
    }
    #[test]
    fn selected_edges_order_and_safe_html_without_execution() {
        let producer = Producer(Meta::new());
        let handle = producer.0.handle::<Data>();
        let consumer =
            hooks::<TestLanguage>().on_contract(Some(handle), |_, _| panic!("must not execute"));
        let plan = Packages::new()
            .package(Package::new("docs").with(consumer).with(producer))
            .plan(false);
        let package = &plan.packages[0];
        assert_eq!(package.status, "ready");
        assert_eq!(package.edges[0].provider, Some(1));
        assert_eq!(package.plugins[1].order, Some(0));
        assert_eq!(package.plugins[0].handlers, vec!["on_contract<test.data>"]);
        assert!(plan.to_text().contains("from #1"));
        let html = plan.to_html().unwrap();
        assert!(!html.contains("producer</script>"));
        assert!(html.contains("producer\\u003c/script\\u003e"));
    }
    #[test]
    fn invalid_graph_is_inspectable() {
        let consumer = hooks::<TestLanguage>().on_contract::<Data>(None, |_, _| Ok(()));
        let plan = Packages::new()
            .package(Package::new("docs").with(consumer))
            .plan(false);
        assert_eq!(plan.packages[0].status, "invalid");
        assert!(plan.packages[0].diagnostic.is_some());
        assert_eq!(plan.packages[0].edges[0].provider, None);
    }
}
