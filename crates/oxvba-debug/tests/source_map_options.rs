#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn attribute_dropped_option_explicit_preserved() {
    let map = support_source_map::debug_map(
        "Attribute VB_Name = \"Module1\"\nOption Explicit\nSub Main()\nEnd Sub",
    );
    assert_eq!(map.file_to_runtime("Module1", 1), None);
    assert_eq!(map.file_to_runtime("Module1", 2), Some(1));
}

#[test]
fn option_compare_and_option_base_are_preserved() {
    let map =
        support_source_map::debug_map("Option Compare Text\nOption Base 1\nSub Main()\nEnd Sub");
    assert_eq!(map.file_to_runtime("Module1", 1), Some(1));
    assert_eq!(map.file_to_runtime("Module1", 2), Some(2));
}
