pub(crate) fn pascal_identifier(value: &str) -> String {
    crate::symbols::identifier(value)
}

pub(crate) fn lower_camel_identifier(value: &str) -> String {
    crate::symbols::camel(value)
}

/// Stable, filesystem-safe module name for an operation. Public function names
/// stay fully descriptive; only an excessively long filename is compacted.
pub(crate) fn operation_file_identifier(value: &str) -> String {
    let identifier = lower_camel_identifier(value);
    const MAX_PREFIX_CHARS: usize = 96;
    if identifier.chars().count() <= MAX_PREFIX_CHARS {
        return identifier;
    }
    let prefix = identifier
        .chars()
        .take(MAX_PREFIX_CHARS)
        .collect::<String>();
    format!("{prefix}_{:016x}", stable_hash(value))
}

pub(crate) fn stable_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}
