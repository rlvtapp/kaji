//! Optional, typed building blocks. Whole contracts need not expose blocks.
//!
//! A provider may publish `Blocks<T>` alongside its native contract. Consumers
//! require the exact block type through the existing typed dependency graph;
//! tags are descriptive capabilities, never a substitute for Rust type safety.
pub mod http;
use crate::engine::Contract;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A third-party block type owns its stable contract identity.
pub trait Block: Send + Sync + 'static {
    const CONTRACT_NAME: &'static str;
}

/// An ID is scoped to a provider's document, not a generated symbol name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockId {
    pub source: String,
    pub local: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockReference {
    pub contract: String,
    pub id: BlockId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMetadata {
    pub id: BlockId,
    #[serde(default)]
    pub parent: Option<ContractReference>,
    /// Open vocabulary: plugins can add namespaced capabilities.
    pub capabilities: BTreeSet<String>,
    /// Related elements retain context across separately published contracts.
    pub references: Vec<BlockReference>,
    /// Native location, such as a JSON pointer or schema coordinate.
    pub location: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingBlock<T> {
    pub metadata: BlockMetadata,
    pub value: T,
}

/// Completeness is independent of cardinality: an empty collection can be complete.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CollectionState {
    #[default]
    Complete,
    Partial {
        diagnostics: Vec<String>,
    },
    Unavailable {
        diagnostics: Vec<String>,
    },
}

/// Stable contract identity and changing revision are deliberately separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractReference {
    pub contract: String,
    pub instance: String,
    pub revision: String,
}
impl ContractReference {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.contract.trim().is_empty()
                && !self.instance.trim().is_empty()
                && !self.revision.trim().is_empty(),
            "contract provenance requires contract identity, instance and revision"
        );
        Ok(())
    }

    pub fn from_bytes(
        contract: impl Into<String>,
        instance: impl Into<String>,
        bytes: &[u8],
    ) -> Self {
        use sha2::{Digest, Sha256};
        Self {
            contract: contract.into(),
            instance: instance.into(),
            revision: Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        }
    }
}

/// Independently consumable collection, published using normal contract APIs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blocks<T> {
    #[serde(default)]
    pub parent: Option<ContractReference>,
    #[serde(default)]
    pub state: CollectionState,
    pub items: Vec<BuildingBlock<T>>,
}

impl<T: Block> Blocks<T> {
    pub fn with_parent(mut self, parent: ContractReference) -> Self {
        for item in &mut self.items {
            item.metadata.parent = Some(parent.clone());
        }
        self.parent = Some(parent);
        self
    }
    pub fn require_parent(&self, parent: &ContractReference) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.parent.as_ref() == Some(parent)
                && self
                    .items
                    .iter()
                    .all(|item| item.metadata.parent.as_ref() == Some(parent)),
            "block collection parent contract/revision does not match selected whole contract"
        );
        Ok(())
    }
    pub fn require_complete(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(self.state, CollectionState::Complete),
            "{} requires complete blocks; collection state: {:?}",
            T::CONTRACT_NAME,
            self.state
        );
        Ok(())
    }

    pub fn require_nonempty(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.items.is_empty(),
            "{} requires nonempty blocks",
            T::CONTRACT_NAME
        );
        Ok(())
    }
    /// Validate identity before publishing or merging a block collection.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !T::CONTRACT_NAME.trim().is_empty(),
            "block contract identity cannot be empty"
        );
        if let Some(parent) = &self.parent {
            parent.validate()?;
            anyhow::ensure!(
                self.items
                    .iter()
                    .all(|item| item.metadata.parent.as_ref() == Some(parent)),
                "block item provenance differs from collection parent"
            );
        }
        anyhow::ensure!(
            !matches!(self.state, CollectionState::Unavailable { .. }) || self.items.is_empty(),
            "unavailable collection cannot contain blocks"
        );
        let mut ids = BTreeSet::new();
        for item in &self.items {
            if let Some(parent) = &item.metadata.parent {
                parent.validate()?;
            }
            let id = &item.metadata.id;
            anyhow::ensure!(
                !id.source.trim().is_empty() && !id.local.trim().is_empty(),
                "block source and local identities cannot be empty"
            );
            anyhow::ensure!(
                ids.insert(id),
                "duplicate block identity {}:{} in {}",
                id.source,
                id.local,
                T::CONTRACT_NAME
            );
            anyhow::ensure!(
                item.metadata
                    .capabilities
                    .iter()
                    .all(|tag| !tag.trim().is_empty()),
                "capability tags cannot be empty"
            );
            for reference in &item.metadata.references {
                anyhow::ensure!(
                    !reference.contract.trim().is_empty()
                        && !reference.id.source.trim().is_empty()
                        && !reference.id.local.trim().is_empty(),
                    "block reference identities cannot be empty"
                );
            }
        }
        Ok(())
    }
    /// Resolve only this exact typed contract; cross-contract refs need another collection.
    pub fn resolve(&self, reference: &BlockReference) -> Option<&BuildingBlock<T>> {
        if reference.contract != T::CONTRACT_NAME {
            return None;
        }
        self.items
            .iter()
            .find(|item| item.metadata.id == reference.id)
    }
    /// Merge providers without silently overwriting conflicting identities.
    pub fn merge(&mut self, other: Self) -> anyhow::Result<()> {
        self.validate()?;
        other.validate()?;
        let ids: BTreeSet<_> = self.items.iter().map(|item| &item.metadata.id).collect();
        anyhow::ensure!(
            other
                .items
                .iter()
                .all(|item| !ids.contains(&item.metadata.id)),
            "block collections contain overlapping identities for {}",
            T::CONTRACT_NAME
        );
        let diagnostics = |state: &CollectionState| match state {
            CollectionState::Complete => Vec::new(),
            CollectionState::Partial { diagnostics }
            | CollectionState::Unavailable { diagnostics } => diagnostics.clone(),
        };
        let mut combined = diagnostics(&self.state);
        combined.extend(diagnostics(&other.state));
        self.state = match (&self.state, &other.state) {
            (CollectionState::Complete, CollectionState::Complete) => CollectionState::Complete,
            (CollectionState::Unavailable { .. }, CollectionState::Unavailable { .. }) => {
                CollectionState::Unavailable {
                    diagnostics: combined,
                }
            }
            _ => CollectionState::Partial {
                diagnostics: combined,
            },
        };
        if self.parent != other.parent {
            self.parent = None;
        }
        self.items.extend(other.items);
        self.items.sort_by(|a, b| a.metadata.id.cmp(&b.metadata.id));
        Ok(())
    }
}

