//! Opt-in serialized bridge. Opaque Rust contracts need not implement a codec.
use crate::{blocks::ContractReference, engine::Contract};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const ENVELOPE_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractEnvelope<T> {
    pub envelope_version: u32,
    pub reference: ContractReference,
    pub payload: T,
}
/// A custom contract can supply its own payload representation.
pub trait ContractCodec<C: Contract> {
    fn encode(&self, value: &C, reference: &ContractReference) -> Result<Vec<u8>>;
    fn decode(&self, bytes: &[u8]) -> Result<(C, ContractReference)>;
}
/// Selecting this codec explicitly opts a serializable contract into the bridge.
pub struct JsonCodec;
impl<C: Contract + Serialize + DeserializeOwned> ContractCodec<C> for JsonCodec {
    fn encode(&self, value: &C, reference: &ContractReference) -> Result<Vec<u8>> {
        validate::<C>(reference)?;
        Ok(serde_json::to_vec(&ContractEnvelope {
            envelope_version: ENVELOPE_VERSION,
            reference: reference.clone(),
            payload: value,
        })?)
    }
    fn decode(&self, bytes: &[u8]) -> Result<(C, ContractReference)> {
        let envelope: ContractEnvelope<C> = serde_json::from_slice(bytes)?;
        ensure!(
            envelope.envelope_version == ENVELOPE_VERSION,
            "unsupported contract envelope version {}",
            envelope.envelope_version
        );
        validate::<C>(&envelope.reference)?;
        Ok((envelope.payload, envelope.reference))
    }
}
fn validate<C: Contract>(reference: &ContractReference) -> Result<()> {
    ensure!(
        reference.contract == C::NAME,
        "codec contract mismatch: expected {}, received {}",
        C::NAME,
        reference.contract
    );
    ensure!(
        !reference.instance.trim().is_empty() && !reference.revision.trim().is_empty(),
        "contract envelope requires instance and revision"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{Blocks, CollectionState, ModelBlock};
    #[test]
    fn envelope_preserves_revision_and_collection_state() {
        let data = Blocks::<ModelBlock> {
            parent: None,
            state: CollectionState::Partial {
                diagnostics: vec!["unrepresented model".into()],
            },
            items: vec![],
        };
        let reference = ContractReference::from_bytes(
            <Blocks<ModelBlock> as Contract>::NAME,
            "doc",
            b"semantic revision",
        );
        let bytes = JsonCodec.encode(&data, &reference).unwrap();
        let (decoded, parent): (Blocks<ModelBlock>, _) = JsonCodec.decode(&bytes).unwrap();
        assert_eq!(decoded, data);
        assert_eq!(parent, reference);
        let mut wrong: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        wrong["envelope_version"] = 2.into();
        assert!(
            <JsonCodec as ContractCodec<Blocks<ModelBlock>>>::decode(
                &JsonCodec,
                &serde_json::to_vec(&wrong).unwrap()
            )
            .is_err()
        );
    }
}
