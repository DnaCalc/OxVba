#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn compiler_inserted_helper_lines_are_non_user() {
    let map = support_source_map::debug_map("Sub Main()\nDim x As Long\nx = 1\nEnd Sub");
    assert_eq!(map.runtime_to_file("__OxVbaGenerated", 1), None);
    assert!(!map.executable_file_lines("Module1").contains(&0));
}
