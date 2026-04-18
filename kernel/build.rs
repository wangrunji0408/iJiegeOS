fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{}/linker.ld", dir);
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rustc-link-arg=-static");
    println!("cargo:rerun-if-changed={}/linker.ld", dir);
}
