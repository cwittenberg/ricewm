use std::{fs, path::Path};

#[test]
fn production_modules_stay_small_and_separated() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let expected = [
        "animation.rs",
        "cache.rs",
        "command.rs",
        "geometry.rs",
        "lib.rs",
        "model.rs",
        "navigation.rs",
    ];
    let mut actual: Vec<_> = fs::read_dir(&source_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    actual.sort();
    assert_eq!(actual, expected);

    for file in expected {
        let text = fs::read_to_string(source_dir.join(file)).unwrap();
        let lines = text.lines().count();
        assert!(lines <= 600, "{file} has {lines} lines; split at 600");
        assert!(!text.contains("unsafe"), "{file} contains unsafe code");
        assert!(!text.contains("std::fs"), "{file} performs filesystem I/O");
        assert!(!text.contains("std::time"), "{file} reads a host clock");
    }
}

#[test]
fn crate_depends_only_on_the_shared_first_party_image_contract() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let dependency_section = manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap()
        .split('[')
        .next()
        .unwrap();
    assert_eq!(
        dependency_section.trim(),
        "rice-image-contract.workspace = true"
    );
}
