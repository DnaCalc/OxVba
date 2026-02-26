use oxvba_host::{Engine, HostConfig};

fn main() {
    let config = HostConfig {
        enable_jit: false,
        root_object_name: Some("Application".to_string()),
    };

    let engine = Engine::new(config);
    let source = "Sub Main()\nEnd Sub";

    if let Err(err) = engine.execute_source(source) {
        eprintln!("oxvba: execution failed: {err}");
        std::process::exit(1);
    }
}
