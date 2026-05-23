#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn preamble_only_module_has_no_executable_lines() {
    let map = support_source_map::debug_map("Option Explicit\nOption Compare Text");
    assert_eq!(map.file_to_runtime("Module1", 1), Some(1));
    assert!(map.executable_file_lines("Module1").is_empty());
}
