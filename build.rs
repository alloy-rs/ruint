fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(feature = "pyo3")]
    pyo3_build_config::use_pyo3_cfgs();
}
