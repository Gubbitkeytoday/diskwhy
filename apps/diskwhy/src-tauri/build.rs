use std::path::Path;

fn main() {
    // `bundle.resources` maps binaries/diskwhy-cli.exe into the installer, and tauri-build refuses to run when a mapped
    // file is missing. `pnpm cli:build` (run by `beforeBuildCommand`) puts the real CLI there. For plain `cargo build/test`
    // and `tauri dev` we create an EMPTY placeholder so the crate still compiles; the app treats a 0-byte CLI as "not found"
    // and shows the `pnpm cli:build` hint instead of pretending it works.
    let cli = Path::new("binaries").join("diskwhy-cli.exe");
    println!("cargo:rerun-if-changed={}", cli.display());
    if !cli.exists() {
        let _ = std::fs::create_dir_all("binaries");
        if std::fs::write(&cli, b"").is_ok() {
            println!("cargo:warning=binaries/diskwhy-cli.exe missing - created an empty placeholder. Run `pnpm cli:build` before `tauri build`.");
        }
    }
    tauri_build::build()
}
