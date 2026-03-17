use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_initrd)");
    // 告诉 cargo 如果 initrd.cpio 改变则重新构建
    let initrd_path = "../initrd.cpio";
    println!("cargo:rerun-if-changed={}", initrd_path);
    println!("cargo:rerun-if-changed=src/arch/riscv64/linker.ld");
    println!("cargo:rerun-if-changed=src/arch/riscv64/boot/entry.asm");
    println!("cargo:rerun-if-changed=src/arch/riscv64/trap/trap.asm");

    // 将 initrd.cpio 路径传给 Rust 代码
    let initrd = std::fs::canonicalize(initrd_path)
        .unwrap_or_else(|_| PathBuf::from(""));

    if initrd.exists() {
        println!("cargo:rustc-env=INITRD_PATH={}", initrd.display());
        println!("cargo:rustc-cfg=has_initrd");
    }
}
