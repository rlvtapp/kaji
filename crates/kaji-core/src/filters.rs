//! Target-neutral operation selection.
//!
//! Kaji applies include/exclude rules before a plugin sees a node.  The Rust
//! engine keeps that rule independent of a specific language backend so a
//! TypeScript client and a Rust client cannot accidentally expose different
//! operation sets for the same configuration.

use crate::{GeneratorConfig, HttpMethod, Operation, Schema};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationFilter {
    Id(String),
    Path(String),
    Method(HttpMethod),
    /// Tags are adapter metadata rather than a property of the minimal neutral
    /// AST. Callers that have them use `matches_with_tags`.
    Tag(String),
}

impl OperationFilter {
    pub fn matches(&self, operation: &Operation) -> bool {
        self.matches_with_tags(operation, &[])
    }

    pub fn matches_with_tags(&self, operation: &Operation, tags: &[String]) -> bool {
        match self {
            Self::Id(pattern) => wildcard_matches(pattern, &operation.id),
            Self::Path(pattern) => wildcard_matches(pattern, &operation.path),
            Self::Method(method) => method == &operation.method,
            Self::Tag(pattern) => tags.iter().any(|tag| wildcard_matches(pattern, tag)),
        }
    }
}

/// Adapter-supplied operation data used by filters that OpenAPI itself owns
/// (such as tags) but that not every target needs in its core AST.
#[derive(Clone, Copy, Debug)]
pub struct OperationContext<'a> {
    pub operation: &'a Operation,
    pub tags: &'a [String],
}

/// Include rules are an OR-set; exclusions always win. An empty include set
/// selects every operation, matching Kaji's default plugin behaviour.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationSelection {
    pub include: Vec<OperationFilter>,
    pub exclude: Vec<OperationFilter>,
}

impl OperationSelection {
    pub fn includes(&self, operation: &Operation) -> bool {
        self.includes_with_tags(operation, &[])
    }

    pub fn includes_with_tags(&self, operation: &Operation, tags: &[String]) -> bool {
        if self
            .exclude
            .iter()
            .any(|filter| filter.matches_with_tags(operation, tags))
        {
            return false;
        }
        self.include.is_empty()
            || self
                .include
                .iter()
                .any(|filter| filter.matches_with_tags(operation, tags))
    }

    pub fn apply<'a>(
        &self,
        operations: impl IntoIterator<Item = &'a Operation>,
    ) -> Vec<&'a Operation> {
        operations
            .into_iter()
            .filter(|operation| self.includes(operation))
            .collect()
    }

    pub fn apply_contexts<'a>(
        &self,
        operations: impl IntoIterator<Item = OperationContext<'a>>,
    ) -> Vec<OperationContext<'a>> {
        operations
            .into_iter()
            .filter(|context| self.includes_with_tags(context.operation, context.tags))
            .collect()
    }
}

/// A rule for a generator's per-node options. Rules are evaluated in source
/// order; every matching rule overlays its partial config, so the last value
/// for a key wins while unrelated earlier keys remain intact. This gives users
/// Speakeasy-style targeted overrides without target backends implementing
/// their own subtly different precedence rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideRule {
    pub filter: OverrideFilter,
    pub values: GeneratorConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverrideFilter {
    OperationId(String),
    Path(String),
    Method(HttpMethod),
    Tag(String),
    SchemaName(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverrideRules {
    pub rules: Vec<OverrideRule>,
}

impl OverrideRules {
    pub fn resolve_operation(
        &self,
        operation: &Operation,
        tags: &[String],
        base: &GeneratorConfig,
    ) -> GeneratorConfig {
        self.resolve(base, |filter| match filter {
            OverrideFilter::OperationId(pattern) => wildcard_matches(pattern, &operation.id),
            OverrideFilter::Path(pattern) => wildcard_matches(pattern, &operation.path),
            OverrideFilter::Method(method) => method == &operation.method,
            OverrideFilter::Tag(pattern) => tags.iter().any(|tag| wildcard_matches(pattern, tag)),
            OverrideFilter::SchemaName(_) => false,
        })
    }

    pub fn resolve_schema(&self, schema: &Schema, base: &GeneratorConfig) -> GeneratorConfig {
        self.resolve(base, |filter| match filter {
            OverrideFilter::SchemaName(pattern) => wildcard_matches(pattern, &schema.name),
            OverrideFilter::OperationId(_)
            | OverrideFilter::Path(_)
            | OverrideFilter::Method(_)
            | OverrideFilter::Tag(_) => false,
        })
    }

    fn resolve(
        &self,
        base: &GeneratorConfig,
        matches: impl Fn(&OverrideFilter) -> bool,
    ) -> GeneratorConfig {
        let mut resolved = base.clone();
        for rule in &self.rules {
            if matches(&rule.filter) {
                resolved.extend(rule.values.clone());
            }
        }
        resolved
    }
}

/// A small, allocation-free glob matcher for config patterns. `*` matches any
/// sequence (including `/`) and `?` matches one Unicode scalar value.
pub fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let (mut pattern_index, mut value_index) = (0usize, 0usize);
    let (mut star, mut retry_value) = (None, 0usize);

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star = Some(pattern_index);
            pattern_index += 1;
            retry_value = value_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            retry_value += 1;
            value_index = retry_value;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}
