use super::source_file_stem;
#[test]
fn source_names_are_readable_bounded_and_identity_specific() {
    assert_eq!(source_file_stem("ReadUser"), "ReadUser");
    let name = "Read".repeat(100);
    let first = source_file_stem(&name);
    assert!(first.len() < 96);
    assert_eq!(first, source_file_stem(&name));
    assert_ne!(first, source_file_stem(&(name + "Other")));
    assert_ne!(source_file_stem("a/b"), source_file_stem("a_b"));
    assert!(!source_file_stem("../escape").contains('/'));
}
