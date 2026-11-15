fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    let t = std::fs::read_to_string("Cargo.toml").unwrap_or_default();
    let v = t
        .lines()
        .find(|l| l.trim_start().starts_with("version_name"))
        .and_then(|l| l.split('"').nth(1))
        .unwrap_or("?")
        .to_string();
    println!("cargo:rustc-env=CSAN_VERSION={}", v);
}
