#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn comments_are_preserved_in_file_mapping() {
    let map = support_source_map::debug_map("Sub Main()\n' comment\nDim x As Long\nEnd Sub");
    assert_eq!(map.file_to_runtime("Module1", 2), Some(2));
    assert_eq!(map.runtime_to_file("Module1", 2), Some(2));
}
