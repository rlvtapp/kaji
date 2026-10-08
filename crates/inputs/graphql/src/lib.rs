//! GraphQL schema ingestion. Query, mutation and subscription roots remain
//! GraphQL operations; the validated schema retains fields, arguments, wrappers,
//! directives, extensions and source locations for future generators.

mod defaults;
mod lowering;
pub use lowering::lower as lower_operations;

use anyhow::{Result, anyhow};
use apollo_compiler::{Schema, schema::ExtendedType, validation::Valid};

use poolster_core::input::{InputOperation as OperationSummary, InputSummary as ContractSummary};

#[derive(Debug, Clone)]
pub struct GraphqlDocument {
    pub schema: Valid<Schema>,
}

/// Parse and validate a complete GraphQL SDL schema.
pub fn parse(source: &str) -> Result<GraphqlDocument> {
    let schema = Schema::parse_and_validate(source, "schema.graphql")
        .map_err(|errors| anyhow!("invalid GraphQL schema: {errors}"))?;
    defaults::validate(&schema)?;
    Ok(GraphqlDocument { schema })
}

impl GraphqlDocument {
    pub fn summary(&self) -> ContractSummary {
        let roots = &self.schema.schema_definition;
        let mut operations = Vec::new();
        for (kind, root) in [
            ("query", &roots.query),
            ("mutation", &roots.mutation),
            ("subscription", &roots.subscription),
        ] {
            if let Some(root) = root {
                if let Some(ExtendedType::Object(object)) = self.schema.types.get(root.as_str()) {
                    operations.extend(object.fields.keys().map(|name| OperationSummary {
                        name: name.to_string(),
                        kind: kind.to_owned(),
                    }));
                }
            }
        }
        // Exclude compiler-injected introspection types and built-in scalars.
        let types = self
            .schema
            .types
            .iter()
            .filter(|(_, definition)| !definition.is_built_in())
            .map(|(name, _)| name.to_string())
            .collect();
        ContractSummary {
            format: "graphql".into(),
            title: roots
                .description
                .as_ref()
                .map(|text| text.to_string())
                .unwrap_or_else(|| "GraphQL schema".to_owned()),
            version: None,
            types,
            operations,
        }
    }
}

use anyhow::Context;

impl poolster_core::engine::Contract for GraphqlDocument {
    const NAME: &'static str = "poolster.graphql";
}

