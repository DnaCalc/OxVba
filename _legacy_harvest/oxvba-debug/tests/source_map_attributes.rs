#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn attribute_lines_are_dropped() {
    let map = support_source_map::debug_map("Attribute VB_Name = \"Module1\"\nSub Main()\nEnd Sub");
    assert_eq!(map.file_to_runtime("Module1", 1), None);
    assert_eq!(map.file_to_runtime("Module1", 2), Some(1));
}
