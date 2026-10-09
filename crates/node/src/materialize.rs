//! Ownership-aware output materialization.
use super::*;
pub struct Materialize {
    files: String,
    output: String,
    write: bool,
    preserve_prefixes: Vec<String>,
}

impl Task for Materialize {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let files: Vec<OutputFile> = serde_json::from_str(&self.files).map_err(napi_error)?;
        let mut tree = GeneratedTree::default();
        for file in files {
            let path = file.path.clone();
            let generated = GeneratedFile::new(&path, file.contents).map_err(napi_error)?;
            if file.preserve_existing {
                tree.insert_custom(generated).map_err(napi_error)?;
            } else {
                tree.insert(generated).map_err(napi_error)?;
            }
            if let Some(owner) = file.owner {
                tree.set_owner(path, owner).map_err(napi_error)?;
            }
        }
        let output = Path::new(&self.output);
        for prefix in &self.preserve_prefixes {
            tree.preserve_owned_prefix(output, prefix)
                .map_err(napi_error)?;
        }
        let changes = tree.check(output).map_err(napi_error)?;
        if self.write {
            tree.write_to(output)
                .with_context(|| format!("write generated output to {}", output.display()))
                .map_err(napi_anyhow)?;
        }
        serde_json::to_string(&changes).map_err(napi_error)
    }

    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn materialize(
    files: String,
    output: String,
    write: bool,
    preserve_prefixes: Option<Vec<String>>,
) -> AsyncTask<Materialize> {
    AsyncTask::new(Materialize {
        files,
        output,
        write,
        preserve_prefixes: preserve_prefixes.unwrap_or_default(),
    })
}
