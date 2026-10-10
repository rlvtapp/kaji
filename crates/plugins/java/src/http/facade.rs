//! Facade emission for the java HTTP SDK.
use crate::*;

pub(crate) fn normalized_output_dir(output_dir: &str) -> Result<String> {
    let root = output_dir.trim_matches('/');
    if root.split('/').any(|part| part == "..") {
        bail!("Java SDK output directory cannot contain parent-directory components");
    }
    Ok(root.to_owned())
}
