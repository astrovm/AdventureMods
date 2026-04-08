use std::fs;
use std::path::Path;

#[test]
fn vendored_encode_unicode_matches_lockfile_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lockfile = fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let cargo_sources = fs::read_to_string(root.join("build-aux/cargo-sources.json")).unwrap();

    assert!(
        lockfile.contains("name = \"encode_unicode\"\nversion = \"1.0.0\""),
        "expected Cargo.lock to resolve encode_unicode 1.0.0"
    );
    assert!(
        cargo_sources.contains("encode_unicode-1.0.0.crate"),
        "expected vendored cargo sources to include encode_unicode-1.0.0"
    );
    assert!(
        !cargo_sources.contains("encode_unicode-0.3.6.crate"),
        "vendored cargo sources should not point at encode_unicode-0.3.6"
    );
}
