//! Target-neutral grouping of atomic source declarations.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_bytes() -> usize {
    128 * 1024
}

/// A source layout preserves public entrypoints while grouping declarations.
/// Atomic declarations may exceed a byte budget; they are never sliced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SourceLayout {
    SingleFile,
    Chunked {
        #[serde(default = "default_bytes")]
        max_file_bytes: usize,
        #[serde(default)]
        max_declarations: Option<usize>,
    },
    PerOperation,
    PerResource {
        #[serde(default = "default_bytes")]
        max_file_bytes: usize,
        #[serde(default)]
        max_declarations: Option<usize>,
    },
}
impl Default for SourceLayout {
    fn default() -> Self {
        Self::chunked(default_bytes())
    }
}

/// A declaration's estimated source size and optional owning resource.
#[derive(Clone, Copy, Debug)]
pub struct SourceUnit<'a> {
    pub bytes: usize,
    pub resource: Option<&'a str>,
}
impl SourceLayout {
    pub fn chunked(max_file_bytes: usize) -> Self {
        Self::Chunked {
            max_file_bytes,
            max_declarations: None,
        }
    }
    pub fn per_resource(max_file_bytes: usize) -> Self {
        Self::PerResource {
            max_file_bytes,
            max_declarations: None,
        }
    }
    pub fn groups(&self, units: &[SourceUnit<'_>], overhead: usize) -> Result<Vec<Vec<usize>>> {
        let (bytes, declarations) = match self {
            Self::SingleFile => {
                return Ok(if units.is_empty() {
                    vec![]
                } else {
                    vec![(0..units.len()).collect()]
                });
            }
            Self::PerOperation => return Ok((0..units.len()).map(|index| vec![index]).collect()),
            Self::Chunked {
                max_file_bytes,
                max_declarations,
            }
            | Self::PerResource {
                max_file_bytes,
                max_declarations,
            } => (*max_file_bytes, *max_declarations),
        };
        ensure!(bytes > 0, "source layout max_file_bytes must be positive");
        ensure!(
            declarations != Some(0),
            "source layout max_declarations must be positive"
        );
        let mut resources = BTreeMap::<&str, Vec<usize>>::new();
        if matches!(self, Self::PerResource { .. }) {
            for (index, unit) in units.iter().enumerate() {
                resources
                    .entry(unit.resource.unwrap_or("default"))
                    .or_default()
                    .push(index);
            }
        } else {
            resources.insert("default", (0..units.len()).collect());
        }
        let mut result = Vec::new();
        for indices in resources.into_values() {
            let mut group = Vec::new();
            let mut size = overhead;
            for index in indices {
                if !group.is_empty()
                    && (size.saturating_add(units[index].bytes) > bytes
                        || declarations.is_some_and(|limit| group.len() >= limit))
                {
                    result.push(std::mem::take(&mut group));
                    size = overhead;
                }
                group.push(index);
                size = size.saturating_add(units[index].bytes);
            }
            if !group.is_empty() {
                result.push(group);
            }
        }
        Ok(result)
    }
    pub fn uses_modules(&self, units: &[SourceUnit<'_>], overhead: usize) -> Result<bool> {
        let groups = self.groups(units, overhead)?;
        Ok(match self {
            Self::SingleFile => false,
            Self::PerOperation | Self::PerResource { .. } => !units.is_empty(),
            Self::Chunked { max_file_bytes, .. } => {
                groups.len() > 1
                    || units
                        .iter()
                        .fold(overhead, |total, unit| total.saturating_add(unit.bytes))
                        > *max_file_bytes
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_group_atomic_declarations_and_validate_budgets() {
        let units = [
            SourceUnit {
                bytes: 100,
                resource: Some("b"),
            },
            SourceUnit {
                bytes: 800,
                resource: Some("a"),
            },
            SourceUnit {
                bytes: 100,
                resource: Some("b"),
            },
        ];
        assert_eq!(
            SourceLayout::chunked(250).groups(&units, 10).unwrap(),
            vec![vec![0], vec![1], vec![2]]
        );
        assert_eq!(
            SourceLayout::per_resource(250).groups(&units, 10).unwrap(),
            vec![vec![1], vec![0, 2]]
        );
        assert_eq!(
            SourceLayout::PerOperation.groups(&units, 10).unwrap(),
            vec![vec![0], vec![1], vec![2]]
        );
        assert_eq!(
            SourceLayout::SingleFile.groups(&units, 10).unwrap(),
            vec![vec![0, 1, 2]]
        );
        assert!(SourceLayout::chunked(0).groups(&[], 0).is_err());
        assert!(
            SourceLayout::Chunked {
                max_file_bytes: 100,
                max_declarations: Some(0)
            }
            .groups(&[], 0)
            .is_err()
        );
        assert_eq!(
            SourceLayout::Chunked {
                max_file_bytes: usize::MAX,
                max_declarations: Some(2)
            }
            .groups(&units, 0)
            .unwrap(),
            vec![vec![0, 1], vec![2]]
        );
    }
}
