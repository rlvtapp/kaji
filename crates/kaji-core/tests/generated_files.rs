use kaji_core::{GeneratedFile, GeneratedTree};

#[test]
fn generated_files_cannot_escape_the_output_directory() {
    assert!(GeneratedFile::new("../outside.ts", "").is_err());
    assert!(GeneratedFile::new("/absolute.ts", "").is_err());
    assert!(GeneratedTree::default().get("missing.ts").is_none());
}
