#[path = "support_source_map/mod.rs"]
mod support_source_map;

#[test]
fn runtime_to_file_round_trips_executable_user_lines() {
    let map = support_source_map::debug_map("Sub Main()\nDim x As Long\nx = 1\nEnd Sub");
    for line in map.executable_file_lines("Module1") {
        let runtime = map.file_to_runtime("Module1", line).expect("runtime line");
        assert_eq!(map.runtime_to_file("Module1", runtime), Some(line));
    }
}
