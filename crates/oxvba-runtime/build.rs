fn main() {
    println!("cargo::rustc-check-cfg=cfg(miri)");
}