/// Native graphql input provider.
pub struct GraphqlInput;
impl poolster_core::input::InputPlugin for GraphqlInput {
    fn id(&self) -> &str {
        "graphql.apollo"
    }
    fn format(&self) -> &str {
        "graphql"
    }
    fn load(&self, path: &std::path::Path) -> anyhow::Result<poolster_core::input::InputContract> {
        let document = parse(
            &std::fs::read_to_string(path)
                .with_context(|| format!("cannot read contract {}", path.display()))?,
        )?;
        let mut input = poolster_core::input::InputContract::new(document.summary());

        input.publish(document)?;
        Ok(input)
    }
    fn load_with_options(
        &self,
        path: &std::path::Path,
        options: &poolster_core::input::InputOptions,
    ) -> Result<poolster_core::input::InputContract> {
        anyhow::ensure!(
            options.import_roots.is_empty()
                && options.broker.is_none()
                && options.workflow_sources.is_empty(),
            "GraphQL input supports operation_files only; imports, broker and workflow options are unsupported"
        );
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read schema {}", path.display()))?;
        let document = parse(&source)?;
        let operations = options
            .operation_files
            .iter()
            .map(|p| {
                std::fs::read_to_string(p)
                    .with_context(|| format!("cannot read operation file {}", p.display()))
            })
            .collect::<Result<Vec<_>>>()?
            .join("\n");
        let mut input = poolster_core::input::InputContract::new(document.summary());
        if !operations.trim().is_empty() {
            input.publish(lower_operations(&document.schema, &source, &operations)?)?;
        }
        input.publish(document)?;
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_roots_scalars_and_nested_references_are_preserved() {
        let document = parse(
            r#"
            "Inventory API"
            schema { query: Read mutation: Write subscription: Events }
            scalar DateTime
            interface Node { id: ID! }
            type Item implements Node { id: ID! createdAt: DateTime! }
            input Filter { ids: [ID!]! }
            type Read { items(filter: Filter): [[Item!]!]! }
            type Write { add: Item! }
            type Events { changed: Item! }
        "#,
        )
        .unwrap();
        let summary = document.summary();
        assert_eq!(summary.title, "Inventory API");
        assert_eq!(
            summary
                .operations
                .iter()
                .map(|op| (op.name.as_str(), op.kind.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("items", "query"),
                ("add", "mutation"),
                ("changed", "subscription")
            ]
        );
        assert!(summary.types.contains(&"DateTime".to_owned()));
        assert!(summary.types.contains(&"Filter".to_owned()));
        assert!(!summary.types.contains(&"__Schema".to_owned()));
        assert!(!summary.types.contains(&"ID".to_owned()));
        let ExtendedType::Object(read) = &document.schema.types["Read"] else {
            panic!()
        };
        assert_eq!(read.fields["items"].ty.to_string(), "[[Item!]!]!");
        assert!(
            read.fields["items"]
                .arguments
                .iter()
                .any(|arg| arg.name == "filter")
        );
    }

    #[test]
    fn default_roots_and_extensions_are_included() {
        let document =
            parse("type Query { hello: String! } extend type Query { goodbye: String }").unwrap();
        assert_eq!(
            document
                .summary()
                .operations
                .iter()
                .map(|op| op.name.as_str())
                .collect::<Vec<_>>(),
            vec!["hello", "goodbye"]
        );
    }

    #[test]
    fn schema_extensions_and_shared_field_names_keep_operation_kinds() {
        let document = parse(
            r#"
            schema { query: Read }
            extend schema { mutation: Write subscription: Events }
            type Read { item: String }
            type Write { item: String }
            type Events { item: String }
        "#,
        )
        .unwrap();
        let summary = document.summary();
        assert_eq!(
            summary
                .operations
                .iter()
                .map(|operation| (operation.name.as_str(), operation.kind.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("item", "query"),
                ("item", "mutation"),
                ("item", "subscription")
            ]
        );
    }

    #[test]
    fn directives_unions_and_enum_values_remain_in_native_schema() {
        let document = parse(
            r#"
            directive @owner(team: String!) on OBJECT
            enum State { ACTIVE ARCHIVED }
            type Item @owner(team: "inventory") { state: State! }
            type Other { id: ID! }
            union SearchResult = Item | Other
            type Query { search: [SearchResult!]! }
        "#,
        )
        .unwrap();
        assert!(document.schema.directive_definitions.contains_key("owner"));
        let ExtendedType::Object(item) = &document.schema.types["Item"] else {
            panic!()
        };
        assert_eq!(item.directives.0.len(), 1);
        let ExtendedType::Union(union) = &document.schema.types["SearchResult"] else {
            panic!()
        };
        assert_eq!(union.members.len(), 2);
        let ExtendedType::Enum(state) = &document.schema.types["State"] else {
            panic!()
        };
        assert_eq!(state.values.len(), 2);
    }

    #[test]
    fn invalid_syntax_references_duplicates_and_roots_are_rejected() {
        for source in [
            "type Query {",
            "type Query { missing: [[Unknown!]!]! }",
            "type Query { hello: String } type Query { goodbye: String }",
            "type Query { hello: String hello: Int }",
            "schema { query: String }",
            "type Query { item(filter: Item): String } type Item { id: ID }",
            "type Query { item: Filter } input Filter { id: ID }",
            "query Hello { hello }",
            "type Query { hello: String } extend type Query { hello: Int }",
            "type Mutation { hello: String }",
        ] {
            assert!(parse(source).is_err(), "accepted invalid schema: {source}");
        }
    }
}
