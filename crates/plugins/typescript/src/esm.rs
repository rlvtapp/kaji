//! Relative ESM specifiers must name an actual emitted JavaScript file.
//! TypeScript's Bundler resolution alone does not make extensionless output
//! importable by Node. This pass uses the assembled file inventory, including
//! custom providers and bundled middleware, without modifying source overlays.
use anyhow::Result;
use kaji_core::{GeneratedFile, GeneratedTree};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

pub(crate) fn finalize(tree: &mut GeneratedTree) -> Result<()> {
    let paths = tree
        .iter()
        .map(|(path, _)| path.to_owned())
        .collect::<BTreeSet<_>>();
    let changed = tree
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ts"))
        .map(|(path, source)| (path.to_owned(), rewrite(path, source, &paths)))
        .filter(|(path, rewritten)| tree.get(path) != Some(rewritten.as_str()))
        .collect::<Vec<_>>();
    for (path, source) in changed {
        tree.replace(GeneratedFile::new(path, source)?)?;
    }
    Ok(())
}
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}
fn rewrite(path: &Path, source: &str, paths: &BTreeSet<PathBuf>) -> String {
    let bytes = source.as_bytes();
    let mut edits = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' | b'`' => {
                index = skip_string(bytes, index);
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && &bytes[index..index + 2] != b"*/" {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'$'))
                {
                    index += 1;
                }
                let token = &source[start..index];
                if token != "from" && token != "import" {
                    continue;
                }
                let mut value = index;
                while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
                    value += 1;
                }
                if token == "import" && bytes.get(value) == Some(&b'(') {
                    value += 1;
                    while bytes.get(value).is_some_and(u8::is_ascii_whitespace) {
                        value += 1;
                    }
                }
                if !bytes
                    .get(value)
                    .is_some_and(|byte| matches!(byte, b'\'' | b'"'))
                {
                    continue;
                }
                let end = skip_string(bytes, value);
                if end <= value + 1 || bytes[end - 1] != bytes[value] {
                    continue;
                }
                let specifier = &source[value + 1..end - 1];
                if let Some(updated) = resolve(path, specifier, paths) {
                    edits.push((value + 1, end - 1, updated));
                }
                index = end;
            }
            _ => {
                index += 1;
            }
        }
    }
    let mut output = String::new();
    let mut copied = 0;
    for (start, end, replacement) in edits {
        output.push_str(&source[copied..start]);
        output.push_str(&replacement);
        copied = end;
    }
    output.push_str(&source[copied..]);
    output
}
fn resolve(path: &Path, specifier: &str, paths: &BTreeSet<PathBuf>) -> Option<String> {
    if !(specifier.starts_with("./") || specifier.starts_with("../"))
        || specifier.contains(['\\', '?', '#'])
    {
        return None;
    }
    if [".js", ".mjs", ".cjs", ".json"]
        .iter()
        .any(|ext| specifier.ends_with(ext))
    {
        return None;
    }
    let joined = path.parent().unwrap_or(Path::new("")).join(specifier);
    let mut normalized = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(value) => normalized.push(value),
            _ => return None,
        }
    }
    if specifier.ends_with(".ts") && paths.contains(&normalized) {
        return Some(format!("{}.js", specifier.strip_suffix(".ts")?));
    }
    let direct = PathBuf::from(format!("{}.ts", normalized.to_string_lossy()));
    if paths.contains(&direct) {
        Some(format!("{specifier}.js"))
    } else if paths.contains(&normalized.join("index.ts")) {
        Some(format!("{}/index.js", specifier.trim_end_matches('/')))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewrites_only_module_specifiers_and_keeps_explicit_or_external_imports() {
        let paths = ["src/index.ts", "src/model.test.ts", "models/index.ts"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let source = "// from '../models'\nconst text = \"from '../models'\"; const from = '../models';\nexport * from '../models'; import type {M} from './model.test';\nimport('./model.test.ts'); import '../models'; import x from 'external'; import y from './model.test.js';";
        let updated = rewrite(Path::new("src/index.ts"), source, &paths);
        assert!(updated.contains("// from '../models'"));
        assert!(updated.contains("const text = \"from '../models'\"; const from = '../models';"));
        assert!(updated.contains("from '../models/index.js'"));
        assert!(updated.contains("from './model.test.js'"));
        assert!(updated.contains("import('./model.test.js')"));
        assert!(updated.contains("import '../models/index.js'"));
        assert!(updated.contains("from 'external'"));
        assert_eq!(
            rewrite(Path::new("src/index.ts"), &updated, &paths),
            updated
        );
    }
}
