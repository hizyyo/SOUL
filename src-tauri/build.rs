fn main() {
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    println!("cargo:rerun-if-env-changed=SOUL_SIDECAR_BUILD");
    if std::env::var_os("SOUL_SIDECAR_BUILD").is_some() {
        // Sidecars are built before the Tauri application. Skipping Tauri's
        // externalBin validation here breaks that dependency cycle without
        // manufacturing placeholder binaries.
        return;
    }
    tauri_build::build()
}
