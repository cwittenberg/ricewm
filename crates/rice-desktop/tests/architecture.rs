use std::{fs, path::Path};

const MAX_PRODUCTION_LINES: usize = 600;

#[test]
fn production_modules_remain_small_and_safe() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut modules = 0;
    for entry in fs::read_dir(source).expect("source directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        modules += 1;
        let text = fs::read_to_string(&path).expect("UTF-8 Rust source");
        let lines = text.lines().count();
        assert!(
            lines <= MAX_PRODUCTION_LINES,
            "{} has {lines} lines; split it at a responsibility boundary",
            path.display()
        );
        assert!(
            !text.contains("unsafe {") && !text.contains("unsafe fn"),
            "{} introduced unsafe code into the pure policy crate",
            path.display()
        );
    }
    assert!(modules >= 7, "expected responsibility-focused modules");
}

#[test]
fn crate_stays_dependency_free() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("manifest");
    let dependencies = manifest
        .split_once("[dependencies]")
        .expect("dependencies section")
        .1
        .split_once("[lints]")
        .expect("lints section")
        .0;
    assert!(
        dependencies.trim().is_empty(),
        "new dependencies need an explicit footprint review"
    );
}

#[test]
fn scenario_map_remains_visible_in_readme() {
    let readme = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .expect("readme");
    for scenario in [
        "DESK-001", "DESK-002", "DESK-003", "DESK-004", "DESK-006", "DESK-007", "DESK-008",
    ] {
        assert!(readme.contains(scenario), "missing {scenario} mapping");
    }
    for module in [
        "`model`",
        "`wallpaper`",
        "`grid`",
        "`selection`",
        "`menu`",
        "`events`",
    ] {
        assert!(readme.contains(module), "missing {module} responsibility");
    }
}
