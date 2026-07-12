fn main() {
    println!("cargo:rerun-if-changed=../crates/nextmail-storage/migrations");
    tauri_build::build()
}
