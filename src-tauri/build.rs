fn main() {
    // Embed build timestamp into the binary
    let now = chrono::Local::now();
    println!(
        "cargo:rustc-env=BUILD_DATETIME={}",
        now.format("%Y-%m-%d %H:%M")
    );
    tauri_build::build()
}
