fn main() {
    for variable in ["TARGET", "CARGO_CFG_TARGET_FEATURE"] {
        println!(
            "cargo:rustc-env=TYPST_PACK_{variable}={}",
            std::env::var(variable).unwrap_or_default()
        );
    }
}