impl<T: Block> Contract for Blocks<T> {
    const NAME: &'static str = T::CONTRACT_NAME;
}

impl<T> Blocks<T> {
    /// Tags filter elements after the typed contract has been selected.
    pub fn with_capability<'a>(
        &'a self,
        capability: &'a str,
    ) -> impl Iterator<Item = &'a BuildingBlock<T>> + 'a {
        self.items
            .iter()
            .filter(move |item| item.metadata.capabilities.contains(capability))
    }
}

/// Named shared data shapes; wire-specific details remain separate contracts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelBlock {
    pub name: String,
    pub ty: crate::native::ModelType,
}
impl Block for ModelBlock {
    const CONTRACT_NAME: &'static str = "poolster.model-blocks.v1";
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Custom;
    impl Block for Custom {
        const CONTRACT_NAME: &'static str = "example.custom-blocks.v1";
    }
    struct Opaque;
    impl Contract for Opaque {
        const NAME: &'static str = "example.opaque.v1";
    }

    #[test]
    fn custom_blocks_and_opaque_contracts_are_independent() {
        assert_eq!(
            <Blocks<Custom> as Contract>::NAME,
            "example.custom-blocks.v1"
        );
        assert_eq!(Opaque::NAME, "example.opaque.v1");
        let blocks = Blocks {
            parent: None,
            state: CollectionState::Complete,
            items: vec![BuildingBlock {
                metadata: BlockMetadata {
                    parent: None,
                    id: BlockId {
                        source: "orders".into(),
                        local: "created".into(),
                    },
                    capabilities: BTreeSet::from(["example.delivery".into()]),
                    references: vec![],
                    location: None,
                },
                value: Custom,
            }],
        };
        assert_eq!(blocks.with_capability("example.delivery").count(), 1);
        assert_eq!(blocks.with_capability("http").count(), 0);
        let mut input = crate::input::InputContract::new(crate::input::InputSummary {
            format: "example".into(),
            title: "Custom".into(),
            version: None,
            types: vec![],
            operations: vec![],
        });
        input.publish(Opaque).unwrap();
        input.publish(blocks).unwrap();
        assert!(input.get::<Opaque>().is_ok());
        assert_eq!(input.get::<Blocks<Custom>>().unwrap().items.len(), 1);
        assert!(input.take::<Opaque>().is_ok());
        assert_eq!(input.take::<Blocks<Custom>>().unwrap().items.len(), 1);
    }
}
