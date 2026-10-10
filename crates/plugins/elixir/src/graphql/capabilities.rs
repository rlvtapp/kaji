use super::*;
use poolster_core::native::GraphqlIncrementalOperations;
pub struct GraphqlIncremental {
    meta: Meta,
    provider: Option<Handle<GraphqlIncrementalOperations>>,
    style: GraphqlStyle,
    groups: BTreeMap<String, BTreeMap<String, String>>,
}
pub fn graphql_incremental(
    provider: Option<Handle<GraphqlIncrementalOperations>>,
) -> GraphqlIncremental {
    GraphqlIncremental {
        meta: Meta::new(),
        provider,
        style: GraphqlStyle::default(),
        groups: BTreeMap::new(),
    }
}
impl GraphqlIncremental {
    pub fn raw(mut self) -> Self {
        self.style = GraphqlStyle::Raw;
        self
    }
    pub fn flat(mut self) -> Self {
        self.style = GraphqlStyle::Flat;
        self
    }
    pub fn idiomatic(mut self) -> Self {
        self.style = GraphqlStyle::Idiomatic;
        self
    }
    pub fn groups(mut self, groups: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        self.groups = groups;
        self
    }
}
impl Plugin<Elixir> for GraphqlIncremental {
    fn kind(&self) -> &'static str {
        "elixir-graphql-incremental"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GraphqlClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Elixir>) -> Result<()> {
        let contract = cx.inputs.get::<GraphqlIncrementalOperations>()?;
        let definition = &contract.definition;
        let (mut tree, methods) = render_capabilities(
            definition,
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("graphql_client"),
            self.style,
            &self.groups,
            false,
            true,
            Some(&contract.selections),
        )?;
        if let Some(version) = &cx.common.package_version {
            let content = tree
                .get("mix.exs")
                .unwrap()
                .replace("0.0.0", &crate::package_version(version));
            tree.replace(GeneratedFile::new("mix.exs", content)?)?;
        }
        cx.files.append(tree)?;
        cx.publish(GraphqlClient { methods })
    }
}
