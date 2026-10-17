fn main() {
    emit_engine_version();
    stage_readme();
}

/// Reports the embedded Typst release for `typst_pack::VERSION` and the pack
/// manifest. Pinning policy is enforced by `tests/embedded_typst_gate.rs`, not
/// here, so downstream builds never read workspace files.
fn emit_engine_version() {
    println!("cargo:rerun-if-changed=embedded-typst.toml");
    let baseline: toml::Value = toml::from_str(include_str!("embedded-typst.toml"))
        .unwrap_or_else(|error| panic!("failed to parse embedded-typst.toml: {error}"));
    let version = baseline["typst"]["version"]
        .as_str()
        .expect("embedded-typst.toml must declare [typst] version");
    println!("cargo:rustc-env=TYPST_PACK_ENGINE_VERSION={version}");
}

/// Copies the README into `OUT_DIR` so the crate docs can include it both in
/// the workspace and in the published package.
fn stage_readme() {
    let readme_path = if std::path::Path::new("README.md").is_file() {
        "README.md"
    } else {
        "../../README.md"
    };
    println!("cargo:rerun-if-changed={readme_path}");
    let output = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR to build scripts"),
    )
    .join("README.md");
    std::fs::copy(readme_path, output)
        .unwrap_or_else(|error| panic!("failed to stage {readme_path}: {error}"));
}
