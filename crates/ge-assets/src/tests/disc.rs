use super::*;

#[test]
fn rejects_paths_that_could_escape_asset_root() {
    for name in ["", ".", "..", "../code", "a/b", "a\\b", "C:code", "a\0b"] {
        assert!(validate_component(name).is_err(), "{name:?}");
    }
    assert!(validate_component("textures.bin").is_ok());
}

#[test]
fn identifies_code_separately_from_assets() {
    for path in ["main.dol", "modules/foo.REL", "code.elf", "module.rso"] {
        assert!(is_executable(path));
    }
    assert!(!is_executable("textures.bin"));
}
