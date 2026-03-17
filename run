#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
KERNEL="$SCRIPT_DIR/kernel/target/riscv64gc-unknown-none-elf/release/kernel"

# 如果内核不存在，先构建
if [ ! -f "$KERNEL" ]; then
    echo "Building kernel..."
    cd "$SCRIPT_DIR/kernel"
    cargo build --release
    cd "$SCRIPT_DIR"
fi

echo "Starting QEMU with iJiege OS..."
echo "Nginx will be available at http://localhost:8080"
echo "Press Ctrl+A X to exit QEMU"

qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -smp 1 \
    -m 128M \
    -bios default \
    -kernel "$KERNEL" \
    -nographic \
    -serial mon:stdio \
    -netdev user,id=net0,hostfwd=tcp::8080-:80 \
    -device virtio-net-device,netdev=net0 \
    -drive file="$SCRIPT_DIR/disk.img",format=raw,id=blk0,if=none 2>/dev/null || \
qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -smp 1 \
    -m 128M \
    -bios default \
    -kernel "$KERNEL" \
    -nographic \
    -serial mon:stdio \
    -netdev user,id=net0,hostfwd=tcp::8080-:80 \
    -device virtio-net-device,netdev=net0
