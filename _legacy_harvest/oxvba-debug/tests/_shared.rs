use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn fixture_file(name: &str, file: &str) -> PathBuf {
    fixture_dir(name).join(file)
}

pub fn read_fixture(name: &str, file: &str) -> String {
    fs::read_to_string(fixture_file(name, file)).expect("fixture file should be readable")
}

pub fn line_at(text: &str, one_based_line: usize) -> &str {
    text.lines()
        .nth(one_based_line.saturating_sub(1))
        .expect("fixture line should exist")
}
