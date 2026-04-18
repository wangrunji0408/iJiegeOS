fn main() {
    println!("cargo:rustc-link-arg=-T{}/linker.ld", std::env::var("CARGO_MANIFEST_DIR").unwrap());
}
