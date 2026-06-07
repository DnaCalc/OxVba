fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|arg| arg == "--dump-shell-manifest") {
        let manifest = oxvba_web_shell::shell_manifest();
        let json = serde_json::to_string_pretty(&manifest).expect("serialize shell manifest");
        println!("{json}");
        return;
    }

    println!(
        "oxvba-web-shell baseline scaffold\nentry asset: {}\nuse --dump-shell-manifest to inspect the embedded frontend baseline",
        oxvba_web_shell::shell_manifest().entry_asset_path
    );
}

fn print_help() {
    println!("usage: oxvba-web-shell [--dump-shell-manifest]");
    println!("desktop-first OxVba web shell baseline scaffold");
}
