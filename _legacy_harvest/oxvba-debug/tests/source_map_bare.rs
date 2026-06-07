#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn bare_source_maps_identity() {
    let map = support_source_map::debug_map("Sub Main()\nDim x As Long\nx = 1\nEnd Sub");
    assert_eq!(map.file_to_runtime("Module1", 2), Some(2));
    assert_eq!(map.runtime_to_file("Module1", 3), Some(3));
}
