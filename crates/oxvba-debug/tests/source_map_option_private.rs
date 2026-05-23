#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn option_private_module_is_dropped() {
    let map = support_source_map::debug_map("Option Private Module\nSub Main()\nEnd Sub");
    assert_eq!(map.file_to_runtime("Module1", 1), None);
    assert_eq!(map.nearest_executable_file_line("Module1", 1), Some(2));
}
